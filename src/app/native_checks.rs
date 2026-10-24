//! Deterministic inputs for native integration checks, absent from product builds.

use super::*;
use crate::{
    connections::{
        Bytes, Counts, Endpoint, Life, Observation, ObservationId, ProcessId, Snapshot, TcpState,
        Tuple, Window,
    },
    geography::Point,
    geolocation::Location,
    map::model::{Located, Model},
};
use std::collections::HashMap;
use windows::{Win32::Foundation::E_FAIL, core::Error};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneCounts {
    pub accepted: u32,
    pub discarded: u32,
    pub last_accepted: Option<u64>,
    pub last_accepted_model: usize,
    pub last_discarded: Option<u64>,
}

thread_local! { static SCENES: Cell<Option<SceneCounts>> = const { Cell::new(None) }; }

pub(crate) struct SceneObserver;

pub(crate) fn observe_scenes() -> SceneObserver {
    SCENES.set(Some(SceneCounts::default()));
    SceneObserver
}

impl SceneObserver {
    pub(crate) fn counts(&self) -> SceneCounts {
        SCENES.get().unwrap_or_default()
    }
}

impl Drop for SceneObserver {
    fn drop(&mut self) {
        SCENES.set(None);
    }
}

pub(crate) fn observed_scene(id: u64, accepted: bool, model: usize) {
    if let Some(mut counts) = SCENES.get() {
        if accepted {
            counts.accepted += 1;
            counts.last_accepted = Some(id);
            counts.last_accepted_model = model;
        } else {
            counts.discarded += 1;
            counts.last_discarded = Some(id);
        }
        SCENES.set(Some(counts));
    }
}

pub(crate) fn accepted_scene(app: &Application) -> Option<u64> {
    app.presented_scene.get()
}

pub(crate) fn scene_current(app: &Application) -> bool {
    app.presented_scene.get() == Some(app.scene_generation.get())
}

pub(crate) fn location_report(app: &Application) -> Option<crate::geolocation::Report> {
    app.locations.borrow().clone()
}

pub(crate) fn origin_point(app: &Application) -> Option<Point> {
    app.origin.borrow().point(app.map.world().as_deref())
}

/// Supply a controlled discovery answer without submitting public HTTP. Lookup,
/// origin reconciliation and scene preparation then use the ordinary owners.
pub(crate) fn seed_local_origin(
    app: &Application,
    address: std::net::IpAddr,
) -> Result<origin::Request> {
    let now = Instant::now();
    {
        let mut settings = app.settings.borrow_mut();
        settings.committed.origin = settings::Origin::Automatic;
        settings.committed.detail = settings::Detail::Cities;
    }
    let request = {
        let mut origin = app.origin.borrow_mut();
        origin.configure(settings::Origin::Automatic, now);
        let Some(origin::Request::Discover { id }) = origin.request() else {
            return Err(Error::new(
                E_FAIL,
                "Automatic origin did not request discovery",
            ));
        };
        origin.discovered(id, Ok(address), now);
        origin
            .request()
            .ok_or_else(|| Error::new(E_FAIL, "Local origin requires accepted location data"))?
    };
    if !matches!(request, origin::Request::Locate { .. }) {
        return Err(Error::new(
            E_FAIL,
            "Fixture must submit only a local origin lookup",
        ));
    }
    app.reconcile_origin(now);
    app.prepare_scene();
    Ok(request)
}

pub(crate) fn sample_anchor(app: &Application) -> Option<(Duration, Instant)> {
    app.connections
        .borrow()
        .as_ref()
        .map(|publication| (publication.snapshot.interval.end, publication.sampled))
}

pub(crate) fn set_origin(app: &Application, point: Option<Point>) -> bool {
    let now = Instant::now();
    match point {
        Some(point) => {
            let choice = settings::Origin::Manual(point);
            app.settings.borrow_mut().committed.origin = choice;
            app.origin.borrow_mut().configure(choice, now);
        }
        None => app.origin.borrow_mut().preferences_failed(),
    }
    app.reconcile_origin(now)
}

pub(crate) fn set_highlight(app: &Application, bright: Duration, fade: Duration) {
    {
        let mut settings = app.settings.borrow_mut();
        settings.committed.bright_seconds = bright.as_secs();
        settings.committed.fade_seconds = fade.as_secs();
    }
    app.map.set_highlight(Highlight { bright, fade });
}

pub(crate) fn run() -> Result<()> {
    let app = Rc::new(Application::new());
    app.collection_allowed.set(false);
    app.placement.replace(placement::Restore::Ready(None));
    let origin = settings::Origin::Manual(Point {
        latitude: 0.0,
        longitude: 0.0,
    });
    app.settings.borrow_mut().loaded(Ok(settings::Preferences {
        origin,
        ..Default::default()
    }));
    app.origin.borrow_mut().configure(origin, Instant::now());
    // Setup precedes Host/endpoint creation. No file I/O runs in a UI callback,
    // and no extra asset is embedded in the resource-measurement executable.
    {
        let bytes =
            std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/world.bin"))
                .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
        app.map.set_world(Arc::new(
            World::decode(&bytes).map_err(|error| Error::new(E_FAIL, error.to_string()))?,
        ));
    }
    app.work
        .borrow_mut()
        .start_retirement()
        .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
    let endpoint = work::Endpoint::new(&app)?;
    app.wake.set(endpoint.window());
    let result = host::run(Rc::clone(&app));
    app.finish();
    drop(endpoint);
    result
}

