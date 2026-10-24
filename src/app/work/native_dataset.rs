//! Private local input and registered reader work for dataset lifetime checks.

use super::*;
use crate::geolocation::{
    Location,
    dataset::native_checks::{self, Action, Control, Gate},
};
use std::cell::RefCell;

thread_local! { static NEXT: RefCell<Option<Input>> = const { RefCell::new(None) }; }

pub(crate) struct Input {
    source: PathBuf,
    gate: Option<Gate>,
}

pub(crate) fn arm(action: Option<Action>) -> io::Result<Option<Control>> {
    let source = std::env::var_os("NETMAP_TEST_CITY_DATABASE")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            io::Error::other("NETMAP_TEST_CITY_DATABASE must name the retained owned database")
        })?;
    NEXT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(io::Error::other("A native city candidate is already armed"));
        }
        let (gate, control) = match action {
            Some(action) => {
                let (gate, control) = native_checks::gate(action);
                (Some(gate), Some(control))
            }
            None => (None, None),
        };
        *slot = Some(Input { source, gate });
        Ok(control)
    })
}

pub(crate) fn take_candidate() -> Option<Input> {
    NEXT.with(|slot| slot.borrow_mut().take())
}
pub(crate) fn clear() {
    NEXT.with(|slot| slot.borrow_mut().take());
}

pub(crate) fn build(
    input: Option<Input>,
    directory: &std::path::Path,
    cancelled: &AtomicBool,
    progress: impl FnMut(Phase),
) -> std::result::Result<dataset::Candidate, dataset::Error> {
    match input {
        Some(input) => {
            native_checks::candidate(&input.source, directory, cancelled, progress, input.gate)
        }
        None => dataset::build(directory, cancelled, progress),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Read {
    Load,
    Current,
    Withdraw,
}
pub(crate) struct Readback {
    pub(crate) report: Report,
    pub(crate) location: Option<Location>,
    pub(crate) city: Option<PathBuf>,
}
pub(crate) struct Reading(mpsc::Receiver<std::result::Result<Readback, String>>);
impl Reading {
    pub(crate) fn ready(&self) -> io::Result<Option<Readback>> {
        match self.0.try_recv() {
            Ok(result) => result.map(Some).map_err(io::Error::other),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::other(
                "Registered dataset read retired without a result",
            )),
        }
    }
}

pub(crate) fn read(app: &Application, action: Read) -> io::Result<Reading> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let data = super::super::data_directory()?;
    let mut work = app.work.borrow_mut();
    let geo = Arc::clone(&work.geolocation);
    let reporter = work.reporter(app.wake.get());
    let (sender, receiver) = mpsc::sync_channel(1);
    work.spawn("NetMap native dataset reader", move || {
        let result = (|| -> io::Result<Readback> {
            let mut geo = geo
                .lock()
                .map_err(|_| io::Error::other("Geolocation reader failed"))?;
            if matches!(action, Read::Withdraw) {
                native_checks::withdraw(&mut geo, &data)?;
            }
            let report = if matches!(action, Read::Current) {
                geo.report()
            } else {
                geo.load(&assets, &data)
            };
            let location = geo.find("8.8.8.8".parse().unwrap())?;
            let city = native_checks::city_path(&geo);
            Ok(Readback {
                report,
                location,
                city,
            })
        })();
        if let Ok(readback) = &result {
            reporter.send(Reply::Locations(readback.report.clone()));
        }
        let _ = sender.try_send(result.map_err(|error| error.to_string()));
    })?;
    Ok(Reading(receiver))
}
