use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{KillTimer, PostMessageW, SetTimer, WM_APP},
    },
    core::Result,
};

use crate::{
    connections::{Coverage, Highlight, Publication},
    geography::World,
    geolocation::{
        Availability, Report,
        dataset::{Error as DatasetError, Phase},
    },
    host,
    map::{Inspection, Map, Update},
};

mod desktop;
pub(crate) mod exploration;
mod launch;
#[cfg(test)]
pub(crate) mod native_checks;
mod origin;
pub(crate) mod placement;
pub(crate) mod settings;
mod storage;
pub(crate) mod work;
pub const REFRESH_MESSAGE: u32 = WM_APP + 2;

type DesktopResult =
    std::result::Result<(host::desktop::Attachment, Vec<host::desktop::Monitor>), String>;

pub struct Application {
    pub map: Map,
    assets: RefCell<Option<PathBuf>>,
    collection_allowed: Cell<bool>,
    exploration_rows: RefCell<Option<Arc<exploration::Rows>>>,
    desktop_discovery: RefCell<desktop::Discovery>,
    desktop_result: RefCell<Option<DesktopResult>>,
    exploration_filter: RefCell<Option<crate::map::model::Place>>,
    exploration_requested: Cell<bool>,
    monitors: RefCell<Vec<(String, bool)>>,
    saver_status: RefCell<String>,
    saver_pending: Cell<bool>,
    capture_pending: Cell<bool>,
    connection_source: Cell<Option<work::Source>>,
    source_reset: Cell<bool>,
    work: RefCell<work::Work>,
    status: RefCell<String>,
    wake: Cell<HWND>,
    presentation: Cell<Option<HWND>>,
    locations: RefCell<Option<Report>>,
    city: RefCell<City>,
    city_generation: Cell<u64>,
    settings: RefCell<settings::Settings>,
    settings_dialog: Cell<Option<(u64, HWND)>>,
    dialog_generation: Cell<u64>,
    origin: RefCell<origin::Origin>,
    origin_timer: Cell<Option<Instant>>,
    connections: RefCell<Option<Publication>>,
    scene: Cell<Option<SceneKey>>,
    scene_generation: Cell<u64>,
    presented_scene: Cell<Option<u64>>,
    inspection: RefCell<Inspection>,
    placement: RefCell<placement::Restore>,
}

#[derive(Clone, Copy, PartialEq)]
struct SceneKey {
    snapshot: usize,
    world: usize,
    version: u64,
    cities: bool,
    meridian: f32,
    scale: f32,
}

enum City {
    Idle,
    Running { id: u64, phase: Phase },
    Failed(String),
}

pub fn run() -> Result<()> {
    let assets = std::env::current_exe()
        .map_err(|error| {
            windows::core::Error::new(windows::Win32::Foundation::E_FAIL, error.to_string())
        })?
        .parent()
        .unwrap()
        .join("assets");
    let mut launch =
        launch::Launch::parse(std::env::args_os().skip(1), assets).map_err(|error| {
            windows::core::Error::new(windows::Win32::Foundation::E_INVALIDARG, error.to_string())
        })?;
    if std::env::args_os().len() == 1
        && std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.extension()
                    .map(|extension| extension.eq_ignore_ascii_case("scr"))
            })
            .unwrap_or(false)
    {
        launch.mode = launch::Mode::Configure(None);
    }
    let app = Rc::new(Application::new());
    if launch.mode == launch::Mode::Collector {
        return data_directory()
            .and_then(work::Work::run_collector)
            .map_err(|error| {
                windows::core::Error::new(windows::Win32::Foundation::E_FAIL, error.to_string())
            });
    }
    app.work.borrow_mut().start_retirement().map_err(|error| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, error.to_string())
    })?;
    app.assets.replace(Some(launch.assets));
    app.collection_allowed
        .set(!matches!(launch.mode, launch::Mode::Configure(_)));
    let endpoint = work::Endpoint::new(&app)?;
    app.wake.set(endpoint.window());
    let result = match launch.mode {
        launch::Mode::Collector => unreachable!(),
        launch::Mode::Window => host::run(Rc::clone(&app)),
        launch::Mode::ScreenSaver => {
            host::screensaver::run(Rc::clone(&app), host::screensaver::Mode::FullScreen)
        }
        launch::Mode::Preview(parent) => host::screensaver::run(
            Rc::clone(&app),
            host::screensaver::Mode::Preview(HWND(parent as *mut _)),
        ),
        launch::Mode::Configure(owner) => (|| {
            let _apartment = host::configuration_apartment()?;
            app.load_world().map_err(|error| {
                windows::core::Error::new(windows::Win32::Foundation::E_FAIL, error.to_string())
            })?;
            // A supplied configuration owner is borrowed, never destroyed.
            let owner = owner.map_or(HWND::default(), |value| HWND(value as *mut _));
            // SAFETY: Validation is a native handle query; modal code owns no foreign HWND.
            let owner = if unsafe {
                windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(owner)).as_bool()
            } {
                owner
            } else {
                HWND::default()
            };
            settings::show(&app, owner, settings::Page::Map)
        })(),
    };
    app.finish();
    drop(endpoint);
    result
}