pub(crate) fn observations() -> Vec<Observation> {
    [
        (10, 101, 40000, "8.8.8.8:443", Life::EndedTcp),
        (
            11,
            101,
            40000,
            "8.8.8.8:443",
            Life::Tcp(TcpState::Established),
        ),
        (
            12,
            202,
            40001,
            "1.1.1.1:443",
            Life::Tcp(TcpState::Established),
        ),
        (
            13,
            303,
            40002,
            "127.0.0.1:9000",
            Life::Tcp(TcpState::Established),
        ),
        (
            14,
            202,
            40003,
            "9.9.9.9:443",
            Life::Tcp(TcpState::Established),
        ),
    ]
    .into_iter()
    .map(|(id, pid, port, remote, life)| Observation {
        id: ObservationId(id),
        tuple: Tuple {
            process: ProcessId { pid, generation: 1 },
            local: Endpoint::new(format!("192.0.2.1:{port}").parse().unwrap()),
            remote: Endpoint::new(remote.parse().unwrap()),
        },
        life,
        introduced: Duration::from_secs(100),
        bytes: Bytes::default(),
        rates: None,
    })
    .collect()
}

pub(crate) fn facts(
    app: &Application,
    observations: Vec<Observation>,
    at: u64,
    source: u64,
    previous: Option<&exploration::Rows>,
) -> Arc<Snapshot> {
    let names: Arc<HashMap<_, _>> = Arc::new(
        observations
            .iter()
            .map(|observation| {
                (
                    observation.tuple.process,
                    format!("fixture-{}.exe", observation.tuple.process.pid).into_boxed_str(),
                )
            })
            .collect(),
    );
    let snapshot = Arc::new(Snapshot {
        counts: Counts {
            tcp: observations
                .iter()
                .filter(|observation| matches!(observation.life, Life::Tcp(_)))
                .count(),
            ended_tcp: observations
                .iter()
                .filter(|observation| matches!(observation.life, Life::EndedTcp))
                .count(),
            transferring: observations
                .iter()
                .filter(|observation| {
                    observation
                        .rates
                        .is_some_and(|rates| rates.received + rates.sent > 0.0)
                })
                .count(),
            ..Default::default()
        },
        observations: observations.into(),
        interval: Window {
            begin: Duration::from_secs(at.saturating_sub(2)),
            end: Duration::from_secs(at),
        },
        coverage: Coverage::Restricted,
        bytes: Bytes::default(),
        unattributed: Bytes::default(),
        total_bytes: Bytes::default(),
        rates: None,
        ambiguous_events: 0,
    });
    let rows = Arc::new(exploration::Rows::prepare_pending(
        &snapshot, &names, previous,
    ));
    app.accept_reply(
        work::Reply::Connections(Ok(work::ConnectionUpdate {
            publication: Publication {
                snapshot: Arc::clone(&snapshot),
                sampled: Instant::now(),
                limitation: Some(
                    "Deterministic native-control fixture; no network collection".into(),
                ),
                names,
            },
            rows,
            source: work::Source::Local(source),
        })),
        Instant::now(),
    );
    snapshot
}

pub(crate) fn scene(app: &Application, snapshot: &Arc<Snapshot>, cities: bool) -> Arc<Model> {
    let (model, reply) = prepared_scene(app, snapshot, cities);
    app.accept_reply(reply, Instant::now());
    model
}

pub(crate) fn located_scene(
    app: &Application,
    snapshot: &Arc<Snapshot>,
    cities: bool,
    locations: &HashMap<ObservationId, Location>,
) -> Arc<Model> {
    let (model, reply) = prepared_located_scene(app, snapshot, cities, locations);
    app.accept_reply(reply, Instant::now());
    model
}

pub(crate) fn prepared_scene(
    app: &Application,
    snapshot: &Arc<Snapshot>,
    cities: bool,
) -> (Arc<Model>, work::Reply) {
    let country = Location {
        code: "US".into(),
        country: "United States".into(),
        city: None,
        point: None,
    };
    let coordinate = Location {
        code: "US".into(),
        country: "United States".into(),
        city: Some("Los Angeles".into()),
        point: Some(Point {
            latitude: 34.0522,
            longitude: -118.2437,
        }),
    };
    prepared_located_scene(
        app,
        snapshot,
        cities,
        &HashMap::from([
            (ObservationId(10), country.clone()),
            (ObservationId(11), country),
            (ObservationId(12), coordinate),
        ]),
    )
}

pub(crate) fn prepared_located_scene(
    app: &Application,
    snapshot: &Arc<Snapshot>,
    cities: bool,
    locations: &HashMap<ObservationId, Location>,
) -> (Arc<Model>, work::Reply) {
    app.settings.borrow_mut().committed.detail = if cities {
        settings::Detail::Cities
    } else {
        settings::Detail::Countries
    };
    let world = app.map.world().unwrap();
    let viewport = app.map.viewport();
    let model = Arc::new(Model::arrange(
        &world,
        snapshot.observations.iter().map(|observation| Located {
            observation,
            location: locations.get(&observation.id),
        }),
        cities,
        viewport.meridian,
        viewport.scale,
    ));
    let (names, sampled) = {
        let publication = app.connections.borrow();
        let publication = publication.as_ref().unwrap();
        (Arc::clone(&publication.names), publication.sampled)
    };
    let rows = Arc::new(exploration::Rows::prepare(snapshot, Some(&model), &names));
    let id = app.scene_generation.get() + 1;
    app.scene_generation.set(id);
    let reply = work::Reply::Scene {
        id,
        model: Arc::clone(&model),
        rows,
        at: snapshot.interval.end,
        sampled,
    };
    (model, reply)
}
