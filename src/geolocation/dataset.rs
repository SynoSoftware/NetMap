// Download policy ported from TinyTorrent2 Geolocation.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::app::work::http;
use flate2::read::GzDecoder;
use std::{
    fmt,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{Storage::FileSystem::*, System::SystemInformation::GetSystemTime},
    core::{HSTRING, PCWSTR},
};

// The current Lite archive is roughly 60 MiB, expanding to roughly 125 MiB.
// Keep streaming bounds well above it while rejecting runaway responses/gzip.
const ARCHIVE_LIMIT: u64 = 512 * 1024 * 1024;
const DATABASE_LIMIT: u64 = 1024 * 1024 * 1024;

#[cfg(test)]
pub(crate) mod native_checks;

#[derive(Clone, Debug)]
pub enum Phase {
    Downloading { bytes: u64, total: Option<u64> },
    Unpacking { bytes: u64 },
    Validating,
    Installing,
}

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Http(http::Error),
    File(io::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("City download cancelled"),
            Self::Http(error) => error.fmt(f),
            Self::File(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::File(error)
    }
}
impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        if matches!(error, http::Error::Cancelled) {
            Self::Cancelled
        } else {
            Self::Http(error)
        }
    }
}

#[derive(Clone, Copy)]
struct Month {
    year: u16,
    month: u16,
}
impl Month {
    fn current() -> Self {
        // SAFETY: This value-returning clock query does not retain any pointer.
        let time = unsafe { GetSystemTime() };
        Self {
            year: time.wYear,
            month: time.wMonth,
        }
    }
    fn previous(self) -> Self {
        if self.month == 1 {
            Self {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Self {
                year: self.year,
                month: self.month - 1,
            }
        }
    }
    fn path(self) -> String {
        format!(
            "/free/dbip-city-lite-{:04}-{:02}.mmdb.gz",
            self.year, self.month
        )
    }
}

pub struct Candidate {
    path: PathBuf,
}
impl Candidate {
    fn create(path: PathBuf) -> io::Result<(File, Self)> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok((file, Self { path }))
    }
}
impl Drop for Candidate {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn windows_path(path: &Path) -> HSTRING {
    HSTRING::from_wide(&path.as_os_str().encode_wide().collect::<Vec<_>>())
}

/// Prepares a validated same-directory candidate without touching the installed
/// database. Only the registered dataset worker calls this function.
pub fn build(
    directory: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(Phase),
) -> Result<Candidate, Error> {
    check_cancel(cancelled)?;
    std::fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let name = format!("city-{}-{stamp}", std::process::id());
    let (mut output, archive) = Candidate::create(directory.join(format!("{name}.gz.part")))?;
    let month = Month::current();
    progress(Phase::Downloading {
        bytes: 0,
        total: None,
    });
    let transfer = http::download(
        "download.db-ip.com",
        &month.path(),
        &mut output,
        ARCHIVE_LIMIT,
        cancelled,
        |bytes, total| progress(Phase::Downloading { bytes, total }),
    );
    match transfer {
        Err(http::Error::Status(404)) => {
            // A month's file may not yet exist during publication. Other server
            // or transport failures retain their actual error and do not retry.
            output.set_len(0)?;
            http::download(
                "download.db-ip.com",
                &month.previous().path(),
                &mut output,
                ARCHIVE_LIMIT,
                cancelled,
                |bytes, total| progress(Phase::Downloading { bytes, total }),
            )?;
        }
        other => other?,
    }
    output.sync_all()?;
    drop(output);
    check_cancel(cancelled)?;
    let mut decoder = GzDecoder::new(File::open(&archive.path)?);
    let (mut output, candidate) = Candidate::create(directory.join(format!("{name}.mmdb.tmp")))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        check_cancel(cancelled)?;
        let count = decoder.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > DATABASE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "City database exceeds the supported size",
            )
            .into());
        }
        output.write_all(&buffer[..count])?;
        progress(Phase::Unpacking { bytes: total });
    }
    output.sync_all()?;
    drop(output);
    check_cancel(cancelled)?;
    progress(Phase::Validating);
    validate(&candidate)?;
    check_cancel(cancelled)?;
    Ok(candidate)
}

fn validate(candidate: &Candidate) -> io::Result<()> {
    let database = Database::open(&candidate.path, Kind::City)?;
    database.reader.verify().map_err(invalid)
}

