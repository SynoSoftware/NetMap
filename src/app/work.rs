use std::{
    cell::Cell,
    io,
    marker::PhantomData,
    net::IpAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{GetCurrentProcess, TerminateProcess},
        },
        UI::WindowsAndMessaging::*,
    },
    core::{Error, Result, w},
};

use super::{
    Application,
    origin::{self, Request},
    placement::{self, Placement},
    settings::{Preferences, storage},
};
use crate::{
    connections::{Highlight, Publication, etw},
    geography::World,
    geolocation::{
        Geolocation, Report,
        dataset::{self, Phase},
    },
};

mod appearance;
mod broker;
mod connections;
mod hosting;
pub(crate) use connections::Source;
pub(crate) mod http;
#[cfg(test)]
pub(crate) mod native_checks;
#[cfg(test)]
pub(crate) mod native_dataset;
#[cfg(test)]
pub(crate) mod native_desktop;
#[cfg(test)]
pub(crate) mod native_preferences;
#[cfg(test)]
pub(crate) mod native_shutdown;
mod network;
mod retirement;
pub(crate) mod scene;
#[cfg(test)]
mod trace_checks;

const REPLY_MESSAGE: u32 = WM_APP + 1;
pub(super) const ORIGIN_TIMER: usize = 1;
#[derive(Debug)]
pub enum Reply {
    HostingInfo {
        monitors: Vec<(String, bool)>,
        saver: String,
    },
    SaverAction(std::result::Result<(), String>),
    CaptureCommand(std::result::Result<(), String>),
    Desktop {
        id: u64,
        result: std::result::Result<
            (
                crate::host::desktop::Attachment,
                Vec<crate::host::desktop::Monitor>,
            ),
            String,
        >,
    },
    Palette(crate::map::Palette),
    Connections(std::result::Result<ConnectionUpdate, String>),
    Scene {
        id: u64,
        model: Arc<crate::map::model::Model>,
        rows: Arc<super::exploration::Rows>,
        at: Duration,
        sampled: Instant,
    },
    OriginAddress {
        id: u64,
        result: std::result::Result<IpAddr, String>,
    },
    OriginLocation {
        id: u64,
        version: u64,
        result: std::result::Result<Option<crate::geolocation::Location>, String>,
    },
    NetworkWatch(std::result::Result<(), String>),
    Preferences(io::Result<Preferences>),
    Placement(io::Result<Option<Placement>>),
    PreferencesSaved {
        id: u64,
        result: io::Result<()>,
    },
    World(io::Result<Arc<World>>),
    Locations(Report),
    CityProgress {
        id: u64,
        phase: Phase,
    },
    CityFinished {
        id: u64,
        result: std::result::Result<(), dataset::Error>,
    },
}

pub struct Work {
    sender: mpsc::SyncSender<Reply>,
    replies: mpsc::Receiver<Reply>,
    cancelled: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    geography_started: bool,
    appearance: Option<mpsc::SyncSender<()>>,
    geolocation: Arc<Mutex<Geolocation>>,
    city: Option<(u64, Arc<AtomicBool>)>,
    origin: Option<OriginWork>,
    network_changed: Arc<AtomicBool>,
    connections: Option<mpsc::SyncSender<ConnectionCommand>>,
    pending_highlight: Option<Highlight>,
    pending_rows: Option<(Source, Arc<super::exploration::Rows>)>,
    latest_connections: Arc<Mutex<Option<std::result::Result<ConnectionUpdate, String>>>>,
    endpoint: Cell<HWND>,
    scene: Option<mpsc::SyncSender<scene::Request>>,
    pending_scene: Option<scene::Request>,
    retirement: Option<mpsc::Sender<retirement::Payload>>,
    retirement_receiver: Option<mpsc::Receiver<retirement::Payload>>,
}

#[derive(Debug)]
pub(crate) struct ConnectionUpdate {
    pub publication: Publication,
    pub rows: Arc<super::exploration::Rows>,
    pub source: Source,
}

impl ConnectionUpdate {
    fn prepare(
        publication: Publication,
        source: Source,
        previous: Option<&super::exploration::Rows>,
    ) -> Self {
        let rows = Arc::new(super::exploration::Rows::prepare_pending(
            &publication.snapshot,
            &publication.names,
            previous,
        ));
        Self {
            publication,
            rows,
            source,
        }
    }
}

enum ConnectionCommand {
    Rows(Source, Arc<super::exploration::Rows>),
    Configure(Highlight),
    Failed(u64, String),
}

