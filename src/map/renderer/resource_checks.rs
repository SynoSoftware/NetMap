//! Opt-in acceptance measurements through the production map owners.
//! Run with tests/native-map-resources.ps1 and a completed reference capture.

use super::*;
use crate::{
    connections::{
        Bytes, Endpoint, Life, Observation, ObservationId, ProcessId, Rates, TcpState, Tuple,
    },
    geography::{Point, World},
    geolocation::Location,
    map::{
        Appearance, Inspection, Palette, Speeds,
        model::{Located, Model},
    },
};
use std::{
    cell::Cell,
    fs::{self, File},
    io::{BufWriter, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::{FILETIME, HANDLE, RECT, S_FALSE, S_OK},
        Graphics::{
            Direct2D::{Common::*, *},
            Direct3D11::{
                D3D11_ASYNC_GETDATA_DONOTFLUSH, D3D11_QUERY, D3D11_QUERY_DATA_TIMESTAMP_DISJOINT,
                D3D11_QUERY_DESC, D3D11_QUERY_TIMESTAMP, D3D11_QUERY_TIMESTAMP_DISJOINT,
                ID3D11DeviceContext, ID3D11Query,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::{
                GetCurrentProcess, GetCurrentThread, GetProcessHandleCount, GetProcessTimes,
                GetThreadTimes,
            },
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                SetThreadDpiAwarenessContext,
            },
            WindowsAndMessaging::*,
        },
    },
    core::{Error, Interface, w},
};

const GPU_FRAMES: usize = 64;

struct Queries {
    disjoint: ID3D11Query,
    start: ID3D11Query,
    end: ID3D11Query,
    ended: Cell<bool>,
}

/// Attached only during the dedicated GPU pass, after all memory/CPU samples.
/// Device brackets its main D2D drawing session, excluding prepass and Present.
pub(super) struct Gpu {
    context: ID3D11DeviceContext,
    queries: Vec<Queries>,
    issued: Cell<usize>,
    active: Cell<Option<usize>>,
    skipped: Cell<usize>,
}

impl Gpu {
    pub(super) fn new(context: &ID3D11DeviceContext) -> Result<Self> {
        // SAFETY: Query resources belong to the production context's device;
        // construction and every query operation stay on its render thread.
        let device = unsafe { context.GetDevice()? };
        let query = |kind: D3D11_QUERY| -> Result<ID3D11Query> {
            let mut value = None;
            // SAFETY: The descriptor and COM output live through CreateQuery.
            unsafe {
                device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: kind,
                        MiscFlags: 0,
                    },
                    Some(&mut value),
                )?;
            }
            Ok(value.unwrap())
        };
        let mut queries = Vec::with_capacity(GPU_FRAMES);
        for _ in 0..GPU_FRAMES {
            queries.push(Queries {
                disjoint: query(D3D11_QUERY_TIMESTAMP_DISJOINT)?,
                start: query(D3D11_QUERY_TIMESTAMP)?,
                end: query(D3D11_QUERY_TIMESTAMP)?,
                ended: Cell::new(false),
            });
        }
        Ok(Self {
            context: context.clone(),
            queries,
            issued: Cell::new(0),
            active: Cell::new(None),
            skipped: Cell::new(0),
        })
    }

    pub(super) fn begin(&self) {
        let index = self.issued.get();
        if index == self.queries.len() {
            self.skipped.set(self.skipped.get() + 1);
            return;
        }
        let queries = &self.queries[index];
        // SAFETY: Device calls once before its main BeginDraw. Timestamp
        // queries use End only, enclosed by this frame's disjoint query.
        unsafe {
            self.context.Begin(&queries.disjoint);
            self.context.End(&queries.start);
        }
        self.active.set(Some(index));
        self.issued.set(index + 1);
    }

    pub(super) fn end(&self) {
        let Some(index) = self.active.take() else {
            return;
        };
        let queries = &self.queries[index];
        // SAFETY: Device calls after successful main EndDraw, before Present;
        // this closes only the query pair opened by begin on this thread.
        unsafe {
            self.context.End(&queries.end);
            self.context.End(&queries.disjoint);
        }
        queries.ended.set(true);
    }

    fn read<T: Default>(&self, query: &ID3D11Query) -> Result<Option<T>> {
        let mut value = T::default();
        // SAFETY: Only u64 and native TIMESTAMP_DISJOINT outputs call this
        // method, with their exact native size. Query inherits Asynchronous.
        // Use the raw HRESULT: windows 0.62's Result wrapper erases S_FALSE.
        let status = unsafe {
            (self.context.vtable().GetData)(
                self.context.as_raw(),
                query.as_raw(),
                (&mut value as *mut T).cast(),
                size_of::<T>() as u32,
                D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
            )
        };
        match status {
            S_OK => Ok(Some(value)),
            S_FALSE => Ok(None),
            _ => Err(Error::from_hresult(status)),
        }
    }

    fn record(&self, phase: &str, run: &Run<'_>, output: &mut impl Write) {
        let (mut pending, mut disjoint, mut invalid, mut incomplete) = (0, 0, 0, 0);
        let mut errors = Vec::new();
        let mut milliseconds = Vec::new();
        for queries in self.queries.iter().take(self.issued.get()) {
            if !queries.ended.get() {
                incomplete += 1;
                continue;
            }
            // One read per query after the batch's natural Present submissions.
            // A pending query stays a gap; no spin, forced Flush or GPU wait.
            let values = (|| -> Result<_> {
                Ok((
                    self.read::<D3D11_QUERY_DATA_TIMESTAMP_DISJOINT>(&queries.disjoint)?,
                    self.read::<u64>(&queries.start)?,
                    self.read::<u64>(&queries.end)?,
                ))
            })();
            match values {
                Ok((Some(clock), Some(start), Some(end))) => {
                    if clock.Disjoint.as_bool() {
                        disjoint += 1;
                    } else if clock.Frequency == 0 || start == 0 || end <= start {
                        invalid += 1;
                    } else {
                        milliseconds.push((end - start) as f64 * 1000.0 / clock.Frequency as f64);
                    }
                }
                Ok(_) => pending += 1,
                Err(error) => errors.push(error.code().0),
            }
        }
        writeln!(output, "{{\"journey\":\"gpu-main-draw\",\"phase\":\"{phase}\",\"scene\":\"{}\",\"target_width\":{},\"target_height\":{},\"zoom\":{},\"gpu_main_draw_ms\":{},\"issued\":{},\"pending\":{pending},\"disjoint\":{disjoint},\"invalid\":{invalid},\"incomplete\":{incomplete},\"skipped\":{},\"error_hresult\":{:?},\"unavailable\":null}}", run.scene.name, run.width, run.height, run.prepared.viewport.zoom, stats(&milliseconds), self.issued.get(), self.skipped.get(), errors).unwrap();
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetGuiResources(process: HANDLE, flags: u32) -> u32;
}