impl Application {
    #[cfg(test)]
    pub(crate) fn native_broker_publication(&self) -> Option<(work::Source, Publication)> {
        Some((
            self.connection_source.get()?,
            self.connections.borrow().clone()?,
        ))
    }

    #[cfg(test)]
    pub(crate) fn native_broker_resume(&self) -> std::io::Result<()> {
        self.collection_allowed.set(true);
        let settings = self.settings.borrow();
        self.work.borrow_mut().start_connections(
            Highlight {
                bright: Duration::from_secs(settings.committed.bright_seconds),
                fade: Duration::from_secs(settings.committed.fade_seconds),
            },
            self.wake.get(),
        )
    }

    #[cfg(test)]
    pub(crate) fn native_broker_pause(&self) -> std::io::Result<()> {
        // The native check calls this outside callbacks and retains the same
        // accepted facts, selection and Endpoint across source replacement.
        self.collection_allowed.set(false);
        self.work.borrow_mut().native_broker_restart()
    }

    #[cfg(test)]
    pub(crate) fn native_broker_traffic(
        &self,
        body: impl FnOnce(Arc<std::sync::atomic::AtomicBool>) + Send + 'static,
    ) -> std::io::Result<()> {
        self.work.borrow_mut().native_broker_traffic(body)
    }

    fn finish(&self) {
        if let Some(rows) = self.exploration_rows.take() {
            self.retire_rows(rows);
        }
        if let Some(publication) = self.connections.take() {
            self.work.borrow().retire_publication(publication);
        }
        self.present_model(Arc::default(), Duration::ZERO, Instant::now());
        if let Some(world) = self.map.take_world() {
            self.work.borrow().retire_world(world);
        }
        self.work.borrow_mut().finish(Duration::from_secs(5));
    }

    pub fn refresh_appearance(&self) {
        self.work.borrow().refresh_appearance();
    }

    fn new() -> Self {
        Self {
            map: Map::default(),
            assets: RefCell::new(None),
            collection_allowed: Cell::new(true),
            exploration_rows: RefCell::new(None),
            desktop_discovery: RefCell::new(desktop::Discovery::default()),
            desktop_result: RefCell::new(None),
            exploration_filter: RefCell::new(None),
            exploration_requested: Cell::new(false),
            monitors: RefCell::new(Vec::new()),
            saver_status: RefCell::new("Checking screen saver availability…".into()),
            saver_pending: Cell::new(false),
            capture_pending: Cell::new(false),
            connection_source: Cell::new(None),
            source_reset: Cell::new(false),
            work: RefCell::new(work::Work::new()),
            status: RefCell::new("Preparing the map and connection collection…".into()),
            wake: Cell::new(HWND::default()),
            presentation: Cell::new(None),
            locations: RefCell::new(None),
            city: RefCell::new(City::Idle),
            city_generation: Cell::new(0),
            settings: RefCell::new(settings::Settings::default()),
            settings_dialog: Cell::new(None),
            dialog_generation: Cell::new(0),
            origin: RefCell::new(origin::Origin::default()),
            origin_timer: Cell::new(None),
            connections: RefCell::new(None),
            scene: Cell::new(None),
            scene_generation: Cell::new(0),
            presented_scene: Cell::new(None),
            inspection: RefCell::new(Inspection::default()),
            placement: RefCell::new(placement::Restore::Pending),
        }
    }

    pub fn load_world(&self) -> std::io::Result<()> {
        let directory = self
            .assets
            .borrow()
            .clone()
            .unwrap_or(std::env::current_exe()?.parent().unwrap().join("assets"));
        self.work
            .borrow_mut()
            .load_world(directory, data_directory()?, self.wake.get())
    }

    fn receive_work(&self) {
        if self.work.borrow().retired() {
            return;
        }
        let now = Instant::now();
        let network_changed = self.work.borrow().network_changed();
        if network_changed {
            self.origin.borrow_mut().network_changed(now);
        }
        let reply = self.work.borrow_mut().reply();
        let Some(reply) = reply else {
            if network_changed {
                let redraw = self.reconcile_origin(now);
                self.refresh_presentation(redraw);
                self.refresh_settings();
            }
            return;
        };
        self.accept_reply(reply, now);
    }

