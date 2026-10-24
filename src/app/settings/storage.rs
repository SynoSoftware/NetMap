use super::{Detail, Draft, Preferences, SpeedCorner};
use std::{io, path::Path};

fn encode(value: &Preferences) -> String {
    let draft = Draft::from(value);
    format!(
        "NetMap preferences 1\ndetail={}\nspeeds={}\nbright={}\nfade={}\norigin={}\nlatitude={}\nlongitude={}\ndesktop={}\nmonitor={}\n",
        match value.detail {
            Detail::Countries => "countries",
            Detail::Cities => "cities",
        },
        match value.speeds {
            SpeedCorner::TopLeft => "top-left",
            SpeedCorner::BottomLeft => "bottom-left",
            SpeedCorner::BottomRight => "bottom-right",
        },
        draft.bright,
        draft.fade,
        if draft.manual { "manual" } else { "automatic" },
        draft.latitude,
        draft.longitude,
        draft.desktop,
        draft.monitor,
    )
}

fn decode(text: &str) -> io::Result<Preferences> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid NetMap preferences file",
        )
    };
    let mut lines = text.lines();
    if lines.next() != Some("NetMap preferences 1") {
        return Err(invalid());
    }
    let mut fields = std::collections::HashMap::new();
    for line in lines {
        let (key, value) = line.split_once('=').ok_or_else(invalid)?;
        if fields.insert(key, value).is_some() {
            return Err(invalid());
        }
    }
    let field = |key| fields.get(key).copied().ok_or_else(invalid);
    let detail = match field("detail")? {
        "countries" => Detail::Countries,
        "cities" => Detail::Cities,
        _ => return Err(invalid()),
    };
    let draft = Draft {
        detail,
        speeds: match field("speeds")? {
            "top-left" => SpeedCorner::TopLeft,
            "bottom-left" => SpeedCorner::BottomLeft,
            "bottom-right" => SpeedCorner::BottomRight,
            _ => return Err(invalid()),
        },
        bright: field("bright")?.into(),
        fade: field("fade")?.into(),
        manual: match field("origin")? {
            "manual" => true,
            "automatic" => false,
            _ => return Err(invalid()),
        },
        latitude: field("latitude")?.into(),
        longitude: field("longitude")?.into(),
        desktop: match fields.get("desktop").copied() {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(invalid()),
        },
        monitor: fields.get("monitor").copied().unwrap_or_default().into(),
    };
    draft
        .validate(true, &Preferences::default())
        .map_err(|_| invalid())
}

// Only registered workers call these I/O entry points. A rollback file is
// authoritative until its removal commits a replacement, including after a
// crash or a partially failed native rename. Never infer success from the new
// canonical file alone while that recovery record still exists.
pub fn load(directory: &Path) -> io::Result<Preferences> {
    read_current(&crate::app::storage::File::open(directory, "preferences")?)
}

fn read_current(file: &crate::app::storage::File<'_>) -> io::Result<Preferences> {
    match file.read() {
        Ok(text) => decode(&text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(error),
    }
}

pub fn save(directory: &Path, previous: &Preferences, desired: &Preferences) -> io::Result<()> {
    // Another NetMap process must not interleave this small transaction. An
    // exclusive non-waiting open gives it a retryable error. The empty file
    // persists; the native handle owns the lock and cannot leave a stale lock.
    let file = crate::app::storage::File::open(directory, "preferences")?;
    let previous = match read_current(&file) {
        Ok(previous) => previous,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => previous.clone(),
        Err(error) => return Err(error),
    };
    file.commit(&encode(&previous), &encode(desired))
}

#[cfg(test)]
mod tests {
    use super::*;
    const FILE_NAME: &str = "preferences.txt";
    const ROLLBACK: &str = "preferences.rollback";
    const LOCK: &str = "preferences.lock";
    use crate::app::settings::Origin;
    use crate::geography::Point;
    use std::{
        fs::{self, OpenOptions},
        os::windows::fs::OpenOptionsExt,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn saved_choices_round_trip_and_malformed_values_are_rejected() {
        let value = Preferences {
            origin: Origin::Manual(Point {
                latitude: -5.5,
                longitude: 123.25,
            }),
            detail: Detail::Cities,
            desktop: true,
            monitor: r"\\.\DISPLAY4".into(),
            ..Preferences::default()
        };
        assert_eq!(decode(&encode(&value)).unwrap(), value);
        assert!(decode(&encode(&value).replace("bright=10", "bright=0")).is_err());
        assert!(decode(&encode(&value).replace("123.25", "NaN")).is_err());
        assert!(decode(&(encode(&value) + "bright=20\n")).is_err());
        let legacy = encode(&Preferences::default())
            .lines()
            .filter(|line| !line.starts_with("desktop=") && !line.starts_with("monitor="))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(decode(&legacy).unwrap(), Preferences::default());
        assert!(decode(&encode(&value).replace("desktop=true", "desktop=1")).is_err());
        assert!(decode(&encode(&value).replace(r"\\.\DISPLAY4", "bad\0monitor")).is_err());
    }

    #[test]
    fn blocked_commit_keeps_previous_choices_recoverable_after_restart() {
        let directory = std::env::temp_dir().join(format!(
            "netmap-preferences-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let previous = Preferences::default();
        let desired = Preferences {
            bright_seconds: 15,
            ..previous.clone()
        };
        save(&directory, &previous, &previous).unwrap();
        let blocker = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(directory.join(FILE_NAME))
            .unwrap();
        assert!(save(&directory, &previous, &desired).is_err());
        assert_eq!(load(&directory).unwrap(), previous);
        drop(blocker);
        // Even a new canonical file cannot win before rollback removal.
        fs::write(directory.join(FILE_NAME), encode(&desired)).unwrap();
        assert_eq!(load(&directory).unwrap(), previous);
        save(&directory, &previous, &desired).unwrap();
        assert_eq!(load(&directory).unwrap(), desired);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(directory.join(LOCK))
            .unwrap();
        assert!(save(&directory, &desired, &previous).is_err());
        assert!(load(&directory).is_err());
        assert!(!directory.join(ROLLBACK).exists());
        drop(lock);
        assert_eq!(load(&directory).unwrap(), desired);
        fs::remove_file(directory.join(FILE_NAME)).unwrap();
        fs::remove_file(directory.join(LOCK)).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