#[derive(Clone, Copy, Default)]
struct Sample {
    private: usize,
    working_set: usize,
    handles: u32,
    gdi: u32,
    user: u32,
}

impl Sample {
    fn read() -> Self {
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let mut handles = 0;
        // SAFETY: The pseudo-handle denotes this process and outputs have the
        // native layouts and sizes. PSAPI accepts the EX record through its
        // common prefix, with cb identifying its full size. No handle transfers.
        unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                memory.cb,
            )
            .unwrap();
            GetProcessHandleCount(GetCurrentProcess(), &mut handles).unwrap();
            Self {
                private: memory.PrivateUsage,
                working_set: memory.WorkingSetSize,
                handles,
                gdi: GetGuiResources(GetCurrentProcess(), 0),
                user: GetGuiResources(GetCurrentProcess(), 1),
            }
        }
    }

    fn include(&mut self, sample: Self) {
        self.private = self.private.max(sample.private);
        self.working_set = self.working_set.max(sample.working_set);
        self.handles = self.handles.max(sample.handles);
        self.gdi = self.gdi.max(sample.gdi);
        self.user = self.user.max(sample.user);
    }
}

fn cpu(thread: bool) -> f64 {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: Pseudo-handles are borrowed; each FILETIME output lives through
    // the call. CPU time is separate from elapsed paint/Present time.
    unsafe {
        if thread {
            GetThreadTimes(
                GetCurrentThread(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
            .unwrap();
        } else {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
            .unwrap();
        }
    }
    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    (ticks(kernel) + ticks(user)) as f64 / 10_000.0
}

struct Window(HWND);

impl Window {
    fn new(parent: Option<HWND>, width: u32, height: u32) -> Self {
        // SAFETY: System STATIC class has no application callback or borrowed
        // state. Parent and child are owned by this same fixture thread.
        Self(unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("NetMap map resource acceptance"),
                WS_VISIBLE | WS_CLIPCHILDREN | if parent.is_some() { WS_CHILD } else { WS_POPUP },
                0,
                0,
                width as i32,
                height as i32,
                parent,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }

    fn extent(&self, width: u32, height: u32) {
        let mut rect = RECT::default();
        // SAFETY: This owned window remains live on its creating thread.
        unsafe {
            GetClientRect(self.0, &mut rect).unwrap();
        }
        assert_eq!(
            (rect.right - rect.left, rect.bottom - rect.top),
            (width as i32, height as i32)
        );
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: Renderer is dropped before its target; child precedes parent.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

struct Dpi(DPI_AWARENESS_CONTEXT);
impl Drop for Dpi {
    fn drop(&mut self) {
        // SAFETY: Restore only this test thread's prior DPI context.
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

fn pump() {
    // SAFETY: Dispatch only the current fixture thread's owned window messages.
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct Scene {
    name: String,
    count: usize,
    active: usize,
    cities: bool,
    rates: Rates,
    destinations: [usize; 2],
    observations: Vec<Observation>,
    locations: Vec<Location>,
    expected: Vec<Expected>,
}

struct Expected {
    width: u32,
    code: String,
    point: Point,
    share: f64,
    rank: usize,
    count: usize,
    active: usize,
    rates: Rates,
    estimated: bool,
    area: String,
    marked: bool,
    pointed: bool,
}

impl Expected {
    fn destination<'a>(&self, model: &'a Model) -> &'a crate::map::model::Destination {
        model
            .destinations
            .iter()
            .find(|destination| {
                model.groups[destination.group].place.code.as_ref() == self.code
                    && destination.rank == self.rank
                    && (destination.point.longitude - self.point.longitude).abs() < 0.00001
                    && (destination.point.latitude - self.point.latitude).abs() < 0.00001
            })
            .expect("A source destination is missing from the native model")
    }
}

struct Fixture {
    origin: Point,
    scale: f64,
    palette: Palette,
    scenes: Vec<Scene>,
}

impl Fixture {
    fn read(path: &Path) -> Self {
        let input = fs::read_to_string(path).unwrap();
        let mut lines = input.lines();
        let header: Vec<_> = lines.next().unwrap().split('\t').collect();
        assert!(matches!(header.len(), 5 | 10));
        let mut fixture = Self {
            origin: Point {
                longitude: header[0].parse().unwrap(),
                latitude: header[1].parse().unwrap(),
            },
            scale: header[2].parse().unwrap(),
            palette: Palette::new(
                header[3] == "dark",
                crate::map::palette::argb(u32::from_str_radix(header[4], 16).unwrap()),
            ),
            scenes: Vec::new(),
        };
        if header.len() == 10 {
            let color = |index: usize| {
                crate::map::palette::argb(u32::from_str_radix(header[index], 16).unwrap())
            };
            fixture.palette.background = color(5);
            fixture.palette.ink = color(6);
            fixture.palette.land = color(7);
            fixture.palette.edge = color(8);
            fixture.palette.upload = color(9);
        }
        assert!((0.5..=4.0).contains(&fixture.scale));
        for line in lines {
            let fields: Vec<_> = line.split('\t').collect();
            match fields[0] {
                "scene" => {
                    assert_eq!(fields.len(), 9);
                    fixture.scenes.push(Scene {
                        name: fields[1].into(),
                        count: fields[2].parse().unwrap(),
                        active: fields[3].parse().unwrap(),
                        cities: fields[8] == "cities",
                        rates: Rates {
                            received: fields[4].parse().unwrap(),
                            sent: fields[5].parse().unwrap(),
                        },
                        destinations: [fields[6].parse().unwrap(), fields[7].parse().unwrap()],
                        observations: Vec::new(),
                        locations: Vec::new(),
                        expected: Vec::new(),
                    });
                    assert!(matches!(fields[8], "countries" | "cities"));
                }
                "peer" => {
                    assert!(matches!(fields.len(), 7 | 9));
                    let scene = fixture.scenes.last_mut().unwrap();
                    scene.observations.push(Observation {
                        id: ObservationId(scene.observations.len() as u64 + 1),
                        tuple: Tuple {
                            process: ProcessId {
                                pid: std::process::id(),
                                generation: 1,
                            },
                            local: Endpoint::new("127.0.0.1:40000".parse().unwrap()),
                            remote: Endpoint::new(fields[1].parse().unwrap()),
                        },
                        life: Life::Tcp(TcpState::Established),
                        introduced: Duration::ZERO,
                        bytes: Bytes::default(),
                        rates: Some(Rates {
                            received: fields[5].parse().unwrap(),
                            sent: fields[6].parse().unwrap(),
                        }),
                    });
                    scene.locations.push(Location {
                        code: fields[2].into(),
                        country: fields.get(7).unwrap_or(&fields[2]).to_string().into(),
                        city: fields
                            .get(8)
                            .filter(|name| !name.is_empty())
                            .map(|name| (*name).into()),
                        point: Some(Point {
                            longitude: fields[3].parse().unwrap(),
                            latitude: fields[4].parse().unwrap(),
                        }),
                    });
                }
                "destination" => {
                    assert!(matches!(fields.len(), 13 | 15));
                    fixture.scenes.last_mut().unwrap().expected.push(Expected {
                        width: fields[1].parse().unwrap(),
                        code: fields[2].into(),
                        point: Point {
                            longitude: fields[3].parse().unwrap(),
                            latitude: fields[4].parse().unwrap(),
                        },
                        share: fields[5].parse().unwrap(),
                        rank: fields[6].parse().unwrap(),
                        count: fields[7].parse().unwrap(),
                        active: fields[8].parse().unwrap(),
                        rates: Rates {
                            received: fields[9].parse().unwrap(),
                            sent: fields[10].parse().unwrap(),
                        },
                        estimated: fields[11].parse().unwrap(),
                        area: fields[12].into(),
                        marked: fields.get(13).is_some_and(|marked| *marked == "true"),
                        pointed: fields.get(14).is_some_and(|pointed| *pointed == "true"),
                    });
                }
                _ => panic!("Unknown fixture record"),
            }
        }
        assert!(!fixture.scenes.is_empty());
        fixture
    }
}

impl Scene {
    fn check_facts(&self) {
        assert_eq!(self.observations.len(), self.count);
        assert_eq!(self.locations.len(), self.count);
        assert_eq!(
            self.observations
                .iter()
                .filter(|observation| observation
                    .rates
                    .is_some_and(|rates| rates.received + rates.sent > 0.0))
                .count(),
            self.active
        );
        assert_eq!(
            self.observations
                .iter()
                .map(|observation| observation.rates.unwrap().received)
                .sum::<f64>(),
            self.rates.received
        );
        assert_eq!(
            self.observations
                .iter()
                .map(|observation| observation.rates.unwrap().sent)
                .sum::<f64>(),
            self.rates.sent
        );
        assert!(
            self.locations
                .iter()
                .all(|location| location.point.is_some())
        );
    }

    fn prepare(
        &self,
        fixture: &Fixture,
        world: &Arc<World>,
        width: u32,
        height: u32,
    ) -> (Prepared, f64, f64) {
        self.check_facts();
        let mut viewport = Viewport::default();
        viewport.resize(
            f64::from(width) / fixture.scale,
            f64::from(height) / fixture.scale,
        );
        viewport.rebase(fixture.origin.longitude as f32);
        let before = cpu(true);
        let started = Instant::now();
        let model = Arc::new(Model::arrange(
            world,
            self.observations
                .iter()
                .zip(&self.locations)
                .map(|(observation, location)| Located {
                    observation,
                    location: Some(location),
                }),
            self.cities,
            viewport.meridian,
            viewport.scale,
        ));
        let preparation_ms = started.elapsed().as_secs_f64() * 1000.0;
        let preparation_cpu_ms = cpu(true) - before;
        let prepared = Prepared {
            viewport,
            world: Some(world.clone()),
            model,
            origin: Some(fixture.origin),
            now: Duration::from_secs(60),
            // Source fixtures keep idle marks visible indefinitely. A long static
            // bright hold reproduces that scene while retaining pulse scheduling.
            highlight: Some(Highlight {
                bright: Duration::from_secs(86_400),
                fade: Duration::ZERO,
            }),
            inspection: Inspection::default(),
            appearance: Appearance {
                palette: fixture.palette,
                ..Default::default()
            },
            speeds: Speeds::default(),
            navigation_bounds: None,
            revision: 0,
        };
        self.check(&prepared, width);
        (prepared, preparation_ms, preparation_cpu_ms)
    }

    fn check(&self, prepared: &Prepared, width: u32) {
        let model = &prepared.model;
        assert_eq!(model.unlocated, 0);
        assert!(
            model
                .destinations
                .iter()
                .all(|destination| model.groups[destination.group].place.cities == self.cities)
        );
        let countries = || model.groups.iter().filter(|group| !group.place.cities);
        assert_eq!(
            countries()
                .map(|group| group.observations.len())
                .sum::<usize>(),
            self.count
        );
        assert_eq!(
            countries().map(|group| group.active).sum::<usize>(),
            self.active
        );
        assert_eq!(
            countries()
                .map(|group| group.rates.unwrap().received)
                .sum::<f64>(),
            self.rates.received
        );
        assert_eq!(
            countries()
                .map(|group| group.rates.unwrap().sent)
                .sum::<f64>(),
            self.rates.sent
        );
        assert_eq!(
            model.destinations.len(),
            self.destinations[usize::from(width == 3840)],
            "Destination distribution differs from the reference for {} at {width}",
            self.name
        );
        assert_eq!(
            self.expected
                .iter()
                .filter(|expected| expected.width == width)
                .count(),
            model.destinations.len()
        );
        for expected in self
            .expected
            .iter()
            .filter(|expected| expected.width == width)
        {
            let destination = expected.destination(model);
            let group = &model.groups[destination.group];
            // Source and port share f32 projection/merging boundaries. These
            // tolerances admit rounding, not a different destination workload.
            assert!((destination.point.longitude - expected.point.longitude).abs() < 0.00001);
            assert!((destination.point.latitude - expected.point.latitude).abs() < 0.00001);
            assert!((destination.share - expected.share).abs() < 0.000001);
            assert_eq!(destination.area.as_deref().unwrap_or(""), expected.area);
            assert_eq!(group.estimated, expected.estimated);
            assert_eq!(group.observations.len(), expected.count);
            assert_eq!(group.active, expected.active);
            assert_eq!(group.rates, Some(expected.rates));
        }
        for index in 0..model.groups.len() {
            if model.groups[index].place.cities != self.cities {
                continue;
            }
            let share: f64 = model
                .destinations
                .iter()
                .filter(|destination| destination.group == index)
                .map(|destination| destination.share)
                .sum();
            assert!(
                (share - 1.0).abs() < 0.000001,
                "Destination allocation lost traffic"
            );
        }
    }
}

#[derive(Default)]
struct Draws {
    unmeasured: bool,
    wall: Vec<f64>,
    cpu: Vec<f64>,
    intervals: Vec<f64>,
    last: Option<Instant>,
    again: usize,
    peak: Sample,
}

impl Draws {
    fn unmeasured() -> Self {
        Self {
            unmeasured: true,
            ..Self::default()
        }
    }

    fn paint(
        &mut self,
        renderer: &mut Renderer,
        prepared: &Prepared,
        width: u32,
        height: u32,
        dpi: u32,
    ) -> Frame {
        if self.unmeasured {
            return renderer.paint(prepared, width, height, dpi).unwrap();
        }
        let start = Instant::now();
        if let Some(last) = self.last.replace(start) {
            self.intervals
                .push(start.duration_since(last).as_secs_f64() * 1000.0);
        }
        let before = cpu(true);
        let frame = renderer.paint(prepared, width, height, dpi).unwrap();
        self.cpu.push(cpu(true) - before);
        self.wall.push(start.elapsed().as_secs_f64() * 1000.0);
        self.peak.include(Sample::read());
        self.again += usize::from(matches!(frame, Frame::Again));
        frame
    }

    fn summary(
        &self,
        phase: &str,
        scene: &Scene,
        width: u32,
        height: u32,
        output: &mut impl Write,
    ) {
        writeln!(output, "{{\"journey\":\"{phase}\",\"scene\":\"{}\",\"target_width\":{width},\"target_height\":{height},\"draws\":{},\"pending_rebuild_frames\":{},\"paint_elapsed_ms\":{},\"paint_thread_cpu_ms\":{},\"draw_start_intervals_ms\":{},\"peak_private_bytes\":{},\"peak_working_set\":{}}}", scene.name, self.wall.len(), self.again, stats(&self.wall), stats(&self.cpu), stats(&self.intervals), self.peak.private, self.peak.working_set).unwrap();
    }
}

fn stats(values: &[f64]) -> String {
    if values.is_empty() {
        return "null".into();
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    format!(
        "{{\"count\":{},\"median\":{},\"p95\":{},\"worst\":{},\"samples\":{:?}}}",
        ordered.len(),
        ordered[ordered.len() / 2],
        ordered[(ordered.len() * 95).div_ceil(100).saturating_sub(1)],
        ordered.last().unwrap(),
        values
    )
}

struct Run<'a> {
    scene: &'a Scene,
    window: &'a Window,
    prepared: Prepared,
    width: u32,
    height: u32,
    dpi: u32,
    started: Instant,
}

impl Run<'_> {
    fn draw(&mut self, renderer: &mut Renderer, draws: &mut Draws) -> Frame {
        self.prepared.now = Duration::from_secs(60) + self.started.elapsed();
        draws.paint(renderer, &self.prepared, self.width, self.height, self.dpi)
    }

    fn settle(&mut self, renderer: &mut Renderer, draws: &mut Draws) -> Frame {
        let deadline = Instant::now() + Duration::from_secs(20);
        // Both production interpolation clocks use 1.6 seconds. Pending rebuilds
        // must independently finish; active pulse scheduling remains permitted.
        let warm = Instant::now()
            + if renderer.outlines.is_none() {
                Duration::from_millis(1700)
            } else {
                Duration::ZERO
            };
        loop {
            pump();
            let frame = self.draw(renderer, draws);
            if Instant::now() >= warm && !matches!(frame, Frame::Again) {
                self.check(renderer);
                return frame;
            }
            assert!(
                Instant::now() < deadline,
                "Bounded native map warmup timed out: scene={} target={}x{} zoom={} draws={} pending_rebuild_frames={}",
                self.scene.name,
                self.width,
                self.height,
                self.prepared.viewport.zoom,
                draws.wall.len(),
                draws.again
            );
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn check(&self, renderer: &Renderer) {
        self.scene.check(&self.prepared, self.width);
        self.window.extent(self.width, self.height);
        assert!(
            (f64::from(self.prepared.viewport.size.X) * f64::from(self.dpi) / 96.0
                - f64::from(self.width))
            .abs()
                < 0.1
        );
        assert!(
            (f64::from(self.prepared.viewport.size.Y) * f64::from(self.dpi) / 96.0
                - f64::from(self.height))
            .abs()
                < 0.1
        );
        check_target(renderer.device.as_ref().unwrap(), self.width, self.height);
    }

    fn sample(
        &mut self,
        mut renderer: Option<&mut Renderer>,
        frame: Frame,
        cycle: usize,
        id: usize,
        directory: &Path,
        output: &mut impl Write,
    ) {
        self.scene.check(&self.prepared, self.width);
        let visible = renderer.is_some();
        if let Some(renderer) = renderer.as_deref() {
            self.check(renderer);
        }
        let mut next = due(frame);
        let started = Instant::now();
        let before = cpu(false);
        let mut draws = Draws::default();
        while started.elapsed() < Duration::from_secs(1) {
            self.scheduled(&mut renderer, &mut next, &mut draws);
        }
        let sample = Sample::read();
        draws.peak.include(sample);
        if visible && self.scene.active == 0 {
            assert!(
                draws.wall.is_empty(),
                "Static scene scheduled recurring draws"
            );
        }
        if visible && self.scene.active > 0 {
            assert!(
                !draws.wall.is_empty(),
                "Measured traffic failed to schedule pulse frames"
            );
        }
        let record = format!(
            "{{\"journey\":\"netmap-map-baseline\",\"sample_id\":{id},\"process_id\":{},\"scene\":\"{}\",\"cycle\":{cycle},\"visible\":{visible},\"connections\":{},\"active\":{},\"Down\":{},\"Up\":{},\"target_width\":{},\"target_height\":{},\"map_width\":{},\"map_height\":{},\"rasterization_scale\":{},\"destinations\":{},\"private_bytes\":{},\"working_set\":{},\"handles\":{},\"gdi_objects\":{},\"user_objects\":{},\"sample_cpu_ms\":{},\"sample_elapsed_ms\":{},\"sampled_peak_private_bytes\":{},\"sampled_peak_working_set\":{},\"scheduled_draws\":{}}}",
            std::process::id(),
            self.scene.name,
            self.scene.count,
            self.scene.active,
            self.scene.rates.received,
            self.scene.rates.sent,
            self.width,
            self.height,
            self.prepared.viewport.size.X,
            self.prepared.viewport.size.Y,
            f64::from(self.dpi) / 96.0,
            self.prepared.model.destinations.len(),
            sample.private,
            sample.working_set,
            sample.handles,
            sample.gdi,
            sample.user,
            cpu(false) - before,
            started.elapsed().as_secs_f64() * 1000.0,
            draws.peak.private,
            draws.peak.working_set,
            draws.wall.len()
        );
        writeln!(output, "{record}").unwrap();
        output.flush().unwrap();
        let temporary = directory.join("baseline-phase.tmp");
        fs::write(&temporary, &record).unwrap();
        let checkpoint = directory.join("baseline-phase.json");
        // The reader shares delete access. Remove only our preceding checkpoint.
        if checkpoint.exists() {
            fs::remove_file(&checkpoint).unwrap();
        }
        fs::rename(temporary, checkpoint).unwrap();
        let acknowledgement = directory.join(format!("baseline-phase-{id}.continue"));
        let deadline = Instant::now() + Duration::from_secs(20);
        while !acknowledgement.exists() {
            assert!(
                Instant::now() < deadline,
                "GPU checkpoint acknowledgement timed out"
            );
            self.scheduled(&mut renderer, &mut next, &mut Draws::default());
        }
        if let Some(renderer) = renderer {
            self.check(renderer);
        }
    }

    fn scheduled(
        &mut self,
        renderer: &mut Option<&mut Renderer>,
        next: &mut Option<Instant>,
        draws: &mut Draws,
    ) {
        pump();
        if let Some(renderer) = renderer.as_deref_mut()
            && next.is_some_and(|time| Instant::now() >= time)
        {
            *next = due(self.draw(renderer, draws));
        }
        let delay = next.map_or(Duration::from_millis(50), |time| {
            time.saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50))
        });
        std::thread::sleep(delay.max(Duration::from_millis(1)));
    }

    fn timings(&mut self, renderer: &mut Renderer, output: &mut impl Write) {
        let mut redraw = Draws::default();
        for _ in 0..30 {
            pump();
            self.draw(renderer, &mut redraw);
            std::thread::sleep(Duration::from_millis(16));
        }
        redraw.summary(
            "settled-redraw",
            self.scene,
            self.width,
            self.height,
            output,
        );
        let fitted = self.prepared.viewport;
        let focus = fitted.coordinates(Point {
            longitude: 10.0,
            latitude: 50.0,
        });
        for zoom in [2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0] {
            self.prepared.viewport.hold(zoom, focus, fitted.offset);
            let mut draws = Draws::default();
            self.settle(renderer, &mut draws);
            draws.summary(
                &format!("Europe-zoom-{zoom}"),
                self.scene,
                self.width,
                self.height,
                output,
            );
        }
        self.prepared.viewport.hold(8.0, focus, fitted.offset);
        self.settle(renderer, &mut Draws::default());
        let mut drag = Draws::default();
        for step in 1..=60 {
            self.prepared.viewport.hold(
                8.0,
                focus,
                fitted.offset + Vector2::new(step as f32 * 4.0, step as f32),
            );
            pump();
            self.draw(renderer, &mut drag);
            std::thread::sleep(Duration::from_millis(16));
        }
        drag.summary(
            "Europe-drag-60",
            self.scene,
            self.width,
            self.height,
            output,
        );
        self.prepared.viewport.fit();
        self.settle(renderer, &mut Draws::default());
    }

    fn gpu_batch(
        &mut self,
        renderer: &mut Renderer,
        phase: &str,
        drag: bool,
        output: &mut impl Write,
    ) {
        let device = renderer.device.as_mut().unwrap();
        if device.mode != "Hardware D3D11" {
            self.gpu_unavailable(phase, "software_renderer", output);
            return;
        }
        match Gpu::new(device.immediate()) {
            Ok(gpu) => device.gpu = Some(gpu),
            Err(error) => {
                self.gpu_unavailable(
                    phase,
                    &format!("query_creation_hresult_{:08x}", error.code().0 as u32),
                    output,
                );
                return;
            }
        }
        let focus = self.prepared.viewport.coordinates(Point {
            longitude: 10.0,
            latitude: 50.0,
        });
        let offset = self.prepared.viewport.offset;
        for step in 1..=if drag { 60 } else { 30 } {
            if drag {
                self.prepared.viewport.hold(
                    8.0,
                    focus,
                    offset + Vector2::new(step as f32 * 4.0, step as f32),
                );
            }
            pump();
            self.draw(renderer, &mut Draws::default());
            std::thread::sleep(Duration::from_millis(16));
        }
        if let Some(gpu) = renderer
            .device
            .as_mut()
            .and_then(|device| device.gpu.take())
        {
            gpu.record(phase, self, output);
        } else {
            self.gpu_unavailable(phase, "device_recreated_during_batch", output);
        }
    }

    fn gpu_unavailable(&self, phase: &str, reason: &str, output: &mut impl Write) {
        writeln!(output, "{{\"journey\":\"gpu-main-draw\",\"phase\":\"{phase}\",\"scene\":\"{}\",\"target_width\":{},\"target_height\":{},\"zoom\":{},\"gpu_main_draw_ms\":null,\"issued\":0,\"pending\":0,\"disjoint\":0,\"invalid\":0,\"incomplete\":0,\"skipped\":0,\"error_hresult\":[],\"unavailable\":\"{reason}\"}}", self.scene.name, self.width, self.height, self.prepared.viewport.zoom).unwrap();
    }

    fn gpu_timings(&mut self, renderer: &mut Renderer, output: &mut impl Write) {
        let fitted = self.prepared.viewport;
        self.gpu_batch(renderer, "settled-redraw", false, output);
        let focus = fitted.coordinates(Point {
            longitude: 10.0,
            latitude: 50.0,
        });
        for zoom in [2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0] {
            self.prepared.viewport.hold(zoom, focus, fitted.offset);
            self.settle(renderer, &mut Draws::default());
            self.gpu_batch(renderer, "settled-Europe-zoom", false, output);
        }
        self.prepared.viewport.hold(8.0, focus, fitted.offset);
        self.settle(renderer, &mut Draws::default());
        self.gpu_batch(renderer, "Europe-drag-60", true, output);
        self.prepared.viewport.fit();
        self.settle(renderer, &mut Draws::default());
    }
}

fn due(frame: Frame) -> Option<Instant> {
    match frame {
        Frame::Idle => None,
        Frame::Again => Some(Instant::now()),
        Frame::After(delay) => Some(Instant::now() + delay),
    }
}

fn image(renderer: &Renderer, path: &Path) {
    readback(&renderer.device.as_ref().unwrap().context, path).unwrap();
}

pub(super) fn readback(context: &ID2D1DeviceContext3, path: &Path) -> Result<()> {
    // SAFETY: Drawing has ended. The local staging surface is unmapped before
    // release, including file errors. The dynamic fixture requests this before
    // Present; legacy static fixtures call it only in their final image pass.
    unsafe {
        let target: ID2D1Bitmap1 = context.GetTarget()?.cast()?;
        let size = target.GetPixelSize();
        let staging = context.CreateBitmap(
            size,
            None,
            0,
            &D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            },
        )?;
        staging.CopyFromBitmap(None, &target, None)?;
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;
        let written = (|| -> std::io::Result<()> {
            let mut file = BufWriter::new(File::create_new(path)?);
            let bytes = size.width * size.height * 4;
            file.write_all(b"BM")?;
            file.write_all(&(54 + bytes).to_le_bytes())?;
            file.write_all(&[0; 4])?;
            file.write_all(&54u32.to_le_bytes())?;
            file.write_all(&40u32.to_le_bytes())?;
            file.write_all(&size.width.to_le_bytes())?;
            file.write_all(&(-(size.height as i32)).to_le_bytes())?;
            file.write_all(&1u16.to_le_bytes())?;
            file.write_all(&32u16.to_le_bytes())?;
            file.write_all(&[0; 24])?;
            for row in 0..size.height as usize {
                file.write_all(std::slice::from_raw_parts(
                    mapped.bits.add(row * mapped.pitch as usize),
                    size.width as usize * 4,
                ))?;
            }
            file.flush()
        })();
        let unmapped = staging.Unmap();
        written
            .map_err(|error| Error::new(windows::Win32::Foundation::E_FAIL, error.to_string()))?;
        unmapped?;
    }
    Ok(())
}

#[test]
#[ignore = "Release-only matched native map measurements; use tests/native-map-resources.ps1"]
fn native_matched_map_resources() {
    if cfg!(debug_assertions) {
        panic!("Resource acceptance requires an optimized Release test executable");
    }
    let directory = PathBuf::from(
        std::env::var_os("NETMAP_MAP_RESOURCE_DIRECTORY").expect("Use native-map-resources.ps1"),
    );
    if std::env::var("NETMAP_MAP_RESOURCE_VISUAL_FAMILIES").as_deref() == Ok("1") {
        visual_images(&directory);
        return;
    }
    let gpu_only = std::env::var("NETMAP_MAP_RESOURCE_GPU_ONLY").as_deref() == Ok("1");
    let fixture = Fixture::read(&directory.join("fixture.tsv"));
    let dpi = (fixture.scale * 96.0).round() as u32;
    assert!(
        (f64::from(dpi) / 96.0 - fixture.scale).abs() < 0.000001,
        "Reference scale is not representable by the production integer DPI contract"
    );
    // SAFETY: Affects only this test thread and is restored by Dpi on return.
    let dpi_context =
        Dpi(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) });
    assert!(!dpi_context.0.0.is_null());
    if let Some(arm) = std::env::var("NETMAP_MAP_RESOURCE_COLD_ARM")
        .ok()
        .filter(|arm| !arm.is_empty())
    {
        cold_contribution(&directory, &fixture, dpi, &arm);
        return;
    }
    let world = Arc::new(
        World::decode(
            &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world.bin")).unwrap(),
        )
        .unwrap(),
    );
    let mut output =
        BufWriter::new(File::create_new(directory.join("measurements.jsonl")).unwrap());
    let parent = Window::new(None, 1280, 900);
    let mut sample = 0;
    // Query resources and image readbacks cannot pollute baseline memory/CPU.
    enum Pass {
        Baseline,
        Gpu,
        Images,
    }
    for pass in [Pass::Baseline, Pass::Gpu, Pass::Images] {
        if gpu_only && !matches!(pass, Pass::Gpu) {
            continue;
        }
        for (width, height) in [(1920, 1080), (3840, 2160)] {
            let window = Window::new(Some(parent.0), width, height);
            for scene in &fixture.scenes {
                let (prepared, preparation_ms, preparation_cpu_ms) =
                    scene.prepare(&fixture, &world, width, height);
                let mut run = Run {
                    scene,
                    window: &window,
                    prepared,
                    width,
                    height,
                    dpi,
                    started: Instant::now(),
                };
                let mut renderer = Some(Renderer::new(window.0).unwrap());
                let mut warmup = Draws::default();
                run.settle(renderer.as_mut().unwrap(), &mut warmup);
                if matches!(pass, Pass::Gpu) {
                    run.gpu_timings(renderer.as_mut().unwrap(), &mut output);
                    continue;
                }
                if matches!(pass, Pass::Images) {
                    run.draw(renderer.as_mut().unwrap(), &mut Draws::default());
                    run.draw(renderer.as_mut().unwrap(), &mut Draws::default());
                    image(
                        renderer.as_ref().unwrap(),
                        &directory.join(format!("netmap-{width}x{height}-{}.bmp", scene.name)),
                    );
                    continue;
                }
                writeln!(output, "{{\"journey\":\"scene-preparation\",\"scene\":\"{}\",\"target_width\":{width},\"target_height\":{height},\"model_preparation_elapsed_ms\":{preparation_ms},\"model_preparation_thread_cpu_ms\":{preparation_cpu_ms},\"graphics_mode\":\"{}\"}}", scene.name, renderer.as_ref().unwrap().graphics_mode()).unwrap();
                for cycle in 0..3 {
                    drop(renderer.take());
                    pump();
                    sample += 1;
                    run.sample(None, Frame::Idle, cycle, sample, &directory, &mut output);
                    let mut draws = Draws::default();
                    draws.peak.include(Sample::read());
                    renderer = Some(Renderer::new(window.0).unwrap());
                    let frame = run.settle(renderer.as_mut().unwrap(), &mut draws);
                    draws.summary(
                        &format!("warmup-cycle-{cycle}"),
                        scene,
                        width,
                        height,
                        &mut output,
                    );
                    sample += 1;
                    run.sample(
                        renderer.as_mut(),
                        frame,
                        cycle,
                        sample,
                        &directory,
                        &mut output,
                    );
                    if cycle == 0 {
                        run.timings(renderer.as_mut().unwrap(), &mut output);
                    }
                }
            }
        }
    }
    output.flush().unwrap();
    fs::write(directory.join("complete.txt"), format!("{sample}\n")).unwrap();
}

fn visual_images(directory: &Path) {
    let world = Arc::new(
        World::decode(
            &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world.bin")).unwrap(),
        )
        .unwrap(),
    );
    let mut output =
        BufWriter::new(File::create_new(directory.join("measurements.jsonl")).unwrap());
    let cases = fs::read_to_string(directory.join("visual-cases.tsv")).unwrap();
    for line in cases.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 22);
        let id = fields[0];
        let width = fields[1].parse().unwrap();
        let height = fields[2].parse().unwrap();
        let dpi = fields[3].parse().unwrap();
        let value = |index: usize| fields[index].parse::<f32>().unwrap();
        let fixture = Fixture::read(&directory.join(format!("visual-{id}.tsv")));
        assert_eq!(fixture.scenes.len(), 1);
        assert!((f64::from(dpi) / 96.0 - fixture.scale).abs() < 0.000001);
        // SAFETY: This test thread owns all windows; Dpi restores its context.
        let _dpi = Dpi(unsafe {
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        });
        let parent = Window::new(None, 1280, 900);
        let window = Window::new(Some(parent.0), width, height);
        let scene = &fixture.scenes[0];
        let (mut prepared, _, _) = scene.prepare(&fixture, &world, width, height);
        assert!((prepared.viewport.meridian - value(4)).abs() < 0.00001);
        assert!((prepared.viewport.scale - value(7)).abs() < 0.00001);
        assert!((prepared.viewport.offset.X - value(8)).abs() < 0.00001);
        assert!((prepared.viewport.offset.Y - value(9)).abs() < 0.00001);
        assert!((prepared.viewport.size.X - value(12)).abs() < 0.00001);
        assert!((prepared.viewport.size.Y - value(13)).abs() < 0.00001);
        // Actual source view is authoritative; do not independently place a golden.
        prepared.viewport.meridian = value(4);
        prepared.viewport.zoom = value(5);
        prepared.viewport.ratio = value(6);
        prepared.viewport.scale = value(7);
        prepared.viewport.offset = windows_numerics::Vector2::new(value(8), value(9));
        prepared.viewport.center = windows_numerics::Vector2::new(value(10), value(11));
        prepared.viewport.size = windows_numerics::Vector2::new(value(12), value(13));
        prepared.appearance.animations = fields[14].parse().unwrap();
        if let Some(expected) = scene.expected.iter().find(|expected| expected.marked) {
            let destination = expected.destination(&prepared.model);
            prepared.inspection.selected =
                Some(prepared.model.groups[destination.group].place.clone());
        }
        if let Some(expected) = scene.expected.iter().find(|expected| expected.pointed) {
            prepared.inspection.pointed = Some(expected.destination(&prepared.model).key.clone());
        }
        for expected in &scene.expected {
            let destination = expected.destination(&prepared.model);
            assert_eq!(
                prepared.inspection.selected.as_ref()
                    == Some(&prepared.model.groups[destination.group].place),
                expected.marked
            );
            assert_eq!(
                prepared.inspection.pointed.as_ref() == Some(&destination.key),
                expected.pointed
            );
        }
        let mut run = Run {
            scene,
            window: &window,
            prepared,
            width,
            height,
            dpi,
            started: Instant::now(),
        };
        let mut renderer = Renderer::new(window.0).unwrap();
        let mut draws = Draws::unmeasured();
        run.settle(&mut renderer, &mut draws);
        run.draw(&mut renderer, &mut draws);
        run.draw(&mut renderer, &mut draws);
        run.check(&renderer);
        image(&renderer, &directory.join(format!("native-{id}.bmp")));
        writeln!(output, "{{\"journey\":\"visual-image\",\"schema\":1,\"id\":\"{id}\",\"file\":\"native-{id}.bmp\",\"width\":{width},\"height\":{height},\"dpi\":{dpi},\"theme\":\"{}\",\"detail\":\"{}\",\"meridian\":{},\"zoom\":{},\"ratio\":{},\"center_x\":{},\"center_y\":{},\"requested_center_x\":{},\"requested_center_y\":{},\"full_turns\":{},\"destinations\":{},\"graphics_mode\":\"{}\",\"native_clock_seconds\":60,\"bright_seconds\":86400,\"fade_seconds\":0,\"scope\":\"Full production renderer including default Speeds; imported source palette/viewport/destinations/highlight; native establishment marks differ from source idle roles; no pixel threshold\"}}", fields[20], fields[21], value(4), value(5), value(6), value(10), value(11), value(15), value(16), fields[17], run.prepared.model.destinations.len(), renderer.graphics_mode()).unwrap();
        output.flush().unwrap();
    }
    fs::write(directory.join("complete.txt"), "0\n").unwrap();
}