    fn accept_reply(&self, reply: work::Reply, now: Instant) {
        let mut redraw = false;
        match reply {
            work::Reply::CaptureCommand(result) => {
                self.capture_pending.set(false);
                self.status.replace(match result { Ok(()) => "Whole-PC collection was requested. Waiting for the collector's verified snapshot…".into(), Err(error) => format!("Could not enable whole-PC collection: {error}") });
            }
            work::Reply::HostingInfo { monitors, saver } => {
                self.monitors.replace(monitors);
                self.saver_status.replace(saver);
            }
            work::Reply::SaverAction(result) => {
                self.saver_pending.set(false);
                self.saver_status.replace(match result { Ok(()) => "Windows command launched. Screen saver selection and installation still require verification in Windows.".into(), Err(error) => error });
            }
            work::Reply::Desktop { id, result } => self.receive_desktop(id, result),
            work::Reply::Placement(result) => {
                let value = match result {
                    Ok(value) => value,
                    Err(error) => {
                        self.status
                            .replace(format!("Window placement unavailable: {error}"));
                        None
                    }
                };
                *self.placement.borrow_mut() = placement::Restore::Ready(value);
            }
            work::Reply::Palette(palette) => {
                let mut appearance = self.map.appearance();
                appearance.palette = palette;
                redraw |= self.map.set_appearance(appearance);
            }
            work::Reply::Connections(Ok(update)) => {
                if self.connection_source.replace(Some(update.source)) != Some(update.source) {
                    self.scene.set(None);
                    self.presented_scene.set(None);
                    self.exploration_filter.take();
                    self.inspection.replace(Inspection::default());
                    self.source_reset.set(true);
                    redraw |= self.present_model(
                        Arc::default(),
                        update.publication.snapshot.interval.end,
                        update.publication.sampled,
                    );
                }
                if let Some(previous) = self.exploration_rows.replace(Some(update.rows)) {
                    self.retire_rows(previous);
                }
                let publication = update.publication;
                let snapshot = &publication.snapshot;
                let traffic = if let Some(rates) = snapshot.rates {
                    format!(
                        "↓ {:.0} B/s · ↑ {:.0} B/s. Observed {} bytes received, {} sent.",
                        rates.received,
                        rates.sent,
                        snapshot.total_bytes.received,
                        snapshot.total_bytes.sent
                    )
                } else if snapshot.bytes.active() {
                    format!(
                        "Captured {} bytes received, {} sent this interval; rates unavailable.",
                        snapshot.bytes.received, snapshot.bytes.sent
                    )
                } else {
                    "Traffic rates unavailable.".into()
                };
                let coverage = match snapshot.coverage {
                    Coverage::Complete => "Capture complete".into(),
                    Coverage::WarmingUp => "Capture warming up".into(),
                    Coverage::Restricted => "Capture restricted".into(),
                    Coverage::Lost { events, buffers } => {
                        format!("Capture lost {events} events / {buffers} buffers")
                    }
                };
                let uncertain = if snapshot.ambiguous_events != 0 {
                    format!(
                        " {} ambiguous records; {} received / {} sent bytes unattributed.",
                        snapshot.ambiguous_events,
                        snapshot.unattributed.received,
                        snapshot.unattributed.sent
                    )
                } else {
                    String::new()
                };
                *self.status.borrow_mut() = format!(
                    "{coverage} · {} observed connections ({} TCP, {} ended TCP, {} UDP, {} inactive UDP); {} transferring with measured traffic. {traffic}{uncertain}{}",
                    snapshot.observations.len(),
                    snapshot.counts.tcp,
                    snapshot.counts.ended_tcp,
                    snapshot.counts.udp,
                    snapshot.counts.inactive_udp,
                    snapshot.counts.transferring,
                    publication
                        .limitation
                        .as_ref()
                        .map_or(String::new(), |reason| format!(" {reason}"))
                );
                if let Some(previous) = self.connections.replace(Some(publication)) {
                    self.work.borrow().retire_publication(previous);
                }
            }
            work::Reply::Connections(Err(error)) => {
                *self.status.borrow_mut() = format!("Connection collection unavailable: {error}");
            }
            work::Reply::Scene {
                id,
                model,
                rows,
                at,
                sampled,
            } => {
                #[cfg(test)]
                native_checks::observed_scene(
                    id,
                    id == self.scene_generation.get(),
                    Arc::as_ptr(&model) as usize,
                );
                if id == self.scene_generation.get() {
                    if let Some(source) = self.connection_source.get() {
                        self.work
                            .borrow_mut()
                            .configure_rows(source, Arc::clone(&rows));
                    }
                    if let Some(previous) = self.exploration_rows.replace(Some(rows)) {
                        self.retire_rows(previous);
                    }
                    redraw |= self.present_model(model, at, sampled);
                    self.presented_scene.set(Some(id));
                    self.reconcile_inspection();
                } else {
                    self.retire_rows(rows);
                    self.work.borrow().retire_model(model);
                }
            }
            work::Reply::Preferences(result) => {
                let loading = matches!(self.settings.borrow().phase, settings::Phase::Loading);
                let success = result.is_ok();
                self.settings.borrow_mut().loaded(result);
                if loading {
                    if success {
                        let choice = self.settings.borrow().committed.origin;
                        self.origin.borrow_mut().configure(choice, now);
                    } else {
                        self.origin.borrow_mut().preferences_failed();
                    }
                }
            }
            work::Reply::PreferencesSaved { id, result } => {
                let before = self.settings.borrow().committed.origin;
                let accepted = self.settings.borrow_mut().saved(id, result);
                let settings = self.settings.borrow();
                let choice = settings.committed.origin;
                let success = accepted && settings.error.is_none();
                drop(settings);
                if success && (choice != before || self.origin.borrow().needs_preferences()) {
                    self.origin.borrow_mut().configure(choice, now);
                }
            }
            work::Reply::OriginAddress { id, result } => {
                self.origin.borrow_mut().discovered(id, result, now)
            }
            work::Reply::OriginLocation {
                id,
                version,
                result,
            } => self.origin.borrow_mut().located(id, version, result, now),
            work::Reply::NetworkWatch(result) => self.origin.borrow_mut().watch_result(result),
            work::Reply::World(Ok(world)) => {
                self.accept_world(world);
                redraw = true;
            }
            work::Reply::World(Err(error)) => {
                *self.status.borrow_mut() = format!("Could not load assets/world.bin: {error}");
            }
            work::Reply::Locations(report) => {
                if self
                    .locations
                    .borrow()
                    .as_ref()
                    .is_none_or(|current| report.version >= current.version)
                {
                    self.origin.borrow_mut().dataset_changed(report.version);
                    self.locations.replace(Some(report));
                }
            }
            work::Reply::CityProgress { id, phase } => {
                if matches!(*self.city.borrow(), City::Running { id: current, .. } if current == id)
                {
                    self.city.replace(City::Running { id, phase });
                }
            }
            work::Reply::CityFinished { id, result } => {
                if matches!(*self.city.borrow(), City::Running { id: current, .. } if current == id)
                {
                    self.city.replace(match result {
                        Ok(()) | Err(DatasetError::Cancelled) => City::Idle,
                        Err(error) => City::Failed(error.to_string()),
                    });
                }
            }
        }
        redraw |= self.reconcile_origin(now);
        {
            let settings = self.settings.borrow();
            let highlight = Highlight {
                bright: Duration::from_secs(settings.committed.bright_seconds),
                fade: Duration::from_secs(settings.committed.fade_seconds),
            };
            if self.map.set_highlight(highlight) {
                self.work.borrow_mut().configure_connections(highlight);
                redraw = true;
            }
            redraw |= self.map.set_speeds(crate::map::Speeds {
                corner: settings.committed.speeds,
                rates: self
                    .connections
                    .borrow()
                    .as_ref()
                    .and_then(|publication| publication.snapshot.rates),
            });
            if self.collection_allowed.get()
                && !self.wake.get().is_invalid()
                && let Err(error) = self
                    .work
                    .borrow_mut()
                    .start_connections(highlight, self.wake.get())
            {
                *self.status.borrow_mut() =
                    format!("Could not start connection collection: {error}");
            }
        }
        self.prepare_scene();
        self.refresh_presentation(redraw);
        self.refresh_settings();
    }

