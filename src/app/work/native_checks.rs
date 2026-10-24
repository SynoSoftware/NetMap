//! Bounded typed-result delivery through the ordinary registered Work owner.

use super::*;

thread_local! { static LOCAL_ORIGIN_ONLY: Cell<bool> = const { Cell::new(false) }; }

pub(crate) fn local_origin_only() -> bool {
    LOCAL_ORIGIN_ONLY.get()
}

pub(crate) struct LocalOriginOnly(bool);

pub(crate) fn restrict_origin_to_local() -> LocalOriginOnly {
    LocalOriginOnly(LOCAL_ORIGIN_ONLY.replace(true))
}

impl Drop for LocalOriginOnly {
    fn drop(&mut self) {
        LOCAL_ORIGIN_ONLY.set(self.0);
    }
}

pub(crate) struct Delivery {
    release: mpsc::SyncSender<()>,
    sent: mpsc::Receiver<bool>,
    pub(crate) endpoint: HWND,
}

impl Delivery {
    pub(crate) fn release(&self) -> io::Result<()> {
        self.release
            .try_send(())
            .map_err(|error| io::Error::other(error.to_string()))
    }

    pub(crate) fn sent(&self) -> Option<bool> {
        self.sent.try_recv().ok()
    }
}

pub(crate) fn delayed_replies(
    app: &Application,
    current: Reply,
    stale: Reply,
) -> io::Result<Delivery> {
    let mut work = app.work.borrow_mut();
    let endpoint = app.wake.get();
    let reporter = work.reporter(endpoint);
    let (release, gates) = mpsc::sync_channel(1);
    let (sent, completions) = mpsc::sync_channel(2);
    work.spawn("NetMap native reply barrier", move || {
        for reply in [current, stale] {
            // This wait belongs to the registered test worker. Failure drops the
            // UI-side sender, releasing the gate before Application::finish.
            if gates.recv_timeout(Duration::from_secs(5)).is_err() {
                let _ = sent.try_send(false);
                return;
            }
            let delivered = reporter.send(reply);
            let _ = sent.try_send(delivered);
            if !delivered {
                return;
            }
        }
    })?;
    Ok(Delivery {
        release,
        sent: completions,
        endpoint,
    })
}

pub(crate) struct LoadedLocations {
    pub(crate) previous: Report,
    pub(crate) current: Report,
    pub(crate) first: crate::geolocation::Location,
    pub(crate) second: crate::geolocation::Location,
}

pub(crate) struct LocationLoad {
    pub(crate) delivery: Delivery,
    ready: mpsc::Receiver<std::result::Result<LoadedLocations, String>>,
}

impl LocationLoad {
    pub(crate) fn ready(&self) -> Option<std::result::Result<LoadedLocations, String>> {
        self.ready.try_recv().ok()
    }
}

/// Load the real reader shared by production origin and Scene workers. Only the
/// owned availability replies wait at the gate; no UI thread locks the reader.
pub(crate) fn load_locations(app: &Application) -> io::Result<LocationLoad> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let data = super::super::data_directory()?;
    let mut work = app.work.borrow_mut();
    let endpoint = app.wake.get();
    let reporter = work.reporter(endpoint);
    let geolocation = Arc::clone(&work.geolocation);
    let (release, gates) = mpsc::sync_channel(1);
    let (sent, completions) = mpsc::sync_channel(2);
    let (ready, results) = mpsc::sync_channel(1);
    work.spawn("NetMap native local dataset", move || {
        let loaded = (|| -> std::result::Result<LoadedLocations, String> {
            let mut geo = geolocation.lock().map_err(|error| error.to_string())?;
            let previous = geo.report();
            let current = geo.load(&assets, &data);
            let lookup = |address: &str| {
                geo.find(address.parse().unwrap())
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| format!("No local location for controlled address {address}"))
            };
            Ok(LoadedLocations {
                previous,
                current,
                first: lookup("8.8.8.8")?,
                second: lookup("1.1.1.1")?,
            })
        })();
        let reports = loaded
            .as_ref()
            .ok()
            .map(|loaded| [loaded.current.clone(), loaded.previous.clone()]);
        let _ = ready.try_send(loaded);
        let Some(reports) = reports else {
            return;
        };
        for report in reports {
            if gates.recv_timeout(Duration::from_secs(5)).is_err() {
                let _ = sent.try_send(false);
                return;
            }
            let delivered = reporter.send(Reply::Locations(report));
            let _ = sent.try_send(delivered);
            if !delivered {
                return;
            }
        }
    })?;
    Ok(LocationLoad {
        delivery: Delivery {
            release,
            sent: completions,
            endpoint,
        },
        ready: results,
    })
}

pub(crate) fn wake(app: &Application) {
    app.work.borrow().reporter(app.wake.get()).wake();
}