struct OriginWork {
    commands: mpsc::SyncSender<OriginCommand>,
    current: Option<(u64, Arc<AtomicBool>)>,
}

enum OriginCommand {
    Request(Request, Arc<AtomicBool>),
    Stop,
}

impl Work {
    pub fn start_capture(&mut self, endpoint: HWND) -> io::Result<()> {
        let reporter = self.reporter(endpoint);
        self.spawn("NetMap capture command", move || {
            let result = std::env::current_exe()
                .and_then(|path| shell_launch(&path.to_string_lossy(), "--collector", "runas"))
                .map_err(|error| error.to_string());
            reporter.send(Reply::CaptureCommand(result));
        })
    }

    pub fn run_collector(data: PathBuf) -> io::Result<()> {
        let mut work = Self::new();
        work.start_retirement()?;
        let retired = Arc::clone(&work.cancelled);
        let latest: broker::Latest = Arc::new(Mutex::new(None));
        let (errors, failures) = mpsc::channel::<String>();
        let (consumers, consumer_receiver) = mpsc::sync_channel::<etw::Consumer>(1);
        let result = (|| {
            let consumer_errors = errors.clone();
            let consumer_retired = Arc::clone(&retired);
            work.spawn("NetMap shared trace", move || {
                let consumer = loop {
                    match consumer_receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(consumer) => break consumer,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(mpsc::RecvTimeoutError::Timeout)
                            if consumer_retired.load(Ordering::Acquire) =>
                        {
                            return;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    }
                };
                let result = consumer.run();
                if !consumer_retired.load(Ordering::Acquire) {
                    let _ = consumer_errors.send(result.err().map_or_else(
                        || "Trace consumer stopped".into(),
                        |error| error.to_string(),
                    ));
                    consumer_retired.store(true, Ordering::Release);
                }
            })?;
            let collect_retired = Arc::clone(&retired);
            let collect_latest = Arc::clone(&latest);
            let collect_errors = errors.clone();
            work.spawn("NetMap shared observations", move || {
                if let Err(error) = broker::collect(
                    data,
                    collect_latest,
                    Arc::clone(&collect_retired),
                    consumers,
                ) {
                    let _ = collect_errors.send(error);
                    collect_retired.store(true, Ordering::Release);
                }
            })?;
            let server_retired = Arc::clone(&retired);
            work.spawn("NetMap shared snapshots", move || {
                if let Err(error) = broker::serve(latest, Arc::clone(&server_retired)) {
                    let _ = errors.send(error);
                    server_retired.store(true, Ordering::Release);
                }
            })?;
            while !retired.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(100));
            }
            failures
                .try_recv()
                .map_or(Ok(()), |error| Err(io::Error::other(error)))
        })();
        work.finish(Duration::from_secs(5));
        result
    }

    pub fn screen_saver_action(
        &mut self,
        action: super::settings::SaverAction,
        endpoint: HWND,
    ) -> io::Result<()> {
        let reporter = self.reporter(endpoint);
        self.spawn("NetMap screen saver command", move || {
            let result = (|| -> io::Result<()> {
                let executable = std::env::current_exe()?;
                let saver = executable.with_file_name("netmap.scr");
                let (program, parameters) = match action {
                    super::settings::SaverAction::Preview => {
                        std::fs::metadata(&saver)?;
                        (saver.to_string_lossy().into_owned(), "/s".to_string())
                    }
                    super::settings::SaverAction::Install => {
                        std::fs::metadata(&saver)?;
                        (
                            "rundll32.exe".to_string(),
                            format!("desk.cpl,InstallScreenSaver \"{}\"", saver.display()),
                        )
                    }
                    super::settings::SaverAction::WindowsSettings => (
                        "control.exe".to_string(),
                        "desk.cpl,screensaver,@screensaver".to_string(),
                    ),
                };
                shell_launch(&program, &parameters, "open")
            })()
            .map_err(|error| error.to_string());
            reporter.send(Reply::SaverAction(result));
        })
    }

    pub fn discover_desktop(&mut self, id: u64, endpoint: HWND) -> io::Result<()> {
        let reporter = self.reporter(endpoint);
        #[cfg(test)]
        let gate = native_desktop::take_next(id);
        self.spawn("NetMap desktop discovery", move || {
            let result = crate::host::desktop::discover()
                .and_then(|attachment| {
                    crate::host::desktop::monitors().map(|monitors| (attachment, monitors))
                })
                .map_err(|error| error.to_string());
            #[cfg(test)]
            if let Some(gate) = gate.as_ref()
                && !gate.before_reply(&result)
            {
                return;
            }
            reporter.send(Reply::Desktop { id, result });
        })
    }

    pub fn new() -> Self {
        let (sender, replies) = mpsc::sync_channel(8);
        let (retirement, retirement_receiver) = mpsc::channel();
        Self {
            sender,
            replies,
            cancelled: Arc::new(AtomicBool::new(false)),
            workers: Vec::new(),
            geography_started: false,
            appearance: None,
            geolocation: Arc::new(Mutex::new(Geolocation::default())),
            city: None,
            origin: None,
            network_changed: Arc::new(AtomicBool::new(false)),
            connections: None,
            pending_highlight: None,
            pending_rows: None,
            latest_connections: Arc::new(Mutex::new(None)),
            endpoint: Cell::new(HWND::default()),
            scene: None,
            pending_scene: None,
            retirement: Some(retirement),
            retirement_receiver: Some(retirement_receiver),
        }
    }

    pub fn start_retirement(&mut self) -> io::Result<()> {
        if let Some(receiver) = self.retirement_receiver.take() {
            self.spawn("NetMap retired collections", move || {
                retirement::run(receiver)
            })?;
        }
        Ok(())
    }

    fn retire(&self, payload: retirement::Payload) {
        // The receiver lives through finish, including cancelled/stale replies.
        // This non-capacity-limited send never waits for destruction to finish.
        self.retirement
            .as_ref()
            .expect("Work is live")
            .send(payload)
            .unwrap_or_else(|_| panic!("Collection retirement stopped unexpectedly"));
    }

    pub fn retire_rows(&self, rows: Arc<super::exploration::Rows>) {
        self.retire(retirement::Payload::Rows(rows));
    }

    pub fn retire_model(&self, model: Arc<crate::map::model::Model>) {
        self.retire(retirement::Payload::Model(model));
    }

    pub fn retire_world(&self, world: Arc<World>) {
        self.retire(retirement::Payload::World(world));
    }

    pub fn retire_publication(&self, publication: Publication) {
        self.retire(retirement::Payload::Publication(publication));
    }

    pub fn start_connections(&mut self, highlight: Highlight, endpoint: HWND) -> io::Result<()> {
        if self.connections.is_some() || self.retired() {
            return Ok(());
        }
        let reporter = self.reporter(endpoint);
        let retired = Arc::clone(&self.cancelled);
        let latest = Arc::clone(&self.latest_connections);
        let (commands, requests) = mpsc::sync_channel(2);
        let (consumer_sender, consumer_receiver) = mpsc::sync_channel::<(u64, etw::Consumer)>(1);
        let trace_commands = commands.clone();
        let trace_cancelled = Arc::clone(&retired);
        self.spawn("NetMap trace", move || {
            while !trace_cancelled.load(Ordering::Acquire) {
                let (generation, consumer) =
                    match consumer_receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(value) => value,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    };
                let result = consumer.run();
                if !trace_cancelled.load(Ordering::Acquire) {
                    let error = result.err().map_or_else(
                        || "Trace consumption ended unexpectedly".into(),
                        |error| error.to_string(),
                    );
                    let _ = trace_commands.send(ConnectionCommand::Failed(generation, error));
                }
            }
        })?;
        self.spawn("NetMap connections", move || {
            connections::run(
                highlight,
                requests,
                consumer_sender,
                latest,
                reporter,
                retired,
            );
        })?;
        self.connections = Some(commands);
        Ok(())
    }

    pub fn configure_connections(&mut self, highlight: Highlight) {
        self.pending_highlight = Some(highlight);
        self.pump_connections();
    }

    pub fn configure_rows(&mut self, source: Source, rows: Arc<super::exploration::Rows>) {
        if let Some((_, previous)) = self.pending_rows.replace((source, rows)) {
            self.retire_rows(previous);
        }
        self.pump_connections();
    }

    fn pump_connections(&mut self) {
        if let Some((source, rows)) = self.pending_rows.take() {
            match self.connections.as_ref().map(|commands| {
                commands.try_send(ConnectionCommand::Rows(source, Arc::clone(&rows)))
            }) {
                Some(Ok(())) => self.retire_rows(rows),
                Some(Err(mpsc::TrySendError::Full(_))) => self.pending_rows = Some((source, rows)),
                _ => self.retire_rows(rows),
            }
        }
        if let Some(highlight) = self.pending_highlight.take()
            && let Some(commands) = &self.connections
            && matches!(
                commands.try_send(ConnectionCommand::Configure(highlight)),
                Err(mpsc::TrySendError::Full(_))
            )
        {
            self.pending_highlight = Some(highlight);
        }
    }

    pub fn prepare_scene(&mut self, request: scene::Request, endpoint: HWND) -> io::Result<()> {
        if self.retired() {
            self.retire(retirement::Payload::Scene(request));
            return Ok(());
        }
        if self.scene.is_none() {
            let (sender, receiver) = mpsc::sync_channel(1);
            let reporter = self.reporter(endpoint);
            let geo = Arc::clone(&self.geolocation);
            let cancelled = Arc::clone(&self.cancelled);
            self.spawn("NetMap locations and map", move || {
                scene::run(receiver, reporter, geo, cancelled)
            })?;
            self.scene = Some(sender);
        }
        if let Some(previous) = self.pending_scene.replace(request) {
            self.retire(retirement::Payload::Scene(previous));
        }
        self.pump_scene();
        Ok(())
    }

    fn pump_scene(&mut self) {
        if let Some(request) = self.pending_scene.take() {
            if let Some(sender) = &self.scene {
                match sender.try_send(request) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(request)) => self.pending_scene = Some(request),
                    Err(mpsc::TrySendError::Disconnected(request)) => {
                        self.retire(retirement::Payload::Scene(request))
                    }
                }
            } else {
                self.retire(retirement::Payload::Scene(request));
            }
        }
    }

    pub fn load_world(
        &mut self,
        directory: PathBuf,
        data: PathBuf,
        endpoint: HWND,
    ) -> io::Result<()> {
        if self.geography_started || self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::other(
                "World loading is already started or retired",
            ));
        }
        let reporter = self.reporter(endpoint);
        let geolocation = Arc::clone(&self.geolocation);
        let (appearance, changes) = mpsc::sync_channel(1);
        let retired = Arc::clone(&self.cancelled);
        self.spawn("NetMap geography", move || {
            if !reporter.send(hosting::info()) {
                return;
            }
            if !reporter.send(Reply::Placement(placement::load(&data))) {
                return;
            }
            if let Ok(palette) = appearance::read() {
                reporter.send(Reply::Palette(palette));
            }
            if !reporter.send(Reply::Preferences(storage::load(&data))) {
                return;
            }
            let result = std::fs::read(directory.join("world.bin"))
                .and_then(|bytes| World::decode(&bytes))
                .map(Arc::new);
            if !reporter.send(Reply::World(result)) {
                return;
            }
            // Only registered workers lock this reader owner. The UI
            // receives owned reports/locations and never waits for it.
            let report = geolocation
                .lock()
                .ok()
                .map(|mut geo| geo.load(&directory, &data));
            if let Some(report) = report {
                reporter.send(Reply::Locations(report));
            }
            while changes.recv().is_ok() && !retired.load(Ordering::Acquire) {
                if !reporter.send(hosting::info()) {
                    break;
                }
                if let Ok(palette) = appearance::read()
                    && !reporter.send(Reply::Palette(palette))
                {
                    break;
                }
            }
        })?;
        self.appearance = Some(appearance);
        self.geography_started = true;
        Ok(())
    }

    pub fn refresh_appearance(&self) {
        if let Some(changes) = &self.appearance {
            let _ = changes.try_send(());
        }
    }

    pub fn download_city(&mut self, id: u64, directory: PathBuf, endpoint: HWND) -> io::Result<()> {
        if self.city.is_some() || self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::other(
                "A city operation is already running or retired",
            ));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let operation_cancel = Arc::clone(&cancelled);
        let reporter = self.reporter(endpoint);
        let geolocation = Arc::clone(&self.geolocation);
        #[cfg(test)]
        let fixture = native_dataset::take_candidate();
        self.spawn("NetMap city data", move || {
            let mut last = Instant::now();
            let mut last_phase = None;
            let mut progress = |phase: Phase| {
                let kind = std::mem::discriminant(&phase);
                if last_phase != Some(kind) || last.elapsed() >= Duration::from_millis(200) {
                    last_phase = Some(kind);
                    last = Instant::now();
                    reporter.progress(Reply::CityProgress { id, phase });
                }
            };
            #[cfg(test)]
            let candidate =
                native_dataset::build(fixture, &directory, &operation_cancel, &mut progress);
            #[cfg(not(test))]
            let candidate = dataset::build(&directory, &operation_cancel, &mut progress);
            let result = candidate.and_then(|candidate| {
                if operation_cancel.load(Ordering::Acquire) {
                    return Err(dataset::Error::Cancelled);
                }
                progress(Phase::Installing);
                geolocation
                    .lock()
                    .map_err(|_| io::Error::other("Geolocation worker failed"))?
                    .install(candidate, &directory)
                    .map_err(Into::into)
            });
            let report = geolocation.lock().ok().map(|geo| geo.report());
            if let Some(report) = report {
                reporter.send(Reply::Locations(report));
            }
            reporter.send(Reply::CityFinished {
                id,
                result: result.map(|_| ()),
            });
        })?;
        self.city = Some((id, cancelled));
        Ok(())
    }

    pub fn save_preferences(
        &mut self,
        id: u64,
        directory: PathBuf,
        previous: Preferences,
        desired: Preferences,
        endpoint: HWND,
    ) -> io::Result<()> {
        let reporter = self.reporter(endpoint);
        #[cfg(test)]
        let gate = native_preferences::take_gate();
        self.spawn("NetMap preferences", move || {
            #[cfg(test)]
            if let Some(gate) = gate
                && let Err(error) = gate.wait()
            {
                reporter.send(Reply::PreferencesSaved {
                    id,
                    result: Err(error),
                });
                return;
            }
            // Accepted writes finish independently of dialog and presentation
            // cancellation. Process finalization gives this registered worker
            // the same bounded cleanup allowance as other native operations.
            let result = storage::save(&directory, &previous, &desired);
            reporter.send(Reply::PreferencesSaved { id, result });
        })
    }

    pub fn save_window_placement(
        &mut self,
        directory: PathBuf,
        value: Placement,
    ) -> io::Result<()> {
        self.spawn("NetMap window placement", move || {
            if let Err(error) = placement::save(&directory, value) {
                eprintln!("Window placement could not be saved: {error}");
            }
        })
    }

    fn reporter(&self, endpoint: HWND) -> Reporter {
        self.endpoint.set(endpoint);
        Reporter {
            sender: self.sender.clone(),
            cancelled: Arc::clone(&self.cancelled),
            endpoint: endpoint.0 as usize,
        }
    }

    pub fn network_changed(&self) -> bool {
        self.network_changed.swap(false, Ordering::AcqRel)
    }

    pub fn retired(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn ensure_origin(&mut self, request: Option<Request>, endpoint: HWND) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(work) = &self.origin {
            if work
                .current
                .as_ref()
                .is_some_and(|(id, _)| request.is_some_and(|request| request.id() == *id))
            {
                return Ok(());
            }
            if let Some((_, cancel)) = &work.current {
                cancel.store(true, Ordering::Release);
            }
        }
        let Some(request) = request else {
            return Ok(());
        };
        if self.origin.is_none() {
            self.start_origin(endpoint)?;
        }
        let work = self.origin.as_mut().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        match work
            .commands
            .try_send(OriginCommand::Request(request, Arc::clone(&cancelled)))
        {
            Ok(()) => work.current = Some((request.id(), cancelled)),
            // A cancelled queued request will complete and wake the owner,
            // which retries its current desired request. Never queue a history
            // of route changes or wait for a worker from the message loop.
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err(io::Error::other("Origin worker stopped"));
            }
        }
        Ok(())
    }

    fn start_origin(&mut self, endpoint: HWND) -> io::Result<()> {
        let reporter = self.reporter(endpoint);
        #[cfg(test)]
        let local_only = native_checks::local_origin_only();
        let retired = Arc::clone(&self.cancelled);
        let dirty = Arc::clone(&self.network_changed);
        let geolocation = Arc::clone(&self.geolocation);
        let (commands, requests) = mpsc::sync_channel(1);
        self.spawn("NetMap origin", move || {
            let watch = network::Watch::new(dirty, Arc::clone(&retired), reporter.clone());
            let watch = match watch {
                Ok(watch) => {
                    reporter.send(Reply::NetworkWatch(Ok(())));
                    Some(watch)
                }
                Err(error) => {
                    reporter.send(Reply::NetworkWatch(Err(error.to_string())));
                    None
                }
            };
            while let Ok(command) = requests.recv() {
                if retired.load(Ordering::Acquire) {
                    break;
                }
                let OriginCommand::Request(request, cancelled) = command else {
                    break;
                };
                let reply = match request {
                    Request::Discover { id } => {
                        #[cfg(test)]
                        if local_only {
                            eprintln!("NETMAP_LOCAL_ORIGIN_DISCOVERY_REFUSED");
                            if !reporter.send(Reply::OriginAddress {
                                id,
                                result: Err("Local native check refused public discovery".into()),
                            }) {
                                break;
                            }
                            continue;
                        }
                        let mut bytes = Vec::with_capacity(128);
                        let result = http::download(
                            origin::PROVIDER,
                            "/",
                            &mut bytes,
                            128,
                            &cancelled,
                            |_, _| {},
                        )
                        .map_err(|error| error.to_string())
                        .and_then(|_| origin::parse_address(&bytes));
                        Reply::OriginAddress { id, result }
                    }
                    Request::Locate {
                        id,
                        address,
                        version,
                    } => {
                        let located = if cancelled.load(Ordering::Acquire) {
                            Err(io::Error::other("Origin request cancelled"))
                        } else {
                            geolocation
                                .lock()
                                .map_err(|_| io::Error::other("Geolocation worker failed"))
                                .map(|geo| (geo.report().version, geo.find(address)))
                        };
                        let (version, result) = match located {
                            Ok((version, result)) => {
                                (version, result.map_err(|error| error.to_string()))
                            }
                            Err(error) => (version, Err(error.to_string())),
                        };
                        Reply::OriginLocation {
                            id,
                            version,
                            result,
                        }
                    }
                };
                if !reporter.send(reply) {
                    break;
                }
            }
            drop(watch);
        })?;
        self.origin = Some(OriginWork {
            commands,
            current: None,
        });
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn native_broker_restart(&mut self) -> io::Result<()> {
        let mut next = Self::new();
        next.start_retirement()?;
        // This check keeps accepted UI facts alive. Their retirement worker
        // cannot finish until those owners release them, so transfer its join
        // handle to the continuing Work owner before finishing the old source.
        let (retirements, workers): (Vec<_>, Vec<_>) = self
            .workers
            .drain(..)
            .partition(|worker| worker.thread().name() == Some("NetMap retired collections"));
        self.workers = workers;
        next.workers.extend(retirements);
        let mut previous = std::mem::replace(self, next);
        previous.finish(Duration::from_secs(5));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn native_broker_traffic(
        &mut self,
        body: impl FnOnce(Arc<AtomicBool>) + Send + 'static,
    ) -> io::Result<()> {
        let cancelled = Arc::clone(&self.cancelled);
        self.spawn("NetMap broker loopback check", move || body(cancelled))
    }

    fn spawn(&mut self, name: &str, body: impl FnOnce() + Send + 'static) -> io::Result<()> {
        self.workers.retain(|worker| !worker.is_finished());
        // Five persistent roles (startup/appearance, origin/watch, collector,
        // trace and scene) can overlap city, preferences, final placement and
        // bounded Explorer discovery, saver action, explicit capture launch and
        // plain collection retirement.
        // Application coalesces host requests.
        if self.workers.len() >= 12 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Background work is busy",
            ));
        }
        self.workers
            .push(thread::Builder::new().name(name.into()).spawn(body)?);
        Ok(())
    }

    pub fn reply(&mut self) -> Option<Reply> {
        self.pump_connections();
        self.pump_scene();
        let reply = self.replies.try_recv().ok().or_else(|| {
            match self.latest_connections.try_lock() {
                Ok(mut slot) => slot.take().map(Reply::Connections),
                // A worker only swaps an owned pointer under this lock. Retry
                // through a wake instead of waiting on the message loop.
                Err(std::sync::TryLockError::WouldBlock) => {
                    self.reporter(self.endpoint.get()).wake();
                    None
                }
                Err(std::sync::TryLockError::Poisoned(_)) => None,
            }
        });
        if let Some(Reply::OriginAddress { id, .. } | Reply::OriginLocation { id, .. }) = &reply
            && let Some(origin) = &mut self.origin
            && origin
                .current
                .as_ref()
                .is_some_and(|(current, _)| current == id)
        {
            origin.current = None;
        }
        if let Some(Reply::CityFinished { id, .. }) = &reply
            && self.city.as_ref().is_some_and(|(current, _)| current == id)
        {
            self.city = None;
        }
        if self.cancelled.load(Ordering::Acquire) {
            if let Some(reply) = reply {
                self.retire(retirement::Payload::Reply(reply));
            }
            None
        } else {
            reply
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.cancel_city();
        if let Some(origin) = &self.origin {
            if let Some((_, cancel)) = &origin.current {
                cancel.store(true, Ordering::Release);
            }
            let _ = origin.commands.try_send(OriginCommand::Stop);
        }
    }

    pub fn cancel_city(&self) {
        if let Some((_, cancelled)) = &self.city {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub fn finish(&mut self, allowance: Duration) {
        self.cancel();
        self.appearance.take();
        if let Some((_, rows)) = self.pending_rows.take() {
            self.retire_rows(rows);
        }
        if let Some(request) = self.pending_scene.take() {
            self.retire(retirement::Payload::Scene(request));
        }
        let latest = std::mem::replace(&mut self.latest_connections, Arc::new(Mutex::new(None)));
        self.retire(retirement::Payload::Latest(latest));
        self.connections.take();
        self.scene.take();
        let (empty_sender, empty_replies) = mpsc::sync_channel(0);
        let replies = std::mem::replace(&mut self.replies, empty_replies);
        self.sender = empty_sender;
        self.retire(retirement::Payload::Replies(replies));
        self.retirement.take();
        let deadline = Instant::now() + allowance;
        #[cfg(test)]
        native_shutdown::finish_started(allowance, deadline);
        while self.workers.iter().any(|worker| !worker.is_finished()) {
            if Instant::now() >= deadline {
                #[cfg(test)]
                native_shutdown::fallback_initiated();
                // SAFETY: Only this process is terminated, after its windows have
                // closed. Stuck workers still own their inputs; no state is freed
                // ahead of native completion. Returning into DLL detach is unsafe.
                unsafe {
                    let _ = TerminateProcess(GetCurrentProcess(), 1);
                }
                std::process::abort();
            }
            thread::sleep(Duration::from_millis(10));
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        #[cfg(test)]
        native_shutdown::finish_completed();
    }
}

// Worker-only: shell extensions may wait or dispatch COM. Callers choose the
// operation explicitly; only the capture command supplies an elevation verb.
fn shell_launch(program: &str, parameters: &str, verb: &str) -> io::Result<()> {
    let _apartment = crate::host::configuration_apartment()
        .map_err(|error| io::Error::other(error.to_string()))?;
    // SAFETY: Owned terminated strings outlive the shell call, with no borrowed
    // UI handles or process wait. Work retains this worker through shutdown.
    let result = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            &windows::core::HSTRING::from(verb),
            &windows::core::HSTRING::from(program),
            &windows::core::HSTRING::from(parameters),
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(io::Error::other(format!(
            "Windows could not launch the command ({})",
            result.0 as isize
        )));
    }
    Ok(())
}

#[derive(Clone)]
struct Reporter {
    sender: mpsc::SyncSender<Reply>,
    cancelled: Arc<AtomicBool>,
    endpoint: usize,
}

impl Reporter {
    fn wake(&self) {
        // SAFETY: Endpoint stays live through worker finalization, including
        // modal loops and presentation replacement. Workers never touch controls.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.endpoint as *mut _)),
                REPLY_MESSAGE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }

    fn send(&self, mut reply: Reply) -> bool {
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return false;
            }
            match self.sender.try_send(reply) {
                Ok(()) => {
                    self.wake();
                    return true;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
                Err(mpsc::TrySendError::Full(pending)) => reply = pending,
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn progress(&self, reply: Reply) {
        if !self.cancelled.load(Ordering::Acquire) && self.sender.try_send(reply).is_ok() {
            self.wake();
        }
    }
}

// This message-only window belongs to the application, not a presentation HWND.
// Its borrow keeps Application alive until workers have stopped and it is destroyed.
pub struct Endpoint<'a> {
    window: HWND,
    owner: PhantomData<&'a Application>,
}

impl<'a> Endpoint<'a> {
    pub fn new(app: &'a Application) -> Result<Self> {
        // SAFETY: Application has a stable Rc allocation on this UI thread.
        // No model borrow spans creation. Drop retires the pointer before app dies.
        unsafe {
            let instance = GetModuleHandleW(None)?.into();
            let class = WNDCLASSW {
                hInstance: instance,
                lpfnWndProc: Some(endpoint_proc),
                lpszClassName: w!("NetMap.Work"),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(Error::from_thread());
            }
            let window = CreateWindowExW(
                Default::default(),
                class.lpszClassName,
                None,
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                Some((app as *const Application).cast()),
            )?;
            Ok(Self {
                window,
                owner: PhantomData,
            })
        }
    }

    pub fn window(&self) -> HWND {
        self.window
    }
}

impl Drop for Endpoint<'_> {
    fn drop(&mut self) {
        // SAFETY: This window stays on its creator thread, after workers finish.
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

unsafe extern "system" fn endpoint_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => {
            // SAFETY: CreateWindowEx supplies this live CREATESTRUCT. Endpoint's
            // borrow guarantees the pointed-to Application outlives this HWND.
            unsafe {
                let creation = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
            }
            return LRESULT(1);
        }
        REPLY_MESSAGE => {
            // SAFETY: Only this procedure owns the slot, retired at NCDESTROY.
            let app = unsafe {
                (GetWindowLongPtrW(window, GWLP_USERDATA) as *const Application).as_ref()
            };
            if let Some(app) = app {
                app.receive_work();
            }
            return LRESULT(0);
        }
        WM_TIMER if wparam.0 == ORIGIN_TIMER => {
            // SAFETY: The endpoint's UI-thread registration outlives app work.
            let app = unsafe {
                (GetWindowLongPtrW(window, GWLP_USERDATA) as *const Application).as_ref()
            };
            if let Some(app) = app {
                app.origin_tick();
            }
            return LRESULT(0);
        }
        WM_NCDESTROY => {
            // SAFETY: Retire native delivery before the borrowed Application dies.
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
        }
        _ => {}
    }
    // SAFETY: Preserve standard behavior for all other message-only HWND messages.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_placement_save_is_admitted_while_other_roles_are_busy() {
        let mut work = Work::new();
        let mut gates = Vec::new();
        for _ in 0..7 {
            let (release, wait) = mpsc::channel::<()>();
            work.spawn("Busy role", move || {
                let _ = wait.recv();
            })
            .unwrap();
            gates.push(release);
        }
        let directory =
            std::env::temp_dir().join(format!("netmap-busy-placement-{}", std::process::id()));
        let placement = Placement {
            left: 100,
            top: 100,
            right: 1000,
            bottom: 700,
            maximized: true,
        };
        let submitted = work.save_window_placement(directory.clone(), placement);
        drop(gates);
        for worker in work.workers.drain(..) {
            worker.join().unwrap();
        }
        submitted.unwrap();
        assert_eq!(placement::load(&directory).unwrap(), Some(placement));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saved_manual_or_unreadable_preferences_never_start_public_discovery() {
        let manual = Application::new();
        let point = crate::geography::Point {
            latitude: 5.8,
            longitude: -55.1,
        };
        manual
            .work
            .borrow()
            .sender
            .try_send(Reply::Preferences(Ok(Preferences {
                origin: super::super::settings::Origin::Manual(point),
                ..Preferences::default()
            })))
            .unwrap();
        manual.receive_work();
        assert!(manual.work.borrow().origin.is_none());
        assert_eq!(manual.origin.borrow().point(None), Some(point));
        assert_eq!(manual.map.viewport().meridian, point.longitude as f32);
        let unreadable = Application::new();
        unreadable
            .work
            .borrow()
            .sender
            .try_send(Reply::Preferences(Err(io::Error::other(
                "Preferences are unreadable",
            ))))
            .unwrap();
        unreadable.receive_work();
        assert!(unreadable.work.borrow().origin.is_none());
        assert!(unreadable.origin.borrow().request().is_none());
        assert_eq!(unreadable.origin.borrow().point(None), None);
    }

    #[test]
    fn queued_network_hint_cannot_rearm_origin_after_cancellation() {
        let app = Application::new();
        app.origin
            .borrow_mut()
            .configure(super::super::settings::Origin::Automatic, Instant::now());
        app.work
            .borrow()
            .network_changed
            .store(true, Ordering::Release);
        let status = app.origin_status();
        let meridian = app.map.viewport().meridian;
        app.cancel_work();
        app.receive_work();
        assert_eq!(app.origin_status(), status);
        assert_eq!(app.map.viewport().meridian, meridian);
        assert!(app.origin_timer.get().is_none());
        assert!(app.work.borrow().origin.is_none());
    }

    #[test]
    fn native_dispatch_delivers_replies_and_retired_work_cannot_update_the_app() {
        let app = Application::new();
        let endpoint = Endpoint::new(&app).unwrap();
        let before = app.status();
        let world = World::decode(&[0; 8]).unwrap();
        app.work
            .borrow()
            .sender
            .send(Reply::World(Ok(Arc::new(world))))
            .unwrap();
        // SAFETY: A native modal loop dispatches an HWND message the same way.
        // Delivery must not require Host's outer message loop to recognize it.
        unsafe {
            DispatchMessageW(&MSG {
                hwnd: endpoint.window(),
                message: REPLY_MESSAGE,
                ..Default::default()
            });
        }
        assert_ne!(app.status(), before);
        let accepted = app.status();
        app.cancel_work();
        app.work
            .borrow()
            .sender
            .send(Reply::World(Err(io::Error::other("retired reply"))))
            .unwrap();
        // SAFETY: Endpoint and Application remain live, but reply acceptance ended.
        unsafe {
            DispatchMessageW(&MSG {
                hwnd: endpoint.window(),
                message: REPLY_MESSAGE,
                ..Default::default()
            });
        }
        assert_eq!(app.status(), accepted);
    }
}