    fn refresh_presentation(&self, redraw: bool) {
        if let Some(window) = self.presentation.get() {
            // SAFETY: Host attaches/retires this borrowed HWND on this thread.
            // Posting dispatches after accepted state has been fully applied.
            unsafe {
                let _ = PostMessageW(
                    Some(window),
                    REFRESH_MESSAGE,
                    WPARAM(usize::from(redraw)),
                    LPARAM(0),
                );
            }
        }
    }

    pub fn set_presentation(&self, window: Option<HWND>) {
        self.presentation.set(window);
    }

    fn accept_world(&self, world: Arc<World>) {
        let countries = world.countries.len();
        self.map.set_world(world);
        *self.status.borrow_mut() =
            format!("{countries} countries loaded. Starting connection collection…");
    }

    pub fn prepare_scene(&self) {
        let Some(world) = self.map.world() else {
            return;
        };
        let Some((snapshot, sampled, names)) =
            self.connections.borrow().as_ref().map(|publication| {
                (
                    Arc::clone(&publication.snapshot),
                    publication.sampled,
                    Arc::clone(&publication.names),
                )
            })
        else {
            return;
        };
        let Some(version) = self
            .locations
            .borrow()
            .as_ref()
            .map(|report| report.version)
        else {
            return;
        };
        let viewport = self.map.viewport();
        let cities = self.settings.borrow().committed.detail == settings::Detail::Cities;
        let key = SceneKey {
            snapshot: Arc::as_ptr(&snapshot) as usize,
            world: Arc::as_ptr(&world) as usize,
            version,
            cities,
            meridian: viewport.meridian,
            scale: viewport.scale,
        };
        if self.scene.get() == Some(key) {
            return;
        }
        let id = self.scene_generation.get() + 1;
        let result = self.work.borrow_mut().prepare_scene(
            work::scene::Request {
                id,
                world,
                snapshot,
                sampled,
                names,
                cities,
                meridian: viewport.meridian,
                scale: viewport.scale,
                version,
            },
            self.wake.get(),
        );
        match result {
            Ok(()) => {
                self.scene.set(Some(key));
                self.scene_generation.set(id);
            }
            Err(error) => *self.status.borrow_mut() = format!("Could not prepare the map: {error}"),
        }
    }

