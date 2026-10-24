//! Bounded production-path map journeys, absent from product builds.

use super::*;
use crate::{
    app::native_checks as facts,
    connections::{Highlight, Life, Observation, ObservationId, Rates, Snapshot, TcpState},
    geography::Point,
    geolocation::Location,
    map::{Command, Frame, Input, NativeRendering, Presentations, Update, model::Model},
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::Win32::{
    Graphics::{Direct2D::Common::D2D1_COLOR_F, Gdi::UpdateWindow},
    System::SystemServices::MK_LBUTTON,
    UI::Input::KeyboardAndMouse::GetCapture,
};
use windows_numerics::Vector2;

#[derive(Clone, Copy)]
enum Case {
    Navigation,
    Temporal,
}

#[derive(Clone, Copy)]
enum Wake {
    Idle,
    Again,
    After(Duration),
}

thread_local! {
    static REQUESTED: Cell<Option<Case>> = const { Cell::new(None) };
    static WAKE: Cell<Option<Wake>> = const { Cell::new(None) };
    static CAPTURE: Cell<u64> = const { Cell::new(0) };
}

pub(super) fn requested() -> bool {
    REQUESTED.get().is_some()
}

pub(super) fn capture_requested(id: u64) {
    if requested() {
        CAPTURE.set(id);
    }
}

// This is the paint result before Host applies its ordinary EndPaint/timer
// handling. Presentations independently observes successful actual Presents.
pub(super) fn painted(result: &Result<Frame>) {
    if requested()
        && let Ok(frame) = result
    {
        WAKE.set(Some(match frame {
            Frame::Idle => Wake::Idle,
            Frame::Again => Wake::Again,
            Frame::After(delay) => Wake::After(*delay),
        }));
    }
}

#[test]
#[ignore = "Owned Host navigation, capture, accepted facts and native images; use native-map-dynamics.ps1"]
fn native_map_navigation_updates_and_rebases_during_capture() -> Result<()> {
    run(Case::Navigation)
}

#[test]
#[ignore = "Waits through real default 10/20-second map highlight ages; use native-map-dynamics.ps1"]
fn native_map_temporal_policy_and_retained_resources() -> Result<()> {
    run(Case::Temporal)
}

fn run(case: Case) -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(None);
            WAKE.set(None);
            CAPTURE.set(0);
        }
    }
    if cfg!(debug_assertions) {
        return Err(failure(
            "Dynamic acceptance requires an optimized test executable",
        ));
    }
    REQUESTED.set(Some(case));
    let _scope = Scope;
    facts::run()
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = match REQUESTED.get() {
        Some(Case::Navigation) => navigation(host),
        Some(Case::Temporal) => temporal(host),
        None => Err(failure("No dynamic journey requested")),
    };
    // SAFETY: Only this fixture's main window, outside every FFI callback.
    let cleanup = unsafe { DestroyWindow(host.window.get()) };
    result.and(cleanup)
}

fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}

fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}

