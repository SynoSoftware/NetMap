//! Owned local candidates and a scoped gate at the real installer boundary.

use super::*;
use std::{
    cell::RefCell,
    sync::mpsc,
    time::{Duration, Instant},
};

thread_local! { static BOUNDARY: RefCell<Option<Gate>> = const { RefCell::new(None) }; }

#[derive(Clone, Copy)]
pub(crate) enum Action {
    FailReopen,
    Hold,
}

#[derive(Clone)]
pub(crate) struct Checkpoint {
    pub(crate) target: PathBuf,
    pub(crate) backup: PathBuf,
    pub(crate) displaced: Option<PathBuf>,
    pub(crate) at: u64,
}

pub(crate) struct Gate {
    action: Action,
    reached: mpsc::SyncSender<io::Result<Checkpoint>>,
    release: mpsc::Receiver<()>,
}

pub(crate) struct Control {
    reached: mpsc::Receiver<io::Result<Checkpoint>>,
    release: Option<mpsc::SyncSender<()>>,
}

pub(crate) fn gate(action: Action) -> (Gate, Control) {
    let (reached, checkpoints) = mpsc::sync_channel(1);
    let (release, releases) = mpsc::sync_channel(1);
    (
        Gate {
            action,
            reached,
            release: releases,
        },
        Control {
            reached: checkpoints,
            release: Some(release),
        },
    )
}

impl Control {
    pub(crate) fn checkpoint(&self) -> io::Result<Option<Checkpoint>> {
        match self.reached.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::other(
                "Installer retired before its replacement boundary",
            )),
        }
    }
    pub(crate) fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.try_send(());
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) fn candidate(
    source: &Path,
    directory: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(Phase),
    gate: Option<Gate>,
) -> Result<Candidate, Error> {
    check_cancel(cancelled)?;
    std::fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let path = directory.join(format!("city-{}-{stamp}.mmdb.tmp", std::process::id()));
    let (mut output, candidate) = Candidate::create(path)?;
    let mut input = File::open(source)?;
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        check_cancel(cancelled)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > DATABASE_LIMIT {
            return Err(
                io::Error::other("Owned candidate exceeds production database limit").into(),
            );
        }
        output.write_all(&buffer[..count])?;
    }
    output.sync_all()?;
    drop(output);
    progress(Phase::Validating);
    validate(&candidate)?;
    check_cancel(cancelled)?;
    BOUNDARY.with(|slot| *slot.borrow_mut() = gate);
    println!("NETMAP_DATASET {{\"event\":\"candidate_validated\",\"bytes\":{total}}}");
    let _ = io::stdout().flush();
    Ok(candidate)
}

pub(crate) fn after_replace(target: &Path, backup: &Path) -> io::Result<()> {
    let Some(gate) = BOUNDARY.with(|slot| slot.borrow_mut().take()) else {
        return Ok(());
    };
    let checkpoint = (|| -> io::Result<Checkpoint> {
        if !target.is_file() || !backup.is_file() {
            return Err(io::Error::other(
                "Real replacement or flushed old backup is missing",
            ));
        }
        let displaced = if matches!(gate.action, Action::FailReopen) {
            let path = target.with_file_name("city-displaced-new.evidence");
            std::fs::rename(target, &path)?;
            Some(path)
        } else {
            None
        };
        Ok(Checkpoint {
            target: target.into(),
            backup: backup.into(),
            displaced,
            at: crate::app::work::native_shutdown::tick(),
        })
    })();
    let passed = checkpoint.is_ok();
    let _ = gate.reached.try_send(checkpoint);
    if !passed {
        return Err(io::Error::other("Owned boundary filesystem action failed"));
    }
    println!(
        "NETMAP_DATASET {{\"event\":\"replacement_before_reopen\",\"backup_flushed\":true,\"target_displaced\":{}}}",
        matches!(gate.action, Action::FailReopen)
    );
    let _ = io::stdout().flush();
    let since = Instant::now();
    let released = gate.release.recv_timeout(Duration::from_secs(15));
    println!(
        "NETMAP_DATASET {{\"event\":\"boundary_returned\",\"released\":{},\"elapsed_ms\":{}}}",
        released.is_ok(),
        since.elapsed().as_millis()
    );
    let _ = io::stdout().flush();
    released.map_err(|error| io::Error::other(format!("Dataset boundary gate: {error}")))?;
    Ok(())
}

pub(crate) fn withdraw(geo: &mut Geolocation, directory: &Path) -> io::Result<()> {
    // Only this registered fixture worker holds the real reader owner. End the
    // mapping before renaming the owned file, then real load reports Missing.
    drop(std::mem::take(&mut geo.cities));
    std::fs::rename(
        directory.join("city.mmdb"),
        directory.join("city-withdrawn.evidence"),
    )
}

pub(crate) fn city_path(geo: &Geolocation) -> Option<PathBuf> {
    geo.cities.database().map(|database| database.path.clone())
}