    pub fn status(&self) -> String {
        let mut status = self.status.borrow().clone();
        if let Some(error) = &self.settings.borrow().error {
            status.push_str(&format!(" {error}"));
        }
        if let Some(publication) = self.connections.borrow().as_ref() {
            let current = self.scene.get().is_some_and(|key| {
                key.snapshot == Arc::as_ptr(&publication.snapshot) as usize
                    && self
                        .locations
                        .borrow()
                        .as_ref()
                        .is_some_and(|report| report.version == key.version)
            });
            if current && self.presented_scene.get() == Some(self.scene_generation.get()) {
                let unlocated = self.map.unlocated();
                let located = publication
                    .snapshot
                    .observations
                    .len()
                    .saturating_sub(unlocated);
                status.push_str(&format!(" {located} located; {unlocated} unmapped."));
            } else {
                status.push_str(" Locations updating.");
            }
        }
        status.push(' ');
        status.push_str(&self.origin_status());
        if let Some(report) = &*self.locations.borrow() {
            match &report.country {
                Availability::Missing => status.push_str(" Country data is missing."),
                Availability::Failed(error) => {
                    status.push_str(&format!(" Country data unavailable: {error}"))
                }
                Availability::Ready { .. } => {}
            }
            match &report.city {
                Availability::Ready { built } => {
                    status.push_str(&format!(" City data ready ({built})."))
                }
                Availability::Failed(error) => {
                    status.push_str(&format!(" City data unavailable: {error}"))
                }
                Availability::Missing => {}
            }
        }
        if let Some(description) = self.inspection_description() {
            status = format!("{description}. {status}");
        }
        self.city_status()
            .map(|city| format!("{city} {status}"))
            .unwrap_or(status)
    }

    pub fn paint_map(&self, width: u32, height: u32, dpi: u32) -> Result<crate::map::Frame> {
        let inspection = self.inspection.borrow().clone();
        self.map.paint(width, height, dpi, inspection)
    }

    pub fn inspection_description(&self) -> Option<String> {
        self.map.describe(&self.inspection.borrow())
    }

    fn reconcile_inspection(&self) {
        let mut inspection = self.inspection.borrow_mut();
        if inspection
            .pointed
            .as_ref()
            .is_some_and(|key| self.map.place(key).is_none())
        {
            inspection.pointed = None;
        }
        if inspection
            .selected
            .as_ref()
            .is_some_and(|place| !self.map.has_place(place))
        {
            inspection.selected = None;
        }
    }

    pub fn hover_map(&self, position: Option<windows_numerics::Vector2>) {
        let mut inspection = self.inspection.borrow().clone();
        inspection.pointed = position.and_then(|point| self.map.hit(point));
        self.set_inspection(inspection);
    }

    pub fn choose_map(&self, position: windows_numerics::Vector2) {
        let pointed = self.map.hit(position);
        let selected = pointed.as_ref().and_then(|key| self.map.place(key));
        self.exploration_filter.replace(selected.clone());
        self.exploration_requested.set(selected.is_some());
        self.set_inspection(Inspection { pointed, selected });
        self.refresh_presentation(false);
    }

    fn set_inspection(&self, inspection: Inspection) {
        {
            let mut current = self.inspection.borrow_mut();
            if *current == inspection {
                return;
            }
            *current = inspection;
        }
        self.refresh_presentation(true);
    }

    pub fn clear_inspection(&self) {
        let filtered = self.exploration_filter.take().is_some();
        self.set_inspection(Inspection::default());
        if filtered {
            self.refresh_presentation(false);
        }
    }