fn pump(host: &Host) -> Result<()> {
    for _ in 0..512 {
        if let Some(error) = host.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        let mut message = MSG::default();
        // SAFETY: Ordinary dispatch on the owning UI thread. No global input.
        unsafe {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                return Ok(());
            }
            if message.message == WM_QUIT {
                PostQuitMessage(message.wParam.0 as i32);
                return Err(failure("Host retired before the journey completed"));
            }
            if host.navigation_key(&message)
                || IsDialogMessageW(host.window.get(), &message).as_bool()
            {
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Err(failure(
        "Dynamic journey message dispatch exceeded its bound",
    ))
}

// These waits run in the ignored driver, outside all window callbacks. They
// service the real queue; they do not install a recurring presentation timer.
fn until(host: &Host, deadline: Instant) -> Result<()> {
    while Instant::now() < deadline {
        pump(host)?;
        std::thread::sleep(Duration::from_millis(5));
    }
    pump(host)
}

fn repaint(host: &Host) -> Result<()> {
    // SAFETY: Synchronous repaint of the one live owned map target.
    unsafe {
        let _ = InvalidateRect(Some(host.map_window.get()), None, false);
        UpdateWindow(host.map_window.get()).ok()?;
    }
    pump(host)
}

fn settle(host: &Host) -> Result<()> {
    until(host, Instant::now() + Duration::from_millis(1750))?;
    repaint(host)?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while matches!(WAKE.get(), Some(Wake::Again)) {
        require(
            Instant::now() < deadline,
            "Map rebuild did not settle within eight seconds",
        )?;
        until(host, Instant::now() + Duration::from_millis(10))?;
    }
    Ok(())
}

fn start(host: &Host) -> Result<()> {
    // SAFETY: Size/show only the fixture's main window. Host performs layout.
    unsafe {
        SetWindowPos(
            host.window.get(),
            None,
            0,
            0,
            1280,
            800,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
    }
    pump(host)?;
    repaint(host)
}

fn mouse(host: &Host, message: u32, x: f32, y: f32) -> Result<()> {
    // SAFETY: Read the actual DPI of this UI thread's live owned map HWND.
    let dpi = unsafe { GetDpiForWindow(host.map_window.get()) }.max(1) as f32 / 96.0;
    let x = (x * dpi).round() as i32;
    let y = (y * dpi).round() as i32;
    require(
        i16::try_from(x).is_ok() && i16::try_from(y).is_ok(),
        "Owned mouse coordinates exceed LPARAM bounds",
    )?;
    let packed = (u32::from(x as u16) | (u32::from(y as u16) << 16)) as isize;
    // SAFETY: Numeric same-thread messages to this owned map only. These
    // exercise Host input/capture, not physical mouse or system routing.
    unsafe {
        SendMessageW(
            host.map_window.get(),
            message,
            Some(WPARAM(if message == WM_LBUTTONUP {
                0
            } else {
                MK_LBUTTON.0 as usize
            })),
            Some(LPARAM(packed)),
        );
    }
    pump(host)
}

fn drag(host: &Host) -> Result<(Vector2, u64)> {
    let point = host.app.map.viewport().offset;
    mouse(host, WM_LBUTTONDOWN, point.X, point.Y)?;
    mouse(host, WM_MOUSEMOVE, point.X + 8.0, point.Y)?;
    // SAFETY: Query only pointer capture belonging to the calling UI thread.
    let captured = unsafe { GetCapture() };
    require(
        host.app.map.panning() && captured == host.map_window.get(),
        "Real Host capture did not enter panning",
    )?;
    Ok((point, CAPTURE.get()))
}

fn rendering(host: &Host) -> Result<NativeRendering> {
    host.app
        .map
        .native_rendering()
        .ok_or_else(|| failure("No live rendering observation"))
}

fn present_model(observations: &Presentations, model: &Arc<Model>) -> Result<()> {
    require(
        observations
            .latest
            .get()
            .is_some_and(|frame| frame.model == Arc::as_ptr(model) as usize),
        "Actual Present did not use the accepted model",
    )
}

fn input(host: &Host, input: Input) -> Result<()> {
    host.apply_map_update(host.app.map.input(input));
    pump(host)
}

fn origin(host: &Host, point: Option<Point>) -> Result<()> {
    let changed = facts::set_origin(&host.app, point);
    host.apply_map_update(Update {
        changed,
        capture: host.app.map.capture(),
    });
    pump(host)
}

fn supplied(
    count: usize,
    at: u64,
    active: bool,
) -> (Vec<Observation>, HashMap<ObservationId, Location>) {
    let template = facts::observations().remove(0);
    let countries = [
        ("CA", "Canada", -110.0, 72.0),
        ("NO", "Norway", 8.0, 61.0),
        ("JP", "Japan", 139.69, 35.68),
        ("BR", "Brazil", -47.9, -15.8),
        ("AU", "Australia", 149.1, -35.3),
        ("FJ", "Fiji", 179.0, -17.0),
        ("RU", "Russia", -179.0, 66.0),
    ];
    let mut observations = Vec::new();
    let mut locations = HashMap::new();
    for index in 0..count {
        let mut observation = template.clone();
        observation.id = ObservationId(1000 + index as u64);
        observation.tuple.remote = crate::connections::Endpoint::new(
            format!("198.18.{}.{}:51413", index / 256, index % 256)
                .parse()
                .unwrap(),
        );
        observation.life = Life::Tcp(TcpState::Established);
        observation.introduced = Duration::from_secs(at);
        observation.rates = if active {
            Some(Rates {
                received: 32768.0,
                sent: 24576.0,
            })
        } else if index % 2 == 0 {
            Some(Rates {
                received: 0.0,
                sent: 0.0,
            })
        } else {
            None
        };
        let (code, country, longitude, latitude) = if count > 10 && index < 100 {
            countries[0]
        } else {
            countries[index % countries.len()]
        };
        let offset = if count > 10 && index < 100 {
            index as f64 * 0.0001
        } else {
            0.0
        };
        locations.insert(
            observation.id,
            Location {
                code: code.into(),
                country: country.into(),
                city: Some(format!("Supplied-{index}").into()),
                point: Some(Point {
                    longitude: longitude + offset,
                    latitude: latitude + offset,
                }),
            },
        );
        observations.push(observation);
    }
    (observations, locations)
}

fn accept(
    host: &Host,
    observations: Vec<Observation>,
    at: u64,
    locations: &HashMap<ObservationId, Location>,
) -> (Arc<Snapshot>, Arc<Model>) {
    let previous = host.app.exploration_rows();
    let snapshot = facts::facts(&host.app, observations, at, 1, previous.as_deref());
    let model = facts::located_scene(&host.app, &snapshot, true, locations);
    (snapshot, model)
}

struct Evidence {
    directory: PathBuf,
    phases: File,
    started: Instant,
}

impl Evidence {
    fn new() -> Result<Self> {
        let directory = PathBuf::from(
            std::env::var_os("NETMAP_NATIVE_DYNAMICS_DIRECTORY")
                .ok_or_else(|| failure("Use tests/native-map-dynamics.ps1"))?,
        );
        let phases = File::create_new(directory.join("phases.jsonl"))
            .map_err(|e| failure(&e.to_string()))?;
        Ok(Self {
            directory,
            phases,
            started: Instant::now(),
        })
    }

    fn scene(
        &self,
        phase: &str,
        snapshot: &Snapshot,
        locations: &HashMap<ObservationId, Location>,
    ) -> Result<()> {
        let mut text = String::from(
            "id\tintroduced_s\tcountry\tlongitude\tlatitude\treceived_Bps\tsent_Bps\n",
        );
        for observation in snapshot.observations.iter() {
            let location = locations.get(&observation.id);
            let rates = observation.rates;
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                observation.id.0,
                observation.introduced.as_secs(),
                location.map_or("unlocated", |l| &l.code),
                location
                    .and_then(|l| l.point)
                    .map_or_else(String::new, |p| p.longitude.to_string()),
                location
                    .and_then(|l| l.point)
                    .map_or_else(String::new, |p| p.latitude.to_string()),
                rates.map_or_else(|| "unavailable".into(), |r| r.received.to_string()),
                rates.map_or_else(|| "unavailable".into(), |r| r.sent.to_string())
            ));
        }
        fs::write(self.directory.join(format!("{phase}-facts.tsv")), text)
            .map_err(|e| failure(&e.to_string()))
    }

    fn phase(
        &mut self,
        host: &Host,
        observations: &Presentations,
        phase: &str,
        image: bool,
    ) -> Result<NativeRendering> {
        let frame = if image {
            let previous = observations.attempts.get();
            host.app
                .map
                .native_image(&self.directory.join(format!("{phase}.bmp")))?;
            repaint(host)?;
            let captured = observations
                .captured
                .get()
                .ok_or_else(|| failure("Requested image did not complete EndDraw and Present"))?;
            require(
                observations.attempts.get() > previous
                    && captured.target == host.map_window.get()
                    && captured.revision == host.app.map.native_revision(),
                "Requested image did not capture the current production frame",
            )?;
            captured
        } else {
            observations
                .latest
                .get()
                .ok_or_else(|| failure("No actual Present recorded"))?
        };
        let view = host.app.map.viewport();
        let cache = rendering(host)?;
        require(
            cache.targets == 1,
            "Dynamic journey requires exactly one actual map target",
        )?;
        let (wake, delay) = match WAKE.get() {
            Some(Wake::Idle) => ("idle", 0.0),
            Some(Wake::Again) => ("again", 0.0),
            Some(Wake::After(delay)) => ("after", delay.as_secs_f64()),
            None => ("unobserved", 0.0),
        };
        let origin = facts::origin_point(&host.app);
        let geographic = view.geographic_center();
        let pulses = observations.pulses.borrow();
        // SAFETY: Read the actual DPI of this UI thread's live owned map HWND.
        let dpi = unsafe { GetDpiForWindow(host.map_window.get()) };
        writeln!(self.phases, "{{\"schema\":1,\"phase\":\"{phase}\",\"elapsed_s\":{:.6},\"present_at_s\":{:.6},\"presents\":{},\"model\":{},\"revision\":{},\"width_dip\":{},\"height_dip\":{},\"dpi\":{},\"zoom\":{},\"ratio\":{},\"longitude\":{},\"latitude\":{},\"meridian\":{},\"origin_available\":{},\"panning\":{},\"selected\":{},\"wake\":\"{wake}\",\"delay_s\":{delay},\"guides\":{},\"fills\":{},\"strokes\":{},\"sending_pulses\":{},\"receiving_pulses\":{},\"static_marks\":{},\"image\":{image}}}",
            self.started.elapsed().as_secs_f64(), frame.at.as_secs_f64(), observations.attempts.get(),
            frame.model, frame.revision, view.size.X, view.size.Y,
            dpi, view.zoom, view.ratio,
            geographic.longitude, geographic.latitude, view.meridian, origin.is_some(), host.app.map.panning(),
            host.app.selected_place().is_some(), cache.guides.len(), cache.fills.len(), cache.strokes.len(),
            pulses.iter().filter(|p| p.sending).count(), pulses.iter().filter(|p| !p.sending).count(),
            host.app.map.appearance().static_marks()).map_err(|e| failure(&e.to_string()))?;
        self.phases.flush().map_err(|e| failure(&e.to_string()))?;
        fs::write(
            self.directory.join(format!("{phase}-resources.txt")),
            format!(
                "{cache:#?}\nPulses (actual FillEllipse submissions):\n{}",
                pulses
                    .iter()
                    .map(|pulse| format!(
                        "{:?}\tsending={}\tx={}\ty={}\n",
                        pulse.key, pulse.sending, pulse.position.X, pulse.position.Y
                    ))
                    .collect::<String>()
            ),
        )
        .map_err(|e| failure(&e.to_string()))?;
        Ok(cache)
    }

    fn complete(&self) -> Result<()> {
        fs::write(self.directory.join("complete.txt"), "1\n").map_err(|e| failure(&e.to_string()))
    }
}

fn navigation(host: &Host) -> Result<()> {
    let mut evidence = Evidence::new()?;
    start(host)?;
    let (mut observations, mut locations) = supplied(128, 100, true);
    // One accepted unlocated observation remains visible in totals, not drawn
    // at a fabricated location.
    let mut unlocated = observations[0].clone();
    unlocated.id = ObservationId(9999);
    observations.push(unlocated);
    let (snapshot, model) = accept(host, observations.clone(), 100, &locations);
    require(
        model.unlocated == 1,
        "Dense supplied facts lost unlocated membership",
    )?;
    for cities in [false, true] {
        require(
            model
                .groups
                .iter()
                .filter(|group| group.place.cities == cities)
                .map(|group| group.observations.len())
                .sum::<usize>()
                == 128,
            "Dense supplied facts did not conserve each country/city membership family",
        )?;
    }
    evidence.scene("initial", &snapshot, &locations)?;
    let presents = host.app.map.observe_presentations()?;
    for _ in 0..5 {
        host.navigate(Command::ZoomIn);
    }
    settle(host)?;
    present_model(&presents, &model)?;
    evidence.phase(host, &presents, "turn-start", true)?;
    let view = host.app.map.viewport();
    let (point, _) = drag(host)?;
    let mut previous = view.geographic_center().longitude;
    let mut traversal = 0.0;
    let mut timings = File::create_new(evidence.directory.join("input-latency.tsv"))
        .map_err(|error| failure(&error.to_string()))?;
    writeln!(
        timings,
        "step\tinput_start_us\tpresent_return_us\towned_dispatch_to_present_return_us\trevision"
    )
    .map_err(|error| failure(&error.to_string()))?;
    for step in 1..=24 {
        require(
            presents.capture.borrow().is_none(),
            "Input timing began with an armed image readback",
        )?;
        let previous_presents = presents.attempts.get();
        let input_started = Instant::now();
        mouse(
            host,
            WM_MOUSEMOVE,
            point.X - view.axis_scale().X * 15.0 * step as f32,
            point.Y,
        )?;
        repaint(host)?;
        present_model(&presents, &model)?;
        require(
            presents.attempts.get() > previous_presents
                && presents
                    .latest
                    .get()
                    .is_some_and(|frame| frame.revision == host.app.map.native_revision()),
            "Full-turn step did not successfully Present its current production viewport",
        )?;
        let returned = presents.returned.get().ok_or_else(|| {
            failure("Successful current-revision Present has no return timestamp")
        })?;
        require(
            returned >= input_started,
            "Input timing reused a stale Present-return timestamp",
        )?;
        // This includes the existing owned dispatch/pump and explicit repaint,
        // ending at its last matching Present API return. It is not a first-
        // response, physical-input, input-queue, compositor or display metric.
        writeln!(
            timings,
            "{step}\t{:.3}\t{:.3}\t{:.3}\t{}",
            input_started.duration_since(evidence.started).as_secs_f64() * 1_000_000.0,
            returned.duration_since(evidence.started).as_secs_f64() * 1_000_000.0,
            returned.duration_since(input_started).as_secs_f64() * 1_000_000.0,
            host.app.map.native_revision()
        )
        .map_err(|error| failure(&error.to_string()))?;
        let current = host.app.map.viewport().geographic_center().longitude;
        let delta = (current - previous + 180.0).rem_euclid(360.0) - 180.0;
        require(
            delta > 0.0 && delta < 30.0,
            "Full-turn intermediate production frame skipped or reversed longitude",
        )?;
        traversal += delta;
        previous = current;
        if step % 6 == 0 {
            evidence.phase(host, &presents, &format!("turn-{step:02}"), true)?;
        }
    }
    timings
        .flush()
        .map_err(|error| failure(&error.to_string()))?;
    // SAFETY: Read the actual DPI of this UI thread's live owned map HWND.
    let dpi = unsafe { GetDpiForWindow(host.map_window.get()) }.max(1);
    let pixel_degrees = 96.0 / f64::from(dpi) / f64::from(view.axis_scale().X);
    require(
        (traversal - 360.0).abs() <= pixel_degrees * 2.0,
        "Intermediate production drag frames did not complete a longitude turn within pixel quantization",
    )?;
    mouse(
        host,
        WM_LBUTTONUP,
        point.X - view.axis_scale().X * 360.0,
        point.Y,
    )?;
    while host.app.map.viewport().zoom < 32.0 {
        host.navigate(Command::ZoomIn);
    }
    input(
        host,
        Input::Reveal(Point {
            longitude: -110.0,
            latitude: 72.0,
        }),
    )?;
    settle(host)?;
    let before = evidence.phase(host, &presents, "dense-before-pan", true)?;
    let (point, old_capture) = drag(host)?;
    let scale = host.app.map.viewport().axis_scale().X;
    for step in 1..=4 {
        mouse(
            host,
            WM_MOUSEMOVE,
            point.X - scale * step as f32 * 8.0,
            point.Y,
        )?;
        repaint(host)?;
        evidence.phase(host, &presents, &format!("fast-pan-{step}"), step == 2)?;
        if step == 2 {
            observations.retain(|observation| observation.id != ObservationId(1000));
            locations.remove(&ObservationId(1000));
            let mut added = observations[0].clone();
            added.id = ObservationId(2000);
            locations.insert(
                added.id,
                Location {
                    code: "NO".into(),
                    country: "Norway".into(),
                    city: Some("Supplied-new".into()),
                    point: Some(Point {
                        longitude: 8.0,
                        latitude: 61.0,
                    }),
                },
            );
            observations.push(added);
            let (snapshot, next) = accept(host, observations.clone(), 102, &locations);
            evidence.scene("during-drag", &snapshot, &locations)?;
            repaint(host)?;
            require(
                host.app.map.panning(),
                "Accepted facts cancelled the active production drag",
            )?;
            present_model(&presents, &next)?;
            require(
                !next
                    .groups
                    .iter()
                    .any(|group| group.observations.contains(&ObservationId(1000)))
                    && next
                        .groups
                        .iter()
                        .any(|group| group.observations.contains(&ObservationId(2000))),
                "Accepted membership change was not arranged",
            )?;
        }
    }
    let after = evidence.phase(host, &presents, "fast-pan-after", true)?;
    require(
        before.fill_window != after.fill_window,
        "Fast pan did not cross the retained fill window",
    )?;
    let geographic = host.app.map.viewport().geographic_center();
    origin(host, None)?;
    repaint(host)?;
    // SAFETY: Query only pointer capture belonging to the calling UI thread.
    let captured = unsafe { GetCapture() };
    require(
        !host.app.map.panning() && captured != host.map_window.get(),
        "Origin loss did not retire real pointer capture",
    )?;
    let unavailable = evidence.phase(host, &presents, "origin-unavailable", true)?;
    require(
        unavailable.guides.is_empty(),
        "Origin-unavailable frame retained route paints",
    )?;
    late_end(host, old_capture)?;
    require(
        host.app.map.viewport().geographic_center() == geographic,
        "Late capture/end restored the retired drag center",
    )?;
    let (_, next_capture) = drag(host)?;
    let geographic = host.app.map.viewport().geographic_center();
    origin(
        host,
        Some(Point {
            longitude: 139.69,
            latitude: 35.68,
        }),
    )?;
    late_end(host, next_capture)?;
    let rebased = host.app.map.viewport().geographic_center();
    // Geographic centers are composed from f32 viewport coordinates and the
    // new meridian. Bound their rounding at the largest longitude magnitude.
    let rounding = f64::from(f32::EPSILON) * 360.0;
    require(
        !host.app.map.panning()
            && (rebased.longitude - geographic.longitude).abs() <= rounding
            && (rebased.latitude - geographic.latitude).abs() <= rounding,
        "Manual origin/late end lost the current non-fitted geographic center",
    )?;
    settle(host)?;
    evidence.phase(host, &presents, "manual-origin", true)?;
    input(
        host,
        Input::Reveal(Point {
            longitude: -110.0,
            latitude: 72.0,
        }),
    )?;
    settle(host)?;
    let current = host.app.exploration_rows();
    require(current.is_some(), "Accepted dense rows missing")?;
    // Select the actual retained supplied place through Application, then let
    // normal Host resize/layout/Map preparation establish its new viewport.
    let (_, selected_model) = accept(host, observations, 103, &locations);
    let selected = selected_model
        .groups
        .iter()
        .find(|g| g.place.code.as_ref() == "CA")
        .ok_or_else(|| failure("Dense Canada place missing"))?
        .place
        .clone();
    host.app.select_place(Some(selected.clone()));
    settle(host)?;
    let selected_cache = evidence.phase(host, &presents, "dense-selected", true)?;
    // SAFETY: Resize only the fixture's live main HWND on its creating thread.
    unsafe {
        SetWindowPos(
            host.window.get(),
            None,
            0,
            0,
            1060,
            900,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
    }
    settle(host)?;
    require(
        host.app.selected_place().as_ref() == Some(&selected),
        "Actual Host resize lost selected place",
    )?;
    let resized = evidence.phase(host, &presents, "dense-selected-resize", true)?;
    require(
        selected_cache.guides == resized.guides,
        "Actual Host resize replaced geographic route guides",
    )?;
    evidence.complete()
}

fn late_end(host: &Host, id: u64) -> Result<()> {
    input(host, Input::Captured { id, success: true })?;
    // SAFETY: Late capture notification/release to the owned target only.
    unsafe {
        SendMessageW(
            host.map_window.get(),
            WM_CAPTURECHANGED,
            None,
            Some(LPARAM(0)),
        );
    }
    mouse(host, WM_LBUTTONUP, 10.0, 10.0)
}

fn temporal(host: &Host) -> Result<()> {
    let mut evidence = Evidence::new()?;
    start(host)?;
    facts::set_highlight(&host.app, Duration::from_secs(10), Duration::from_secs(20));
    let (observations, locations) = supplied(5, 100, false);
    let (snapshot, model) = accept(host, observations, 100, &locations);
    evidence.scene("default", &snapshot, &locations)?;
    let anchor = facts::sample_anchor(&host.app).unwrap().1;
    let presents = host.app.map.observe_presentations()?;
    settle(host)?;
    present_model(&presents, &model)?;
    let bright = evidence.phase(host, &presents, "default-bright", true)?;
    let scheduled = match WAKE.get() {
        Some(Wake::After(delay)) => delay,
        _ => {
            return Err(failure(
                "Bright settled map did not schedule its next boundary",
            ));
        }
    };
    require(
        scheduled > Duration::from_secs(1),
        "Bright idle map retained a recurring frame wake",
    )?;
    let count = presents.attempts.get();
    until(host, anchor + Duration::from_secs(8))?;
    require(
        presents.attempts.get() == count,
        "Static bright map redrew before its event-driven deadline",
    )?;
    evidence.phase(host, &presents, "default-bright-quiet", false)?;
    until(host, anchor + Duration::from_millis(10400))?;
    require(
        presents.attempts.get() > count
            && presents.latest.get().unwrap().at >= Duration::from_secs(110),
        "Default bright boundary did not wake actual presentation",
    )?;
    require(
        matches!(WAKE.get(), Some(Wake::After(delay)) if delay <= Duration::from_millis(40)),
        "Actual fading map did not request animation frames",
    )?;
    evidence.phase(host, &presents, "default-fade-start", true)?;
    let hidden_at = presents.latest.get().unwrap().at;
    // SAFETY: Hide only the fixture's live main HWND on its creating thread.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_HIDE);
    }
    host.refresh_activity();
    pump(host)?;
    let hidden_count = presents.attempts.get();
    until(host, anchor + Duration::from_secs(21))?;
    require(
        presents.attempts.get() == hidden_count,
        "Hidden map continued presenting",
    )?;
    // SAFETY: Show only the same owned main HWND; Host resumes its real target.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
    }
    host.refresh_activity();
    pump(host)?;
    let resumed = presents
        .latest
        .get()
        .ok_or_else(|| failure("Resume did not present"))?;
    require(
        resumed.at >= Duration::from_secs(121) && resumed.at > hidden_at + Duration::from_secs(9),
        "Resume replayed hidden time instead of using the accepted current age",
    )?;
    let after_resume = evidence.phase(host, &presents, "default-fade-resumed", true)?;
    require(
        bright.guides == after_resume.guides && bright.fills == after_resume.fills,
        "Hide/resume discarded still-owned geographic resources",
    )?;
    let highlight = Highlight {
        bright: Duration::from_secs(10),
        fade: Duration::from_secs(20),
    };
    require(
        highlight.opacity(Duration::from_secs(100), resumed.at, false) < 0.5,
        "Current resumed clock did not reach the latter default fade",
    )?;
    until(host, anchor + Duration::from_millis(30500))?;
    require(
        matches!(WAKE.get(), Some(Wake::Idle)),
        "Expired default highlight retained a presentation wake",
    )?;
    evidence.phase(host, &presents, "default-expired", true)?;
    let expired_count = presents.attempts.get();
    until(host, Instant::now() + Duration::from_millis(250))?;
    require(
        presents.attempts.get() == expired_count,
        "Expired map did not become event-driven idle",
    )?;

    let (mut observations, locations) = supplied(5, 200, false);
    for (observation, rates) in observations.iter_mut().zip([
        Some(Rates {
            received: 0.0,
            sent: 32768.0,
        }),
        Some(Rates {
            received: 32768.0,
            sent: 0.0,
        }),
        Some(Rates {
            received: 32768.0,
            sent: 24576.0,
        }),
        Some(Rates {
            received: 0.0,
            sent: 0.0,
        }),
        None,
    ]) {
        observation.rates = rates;
    }
    let (snapshot, model) = accept(host, observations.clone(), 200, &locations);
    evidence.scene("activity", &snapshot, &locations)?;
    settle(host)?;
    let first = presents.pulses.borrow().clone();
    require(
        !first.is_empty(),
        "Active measured directions did not submit pulses",
    )?;
    for destination in &model.destinations {
        let rates = model.groups[destination.group].rates;
        for sending in [false, true] {
            let expected = rates.is_some_and(|r| {
                if sending {
                    r.sent > 0.0
                } else {
                    r.received > 0.0
                }
            });
            require(
                first
                    .iter()
                    .any(|p| p.key == destination.key && p.sending == sending)
                    == expected,
                "Submitted pulse directions disagree with measured/unavailable activity",
            )?;
        }
    }
    let steady = evidence.phase(host, &presents, "activity-first", true)?;
    until(host, Instant::now() + Duration::from_millis(300))?;
    let second = presents.pulses.borrow().clone();
    require(
        first.iter().any(|a| {
            second
                .iter()
                .any(|b| a.key == b.key && a.sending == b.sending && a.position != b.position)
        }),
        "Production pulse positions did not advance",
    )?;
    let advanced = evidence.phase(host, &presents, "activity-advanced", true)?;
    require(
        steady.guides == advanced.guides
            && steady.glow != 0
            && steady.glow == advanced.glow
            && steady.route_strokes == advanced.route_strokes,
        "Steady active animation rebuilt retained route resources",
    )?;

    let normal = host.app.map.appearance();
    let mut policy = normal;
    policy.animations = false;
    host.app.map.set_appearance(policy);
    repaint(host)?;
    require(
        presents.pulses.borrow().is_empty(),
        "Supplied reduced motion submitted animated pulses",
    )?;
    evidence.phase(host, &presents, "supplied-reduced-motion", true)?;
    policy.animations = true;
    policy.contrast = Some((
        D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    ));
    host.app.map.set_appearance(policy);
    repaint(host)?;
    require(
        presents.pulses.borrow().is_empty(),
        "Supplied contrast policy submitted animated pulses",
    )?;
    let contrast = evidence.phase(host, &presents, "supplied-high-contrast", true)?;
    let pixels = fs::read(evidence.directory.join("supplied-high-contrast.bmp"))
        .map_err(|error| failure(&error.to_string()))?;
    require(
        pixels.get(54..57).is_some_and(|pixel| pixel == [0, 0, 0]),
        "Captured supplied-contrast clear pixel is not its actual black background",
    )?;
    require(
        contrast.glow == 0 && contrast.guides == advanced.guides,
        "Supplied contrast did not clear glow while retaining geographic route guides",
    )?;
    host.app.map.set_appearance(normal);
    for observation in &mut observations {
        observation.rates = Some(Rates {
            received: 0.0,
            sent: 0.0,
        });
        observation.introduced = Duration::from_secs(300);
    }
    facts::set_highlight(&host.app, Duration::from_secs(3), Duration::ZERO);
    let (snapshot, _) = accept(host, observations, 300, &locations);
    evidence.scene("zero-fade", &snapshot, &locations)?;
    let anchor = facts::sample_anchor(&host.app).unwrap().1;
    settle(host)?;
    evidence.phase(host, &presents, "zero-fade-bright", true)?;
    until(host, anchor + Duration::from_millis(3400))?;
    require(
        matches!(WAKE.get(), Some(Wake::Idle)),
        "Zero-fade boundary failed to become idle",
    )?;
    evidence.phase(host, &presents, "zero-fade-expired", true)?;
    evidence.complete()
}