fn utc_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn process_created_ms() -> u128 {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: Borrowed current-process pseudo-handle and live output records.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
        .unwrap();
    }
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    u128::from(ticks / 10_000 - 11_644_473_600_000)
}

fn check_target(device: &device::Device, width: u32, height: u32) {
    // SAFETY: Query the live production target after EndDraw, without
    // allocating any readback surface during memory measurements.
    unsafe {
        let bitmap: ID2D1Bitmap1 = device.context.GetTarget().unwrap().cast().unwrap();
        assert_eq!(
            (bitmap.GetPixelSize().width, bitmap.GetPixelSize().height),
            (width, height)
        );
    }
}

enum Arm<'a> {
    Control,
    Presentation(Box<device::Device>),
    Text(Box<(device::Device, speeds::Speeds, Prepared)>),
    Treatment(Box<(Run<'a>, Renderer, Option<Instant>)>),
}

impl Arm<'_> {
    fn scheduled(&mut self, draws: &mut Draws) {
        match self {
            Self::Treatment(treatment) => {
                let (run, renderer, next) = treatment.as_mut();
                run.scheduled(&mut Some(renderer), next, draws);
            }
            Self::Control | Self::Presentation(_) | Self::Text(_) => {
                pump();
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn check(&self) {
        match self {
            Self::Control => {}
            Self::Presentation(device) => check_target(device, 3840, 2160),
            Self::Text(text) => {
                let (device, _, prepared) = text.as_ref();
                check_target(device, 3840, 2160);
                assert!(prepared.world.is_none());
                assert!(prepared.model.groups.is_empty());
                assert!(prepared.model.destinations.is_empty());
                assert!(prepared.speeds == Speeds::default());
                assert!(prepared.navigation_bounds.is_none());
            }
            Self::Treatment(treatment) => {
                let (run, renderer, _) = treatment.as_ref();
                run.check(renderer);
            }
        }
    }

    fn graphics_mode(&self) -> Option<&'static str> {
        match self {
            Self::Control => None,
            Self::Presentation(device) => Some(device.mode),
            Self::Text(text) => Some(text.0.mode),
            Self::Treatment(treatment) => Some(treatment.1.graphics_mode()),
        }
    }
}

fn cold_contribution(directory: &Path, fixture: &Fixture, dpi: u32, arm: &str) {
    assert!(matches!(
        arm,
        "control" | "presentation" | "text" | "treatment"
    ));
    assert_eq!(fixture.scenes.len(), 1);
    let scene = &fixture.scenes[0];
    assert_eq!(
        (scene.name.as_str(), scene.count, scene.active),
        ("active-100", 100, 100)
    );
    assert_eq!(
        (scene.rates.received, scene.rates.sent),
        (12_943_360.0, 7_372_800.0)
    );
    assert_eq!(scene.destinations[1], 44);
    assert_eq!(scene.observations.len(), 100);
    assert_eq!(scene.locations.len(), 100);
    let mut output =
        BufWriter::new(File::create_new(directory.join("measurements.jsonl")).unwrap());
    let created = process_created_ms();
    let parent = Window::new(None, 1280, 900);
    let window = Window::new(Some(parent.0), 3840, 2160);
    window.extent(3840, 2160);
    pump();
    let common = Instant::now();
    let common_utc = utc_ms();
    writeln!(output, "{{\"journey\":\"map-contribution-setup\",\"schema\":1,\"arm\":\"{arm}\",\"process_id\":{},\"time_utc_unix_ms\":{common_utc},\"process_age_ms\":{},\"scene\":\"active-100\",\"imported_scene\":\"active-100\",\"scene_arranged\":false,\"target_width\":3840,\"target_height\":2160}}", std::process::id(), common_utc.saturating_sub(created)).unwrap();
    output.flush().unwrap();
    // The split is before world decoding, model arrangement or device creation.
    // The retained imported facts and native target windows are common to both arms.
    let mut draws = Draws::unmeasured();
    let mut resources = match arm {
        "control" => Arm::Control,
        "presentation" | "text" => {
            let mut device = device::Device::new().unwrap();
            device.attach(window.0).unwrap();
            device.select(window.0, 3840, 2160, dpi).unwrap();
            let appearance = Appearance {
                palette: fixture.palette,
                ..Default::default()
            };
            device.apply_appearance(appearance);
            if arm == "text" {
                let prepared = {
                    let map = crate::map::Map::default();
                    map.resize(3840, 2160, dpi);
                    map.set_appearance(appearance);
                    map.prepared(Inspection::default())
                };
                let mut text = speeds::Speeds::new(&device).unwrap();
                text.prepare(&prepared).unwrap();
                for _ in 0..2 {
                    device.begin(appearance);
                    text.draw(&device, &prepared);
                    device.end().unwrap();
                }
                check_target(&device, 3840, 2160);
                Arm::Text(Box::new((device, text, prepared)))
            } else {
                for _ in 0..2 {
                    device.begin(appearance);
                    device.end().unwrap();
                }
                check_target(&device, 3840, 2160);
                Arm::Presentation(Box::new(device))
            }
        }
        "treatment" => {
            let world = Arc::new(
                World::decode(
                    &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world.bin"))
                        .unwrap(),
                )
                .unwrap(),
            );
            let (prepared, _, _) = scene.prepare(fixture, &world, 3840, 2160);
            let mut run = Run {
                scene,
                window: &window,
                prepared,
                width: 3840,
                height: 2160,
                dpi,
                started: Instant::now(),
            };
            let mut renderer = Renderer::new(window.0).unwrap();
            let frame = run.settle(&mut renderer, &mut draws);
            Arm::Treatment(Box::new((run, renderer, due(frame))))
        }
        _ => unreachable!(),
    };
    let ready = (!matches!(&resources, Arm::Control)).then(|| common.elapsed());
    assert!(
        ready.is_none_or(|age| age < Duration::from_secs(30)),
        "Cold arm was not ready before the first checkpoint"
    );
    let scene_arranged = matches!(&resources, Arm::Treatment(_));
    let (scene_json, content, blank_presents, clock, bright, fade, clock_policy, scope) =
        match &resources {
            Arm::Text(_) => (
                "null",
                "device-presentation-and-production-speeds",
                0,
                "null",
                "null",
                "null",
                "Not applicable; empty-map Prepared only for production Speeds; no arranged scene",
                "Fresh whole process; production device, default Speeds and empty Prepared retained after two text Presents; no world/arrangement/map geometry/routes/effects",
            ),
            Arm::Presentation(_) => (
                "null",
                "blank-device-presentation",
                2,
                "null",
                "null",
                "null",
                "Not applicable; blank device/target, no scene or text",
                "Fresh whole process; production device and target retained after two blank Presents; no world/model/renderer/routes/outlines/DirectWrite",
            ),
            Arm::Control | Arm::Treatment(_) => (
                "\"active-100\"",
                if scene_arranged {
                    "production-map"
                } else {
                    "imported-facts-and-host"
                },
                0,
                "60",
                "86400",
                "0",
                "60 seconds plus preparation age; native directional pulses; accepted fixture hold, not default product steady state",
                "Fresh whole process; imported facts and host common; world/model/renderer only in treatment; no forced GC, navigation, queries or readbacks",
            ),
        };
    let (text_content, text_presents) = match &resources {
        Arm::Text(_) => ("[\"Speed\",\"Traffic unavailable\"]", "2"),
        Arm::Treatment(_) => ("[\"Speed\",\"Traffic unavailable\"]", "null"),
        Arm::Control | Arm::Presentation(_) => ("null", "0"),
    };
    let graphics_mode = resources
        .graphics_mode()
        .map_or_else(|| "null".into(), |mode| format!("\"{mode}\""));
    let mut before_cpu = cpu(false);
    let mut before_age = common.elapsed();
    for (index, seconds) in [30_u64, 40, 50].into_iter().enumerate() {
        while common.elapsed() < Duration::from_secs(seconds) {
            resources.scheduled(&mut draws);
        }
        scene.check_facts();
        resources.check();
        window.extent(3840, 2160);
        let sample = Sample::read();
        let age = common.elapsed();
        let time = utc_ms();
        let ready_age = ready.map_or_else(
            || "null".into(),
            |ready| ((age - ready).as_secs_f64() * 1000.0).to_string(),
        );
        let ready_at = ready.map_or_else(
            || "null".into(),
            |ready| (ready.as_secs_f64() * 1000.0).to_string(),
        );
        let after_cpu = cpu(false);
        let id = index + 1;
        let cpu_start = before_age.as_secs_f64() * 1000.0;
        let record = format!(
            "{{\"journey\":\"map-contribution\",\"schema\":1,\"application\":\"netmap\",\"arm\":\"{arm}\",\"sample_id\":{id},\"process_id\":{},\"time_utc_unix_ms\":{time},\"process_age_ms\":{},\"common_age_ms\":{},\"checkpoint_seconds\":{seconds},\"lateness_ms\":{},\"ready_common_age_ms\":{ready_at},\"readiness_age_ms\":{ready_age},\"ready\":{},\"scene\":{scene_json},\"imported_scene\":\"active-100\",\"scene_arranged\":{scene_arranged},\"arm_content\":\"{content}\",\"graphics_mode\":{graphics_mode},\"blank_presents\":{blank_presents},\"text_content\":{text_content},\"text_presents\":{text_presents},\"cycle\":0,\"visible\":{},\"connections\":100,\"active\":100,\"Down\":12943360,\"Up\":7372800,\"target_width\":3840,\"target_height\":2160,\"rasterization_scale\":{},\"destinations\":{},\"private_bytes\":{},\"working_set\":{},\"handles\":{},\"gdi_objects\":{},\"user_objects\":{},\"sample_cpu_ms\":{},\"sample_elapsed_ms\":{},\"cpu_window_start_common_age_ms\":{cpu_start},\"scene_clock_seconds\":{clock},\"highlight_bright_seconds\":{bright},\"highlight_fade_seconds\":{fade},\"scene_clock_policy\":\"{clock_policy}\",\"scope\":\"{scope}\"}}",
            std::process::id(),
            time.saturating_sub(created),
            age.as_secs_f64() * 1000.0,
            age.saturating_sub(Duration::from_secs(seconds))
                .as_secs_f64()
                * 1000.0,
            ready.is_some(),
            ready.is_some(),
            fixture.scale,
            if matches!(&resources, Arm::Treatment(_)) {
                "44"
            } else {
                "null"
            },
            sample.private,
            sample.working_set,
            sample.handles,
            sample.gdi,
            sample.user,
            after_cpu - before_cpu,
            (age - before_age).as_secs_f64() * 1000.0
        );
        writeln!(output, "{record}").unwrap();
        output.flush().unwrap();
        let checkpoint = directory.join("baseline-phase.json");
        let temporary = directory.join("baseline-phase.tmp");
        fs::write(&temporary, record).unwrap();
        if checkpoint.exists() {
            fs::remove_file(&checkpoint).unwrap();
        }
        fs::rename(temporary, checkpoint).unwrap();
        let acknowledgement = directory.join(format!("baseline-phase-{id}.continue"));
        let deadline = Instant::now() + Duration::from_secs(20);
        while !acknowledgement.exists() {
            assert!(
                Instant::now() < deadline,
                "GPU allocation checkpoint acknowledgement timed out"
            );
            resources.scheduled(&mut draws);
        }
        before_cpu = cpu(false);
        before_age = common.elapsed();
    }
    fs::write(directory.join("complete.txt"), "3\n").unwrap();
}