    pub fn exploration_rows(&self) -> Option<Arc<exploration::Rows>> {
        self.exploration_rows.borrow().clone()
    }

    pub(crate) fn retire_rows(&self, rows: Arc<exploration::Rows>) {
        self.work.borrow().retire_rows(rows);
    }

    fn present_model(
        &self,
        model: Arc<crate::map::model::Model>,
        at: Duration,
        sampled: Instant,
    ) -> bool {
        let (changed, previous) = self.map.set_model(model, at, sampled);
        self.work.borrow().retire_model(previous);
        changed
    }

    pub fn selected_place(&self) -> Option<crate::map::model::Place> {
        self.inspection.borrow().selected.clone()
    }

    pub fn reconcile_row_place(&self, selected: Option<crate::map::model::Place>) {
        if self.selected_place() != selected {
            let mut inspection = self.inspection.borrow().clone();
            inspection.selected = selected;
            self.set_inspection(inspection);
        }
    }

    pub fn select_place(&self, selected: Option<crate::map::model::Place>) {
        self.set_inspection(Inspection {
            pointed: None,
            selected,
        });
    }

    pub fn exploration_filter(&self) -> Option<crate::map::model::Place> {
        self.exploration_filter.borrow().clone()
    }

    pub fn take_exploration_request(&self) -> bool {
        self.exploration_requested.replace(false)
    }

    pub fn take_source_reset(&self) -> bool {
        self.source_reset.replace(false)
    }

    pub fn discover_desktop(&self) {
        // A ready answer may still be awaiting Host's posted refresh. Retire it
        // before accepting a new topology request, not just while Work is busy.
        self.desktop_result.take();
        let next = self.desktop_discovery.borrow_mut().request();
        if let Some(id) = next {
            self.start_desktop_discovery(id);
        }
    }

    fn start_desktop_discovery(&self, id: u64) {
        let result = self.work.borrow_mut().discover_desktop(id, self.wake.get());
        if let Err(error) = result {
            self.receive_desktop(id, Err(error.to_string()));
            self.refresh_presentation(false);
        }
    }

    fn receive_desktop(&self, id: u64, result: DesktopResult) {
        let completion = self.desktop_discovery.borrow_mut().complete(id);
        match completion {
            desktop::Completion::Ignore => {}
            desktop::Completion::Refresh(next) => self.start_desktop_discovery(next),
            desktop::Completion::Publish => {
                self.desktop_result.replace(Some(result));
            }
        }
    }

    pub fn take_desktop_result(
        &self,
    ) -> Option<std::result::Result<(host::desktop::Attachment, Vec<host::desktop::Monitor>), String>>
    {
        self.desktop_result.take()
    }

    pub fn inspect_next(&self, backwards: bool) -> Update {
        let next = self.map.cycle(&self.inspection.borrow(), backwards);
        let Some(next) = next else {
            return Update {
                changed: false,
                capture: crate::map::Capture::Keep,
            };
        };
        self.set_inspection(Inspection {
            selected: self.map.place(&next),
            pointed: Some(next.clone()),
        });
        self.map.reveal(&next)
    }

    pub fn zoom_inspection(&self, steps: f64) -> Update {
        self.map.zoom_inspection(&self.inspection.borrow(), steps)
    }

    pub fn desktop_monitors(&self) -> Vec<(String, bool)> {
        self.monitors.borrow().clone()
    }

    pub fn screen_saver_status(&self) -> String {
        self.saver_status.borrow().clone()
    }

    pub fn start_capture(&self) {
        if self.capture_pending.get() {
            return;
        }
        let result = self.work.borrow_mut().start_capture(self.wake.get());
        match result {
            Ok(()) => {
                self.capture_pending.set(true);
                self.status
                    .replace("Requesting permission for whole-PC collection…".into());
            }
            Err(error) => {
                self.status.replace(error.to_string());
            }
        }
        self.refresh_presentation(false);
    }

    pub fn screen_saver_action(&self, action: settings::SaverAction) {
        if self.saver_pending.get() {
            return;
        }
        let result = self
            .work
            .borrow_mut()
            .screen_saver_action(action, self.wake.get());
        match result {
            Ok(()) => {
                self.saver_pending.set(true);
                self.saver_status
                    .replace("Opening the requested Windows command…".into());
            }
            Err(error) => {
                self.saver_status.replace(error.to_string());
            }
        }
        self.refresh_settings();
    }