impl Geolocation {
    /// Serializes the short commit with lookup readers. The caller holds this
    /// owner only on a worker; cancellation cannot interrupt a commit halfway.
    pub fn install(&mut self, candidate: Candidate, directory: &Path) -> io::Result<Report> {
        let target = directory.join("city.mmdb");
        let backup = candidate.path.with_extension("backup");
        let previous_path = self.cities.database().map(|database| database.path.clone());
        if let Some(previous) = &previous_path {
            std::fs::copy(previous, &backup)?;
            OpenOptions::new().write(true).open(&backup)?.sync_all()?;
        }
        let target_exists = target.try_exists()?;
        // Release immutable mappings before native replacement, but retain the
        // independently flushed backup until reopening the new file succeeds.
        let previous = std::mem::take(&mut self.cities);
        drop(previous);
        // SAFETY: Paths are owned same-directory files. Native strings live for
        // each synchronous worker call; the map/UI holds no file/owner borrow.
        let replaced = unsafe {
            if target_exists {
                ReplaceFileW(
                    &windows_path(&target),
                    &windows_path(&candidate.path),
                    PCWSTR::null(),
                    REPLACE_FILE_FLAGS(0),
                    None,
                    None,
                )
            } else {
                MoveFileExW(
                    &windows_path(&candidate.path),
                    &windows_path(&target),
                    MOVEFILE_WRITE_THROUGH,
                )
            }
        }
        .map_err(|error| io::Error::other(error.to_string()))
        .and_then(|_| {
            #[cfg(test)]
            native_checks::after_replace(&target, &backup)?;
            Database::open(&target, Kind::City)
        });
        match replaced {
            Ok(database) => {
                self.cities = Dataset::Ready(Box::new(database));
                self.version += 1;
                let _ = std::fs::remove_file(&backup);
                // A previous failed restore may have kept its backup mapped.
                if let Some(path) = previous_path.filter(|path| path != &target) {
                    let _ = std::fs::remove_file(path);
                }
                Ok(self.report())
            }
            Err(error) => {
                if previous_path.is_some() {
                    // ReplaceFile can partially rename files on failure. Restore
                    // from the independent backup; if another process blocks it,
                    // keep serving that backup and leave it recoverable on disk.
                    // SAFETY: This restores only the flushed process-owned
                    // backup to its canonical dataset path, after mappings end.
                    let restored = unsafe {
                        MoveFileExW(
                            &windows_path(&backup),
                            &windows_path(&target),
                            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                        )
                    }
                    .is_ok();
                    self.cities = load(if restored { &target } else { &backup }, Kind::City);
                } else {
                    self.cities = Dataset::Failed(error.to_string());
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn january_falls_back_to_december_of_the_previous_year() {
        assert_eq!(
            Month {
                year: 2026,
                month: 1
            }
            .previous()
            .path(),
            "/free/dbip-city-lite-2025-12.mmdb.gz"
        );
        assert_eq!(
            Month {
                year: 2026,
                month: 10
            }
            .previous()
            .path(),
            "/free/dbip-city-lite-2026-09.mmdb.gz"
        );
    }

    #[test]
    #[ignore = "Uses NETMAP_TEST_CITY_DATABASE from a completed owned download"]
    fn blocked_replacement_preserves_version_and_backup_survives_restart() {
        let source = PathBuf::from(
            std::env::var_os("NETMAP_TEST_CITY_DATABASE").expect("Set NETMAP_TEST_CITY_DATABASE"),
        );
        let directory = std::env::temp_dir().join(format!(
            "netmap-install-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let target = directory.join("city.mmdb");
        std::fs::copy(&source, &target).unwrap();
        let mut geo = Geolocation {
            cities: load(&target, Kind::City),
            countries: Dataset::Missing,
            version: 41,
        };
        let address = "8.8.8.8".parse().unwrap();
        let before = geo.find(address).unwrap();
        assert!(before.as_ref().unwrap().point.is_some());
        let blocker = Database::open(&target, Kind::City).unwrap();
        let path = directory.join("city-1-1.mmdb.tmp");
        std::fs::copy(&source, &path).unwrap();
        assert!(geo.install(Candidate { path }, &directory).is_err());
        assert_eq!(geo.version, 41);
        assert_eq!(geo.find(address).unwrap(), before);
        let backup = geo.cities.database().unwrap().path.clone();
        assert_ne!(backup, target);
        assert!(backup.is_file());
        drop(blocker);
        drop(geo);
        std::fs::remove_file(&target).unwrap();
        let mut restarted = Geolocation::default();
        let report = restarted.load(&directory.join("absent-assets"), &directory);
        assert!(matches!(report.city, Availability::Ready { .. }));
        assert_eq!(restarted.find(address).unwrap(), before);
        let path = directory.join("city-1-2.mmdb.tmp");
        std::fs::copy(&source, &path).unwrap();
        let report = restarted.install(Candidate { path }, &directory).unwrap();
        assert_eq!(report.version, 2);
        assert_eq!(restarted.find(address).unwrap(), before);
        assert!(!backup.exists());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        drop(restarted);
        std::fs::remove_file(&target).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
}
