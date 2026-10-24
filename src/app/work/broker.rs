use crate::{
    app::settings::storage,
    connections::{Collector, Highlight, Publication, pipe},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub(super) type Latest = Arc<Mutex<Option<Publication>>>;

pub(super) fn collect(
    data: PathBuf,
    latest: Latest,
    retired: Arc<AtomicBool>,
    consumers: std::sync::mpsc::SyncSender<crate::connections::etw::Consumer>,
) -> Result<(), String> {
    let (mut collector, consumer) = Collector::new().map_err(|error| error.to_string())?;
    let consumer = consumer.ok_or("Whole-PC collection requires tracing rights")?;
    consumers
        .send(consumer)
        .map_err(|_| "Trace worker unavailable")?;
    let mut next = Instant::now();
    let defaults = crate::app::settings::Preferences::default();
    let mut highlight = Highlight {
        bright: Duration::from_secs(defaults.bright_seconds),
        fade: Duration::from_secs(defaults.fade_seconds),
    };
    while !retired.load(Ordering::Acquire) {
        collector.poll();
        if Instant::now() >= next {
            // All clients use this user's canonical committed preferences. The
            // pipe stays read-only; no viewer can mutate collection policy.
            highlight = refresh_highlight(&data, highlight)?;
            let publication = collector
                .sample(highlight)
                .map_err(|error| error.to_string())?;
            let previous = latest
                .lock()
                .map_err(|_| "Collector publication unavailable")?
                .replace(publication);
            drop(previous);
            next = Instant::now() + Duration::from_secs(2);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn refresh_highlight(data: &std::path::Path, previous: Highlight) -> Result<Highlight, String> {
    match storage::load(data) {
        Ok(preferences) => Ok(Highlight {
            bright: Duration::from_secs(preferences.bright_seconds),
            fade: Duration::from_secs(preferences.fade_seconds),
        }),
        // Settings holds the same transaction lock while flushing/replacing its
        // accepted save. Keep the last accepted policy for this interval; the
        // next sample reads the completed commit instead of stopping capture.
        Err(error)
            if error.raw_os_error()
                == Some(windows::Win32::Foundation::ERROR_SHARING_VIOLATION.0 as i32) =>
        {
            Ok(previous)
        }
        Err(error) => Err(format!("Collector preferences unavailable: {error}")),
    }
}

pub(super) fn serve(latest: Latest, retired: Arc<AtomicBool>) -> Result<(), String> {
    let mut server = pipe::Server::new().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut last_client = started;
    let mut had_client = false;
    let mut last_sample = None;
    while !retired.load(Ordering::Acquire) {
        let publication = latest
            .lock()
            .map_err(|_| "Collector publication unavailable")?
            .clone();
        if let Some(publication) = publication
            && last_sample != Some(publication.sampled)
        {
            server
                .publish(&publication)
                .map_err(|error| error.to_string())?;
            last_sample = Some(publication.sampled);
        }
        server.poll(&retired).map_err(|error| error.to_string())?;
        if server.has_clients() {
            last_client = Instant::now();
            had_client = true;
        }
        // Five cadences for initial subscription, three for reconnect. The
        // helper never becomes an unattended logon-wide service after viewers exit.
        if (!had_client && started.elapsed() >= Duration::from_secs(10))
            || (had_client && last_client.elapsed() >= Duration::from_secs(6))
        {
            retired.store(true, Ordering::Release);
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saving_preferences_does_not_stop_collection_or_change_retention_early() {
        let directory =
            std::env::temp_dir().join(format!("netmap-broker-settings-{}", std::process::id()));
        let previous = Highlight {
            bright: Duration::from_secs(7),
            fade: Duration::from_secs(9),
        };
        {
            let _transaction = crate::app::storage::File::open(&directory, "preferences").unwrap();
            let retained = refresh_highlight(&directory, previous).unwrap();
            assert_eq!(retained.bright, previous.bright);
            assert_eq!(retained.fade, previous.fade);
        }
        let desired = crate::app::settings::Preferences {
            bright_seconds: 20,
            ..Default::default()
        };
        storage::save(&directory, &Default::default(), &desired).unwrap();
        assert_eq!(
            refresh_highlight(&directory, previous).unwrap().bright,
            Duration::from_secs(20)
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