    pub fn city_status(&self) -> Option<String> {
        match &*self.city.borrow() {
            City::Idle => None,
            City::Failed(error) => Some(format!("City data: {error}.")),
            City::Running { phase, .. } => Some(match phase {
                Phase::Downloading { bytes, total } => format!(
                    "Downloading city data: {:.1} MiB{}.",
                    *bytes as f64 / 1048576.0,
                    total
                        .map(|total| format!(" of {:.1} MiB", total as f64 / 1048576.0))
                        .unwrap_or_default()
                ),
                Phase::Unpacking { bytes } => {
                    format!("Unpacking city data: {:.1} MiB.", *bytes as f64 / 1048576.0)
                }
                Phase::Validating => "Validating city data…".into(),
                Phase::Installing => "Installing city data…".into(),
            }),
        }
    }

    pub fn can_download_city(&self) -> bool {
        self.locations.borrow().is_some() && !matches!(*self.city.borrow(), City::Running { .. })
    }

    pub fn can_cancel_city(&self) -> bool {
        matches!(
            *self.city.borrow(),
            City::Running {
                phase: Phase::Downloading { .. } | Phase::Unpacking { .. } | Phase::Validating,
                ..
            }
        )
    }

    pub fn city_available(&self) -> bool {
        self.locations
            .borrow()
            .as_ref()
            .is_some_and(|report| matches!(report.city, Availability::Ready { .. }))
    }

    pub fn download_city(&self) {
        if !self.can_download_city() {
            return;
        }
        let id = self.city_generation.get() + 1;
        self.city_generation.set(id);
        let result = data_directory().and_then(|directory| {
            self.work
                .borrow_mut()
                .download_city(id, directory, self.wake.get())
        });
        self.city.replace(match result {
            Ok(()) => City::Running {
                id,
                phase: Phase::Downloading {
                    bytes: 0,
                    total: None,
                },
            },
            Err(error) => City::Failed(error.to_string()),
        });
        self.refresh_presentation(false);
        self.refresh_settings();
    }

    pub fn cancel_city(&self) {
        if self.can_cancel_city() {
            self.work.borrow().cancel_city();
        }
    }

    pub fn cancel_work(&self) {
        self.work.borrow().cancel();
        self.arm_origin(None);
    }

    pub fn origin_status(&self) -> String {
        self.origin.borrow().status()
    }

    fn reconcile_origin(&self, now: Instant) -> bool {
        let request = self.origin.borrow().request();
        let result = self
            .work
            .borrow_mut()
            .ensure_origin(request, self.wake.get());
        if let Err(error) = result
            && let Some(request) = request
        {
            self.origin
                .borrow_mut()
                .submission_failed(request, error.to_string(), now);
        }
        let world = self.map.world();
        let point = self.origin.borrow().point(world.as_deref());
        let update = self.map.set_origin(point);
        if update.changed {
            self.inspection.borrow_mut().pointed = None;
        }
        let deadline = self.origin.borrow().deadline();
        self.arm_origin(deadline);
        update.changed
    }

    fn arm_origin(&self, deadline: Option<Instant>) {
        if self.origin_timer.get() == deadline {
            return;
        }
        let window = self.wake.get();
        // SAFETY: One application-lived endpoint timer represents the absolute
        // debounce/retry deadline. It does not drive presentation or poll idle.
        unsafe {
            if self.origin_timer.take().is_some() {
                let _ = KillTimer(Some(window), work::ORIGIN_TIMER);
            }
            if let Some(deadline) = deadline {
                let milliseconds = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .clamp(1, u32::MAX as u128) as u32;
                if SetTimer(Some(window), work::ORIGIN_TIMER, milliseconds, None) == 0 {
                    self.origin.borrow_mut().timer_failed();
                } else {
                    self.origin_timer.set(Some(deadline));
                }
            }
        }
    }

    fn origin_tick(&self) {
        if self.work.borrow().retired() {
            return;
        }
        self.arm_origin(None);
        let now = Instant::now();
        self.origin.borrow_mut().tick(now);
        let redraw = self.reconcile_origin(now);
        self.refresh_presentation(redraw);
        self.refresh_settings();
    }

    pub fn preferences(&self) -> settings::Settings {
        self.settings.borrow().clone()
    }

    pub fn can_choose_detail(&self, detail: settings::Detail) -> bool {
        let settings = self.settings.borrow();
        matches!(settings.phase, settings::Phase::Ready)
            && !self.work.borrow().retired()
            && (detail == settings::Detail::Countries
                || self.city_available()
                || settings.committed.detail == detail)
    }

    pub fn choose_detail(&self, detail: settings::Detail) {
        if !self.can_choose_detail(detail) {
            return;
        }
        let mut desired = self.settings.borrow().committed.clone();
        if desired.detail != detail {
            desired.detail = detail;
            self.save_preferences(desired);
        }
    }

    pub fn take_window_placement(&self) -> Option<Option<placement::Placement>> {
        let mut state = self.placement.borrow_mut();
        if !matches!(*state, placement::Restore::Ready(_)) {
            return None;
        }
        match std::mem::replace(&mut *state, placement::Restore::Applied) {
            placement::Restore::Ready(value) => Some(value),
            _ => unreachable!(),
        }
    }

    pub fn save_window_placement(&self, value: placement::Placement) {
        if !matches!(*self.placement.borrow(), placement::Restore::Applied) {
            return;
        }
        let result = data_directory().and_then(|directory| {
            self.work
                .borrow_mut()
                .save_window_placement(directory, value)
        });
        if let Err(error) = result {
            self.status
                .replace(format!("Window placement could not be saved: {error}"));
        }
    }

    pub fn save_preferences(&self, desired: settings::Preferences) -> Option<u64> {
        let previous = self.settings.borrow().committed.clone();
        if settings::Draft::from(&desired)
            .validate(self.city_available(), &previous)
            .is_err()
        {
            return None;
        }
        let id = self.settings.borrow_mut().begin_save(desired.clone())?;
        let result = data_directory().and_then(|directory| {
            self.work.borrow_mut().save_preferences(
                id,
                directory,
                previous,
                desired,
                self.wake.get(),
            )
        });
        if let Err(error) = result {
            self.settings.borrow_mut().saved(id, Err(error));
        }
        self.refresh_settings();
        Some(id)
    }

    fn refresh_settings(&self) {
        if let Some((id, window)) = self.settings_dialog.get() {
            // SAFETY: The modal dialog registers and retires this borrowed HWND
            // on the UI thread; identity rejects messages after handle reuse.
            unsafe {
                let _ = PostMessageW(
                    Some(window),
                    settings::REFRESH,
                    WPARAM(id as usize),
                    LPARAM(0),
                );
            }
        }
    }

    pub fn location_status(&self) -> String {
        let describe = |name, availability: &Availability| match availability {
            Availability::Missing => format!("{name} data is not installed."),
            Availability::Ready { built } => {
                format!("{name} data: {built}. DB-IP Lite, CC BY 4.0.")
            }
            Availability::Failed(error) => format!("{name} data unavailable: {error}"),
        };
        self.locations
            .borrow()
            .as_ref()
            .map(|report| {
                format!(
                    "{}\r\n{}",
                    describe("Country", &report.country),
                    describe("City", &report.city)
                )
            })
            .unwrap_or_else(|| "Loading location data…".into())
    }
}

fn data_directory() -> std::io::Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|path| PathBuf::from(path).join("NetMap"))
        .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_transitions_clear_hover_and_preserve_place_selection() {
        use crate::{
            connections::*,
            geography::Point,
            geolocation::Location,
            map::model::{Located, Model},
        };
        let app = Application::new();
        let world = World::decode(&[0; 8]).unwrap();
        let location = Location {
            code: "US".into(),
            country: "United States".into(),
            city: None,
            point: Some(Point {
                latitude: 40.0,
                longitude: -100.0,
            }),
        };
        let observation = Observation {
            id: ObservationId(1),
            tuple: Tuple {
                process: ProcessId {
                    pid: 1,
                    generation: 1,
                },
                local: Endpoint::new("127.0.0.1:1".parse().unwrap()),
                remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
            },
            life: Life::Tcp(TcpState::Established),
            introduced: Duration::ZERO,
            bytes: Bytes::default(),
            rates: None,
        };
        let model = Model::arrange(
            &world,
            [Located {
                observation: &observation,
                location: Some(&location),
            }],
            true,
            0.0,
            1.0,
        );
        let selected = model.groups[0].place.clone();
        for longitude in [20.0, -40.0] {
            *app.inspection.borrow_mut() = Inspection {
                pointed: Some(model.destinations[0].key.clone()),
                selected: Some(selected.clone()),
            };
            app.origin.borrow_mut().configure(
                settings::Origin::Manual(Point {
                    latitude: 50.0,
                    longitude,
                }),
                Instant::now(),
            );
            assert!(app.reconcile_origin(Instant::now()));
            assert!(app.inspection.borrow().pointed.is_none());
            assert_eq!(app.inspection.borrow().selected.as_ref(), Some(&selected));
        }
        app.cancel_work();
    }

    #[test]
    fn dataset_status_survives_the_end_of_cancel_eligibility() {
        let app = Application::new();
        app.city.replace(City::Running {
            id: 1,
            phase: Phase::Installing,
        });
        assert!(!app.can_cancel_city());
        assert!(app.city_status().is_some());
        app.city.replace(City::Failed("Disk is full".into()));
        assert!(!app.can_cancel_city());
        assert!(app.city_status().unwrap().contains("Disk is full"));
    }
}
