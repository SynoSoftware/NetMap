// Lookup policy ported from TinyTorrent2 Geolocation.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use crate::geography::Point;
use maxminddb::{Reader, geoip2};
use memmap2::Mmap;
use std::{
    fs::{File, OpenOptions},
    io,
    net::IpAddr,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows::Win32::{
    Foundation::{FILETIME, SYSTEMTIME},
    Storage::FileSystem::FILE_SHARE_READ,
    System::Time::FileTimeToSystemTime,
};

pub mod dataset;

#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub code: Box<str>,
    pub country: Box<str>,
    pub city: Option<Box<str>>,
    pub point: Option<Point>,
}

#[derive(Clone, Debug)]
pub enum Availability {
    Missing,
    Ready { built: Box<str> },
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Report {
    pub version: u64,
    pub country: Availability,
    pub city: Availability,
}

#[derive(Default)]
pub struct Geolocation {
    countries: Dataset,
    cities: Dataset,
    version: u64,
}

#[derive(Clone, Copy)]
enum Kind {
    Country,
    City,
}

#[derive(Default)]
enum Dataset {
    #[default]
    Missing,
    Ready(Box<Database>),
    Failed(String),
}

impl Dataset {
    fn database(&self) -> Option<&Database> {
        if let Self::Ready(database) = self {
            Some(database)
        } else {
            None
        }
    }

    fn availability(&self) -> Availability {
        match self {
            Self::Missing => Availability::Missing,
            Self::Ready(database) => database.availability(),
            Self::Failed(error) => Availability::Failed(error.clone()),
        }
    }
}

struct Database {
    reader: Reader<Mmap>,
    // The read-only guard denies writable/delete opens for the mapping's entire
    // life. Drop the reader before releasing this guard, including replacement.
    _file: File,
    path: PathBuf,
    built: Box<str>,
}

impl Database {
    fn open(path: &Path, kind: Kind) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(path)?;
        // SAFETY: The retained file guard excludes writers/deletion. A previously
        // opened incompatible writer also prevents this open from succeeding.
        let mapping = unsafe { Mmap::map(&file)? };
        let reader = Reader::from_source(mapping).map_err(invalid)?;
        let expected = match kind {
            Kind::Country => "DBIP-Country-Lite",
            Kind::City => "DBIP-City-Lite",
        };
        if reader.metadata().database_type != expected || reader.metadata().build_epoch == 0 {
            return Err(invalid("Unexpected DB-IP dataset type or build date"));
        }
        let ticks = reader
            .metadata()
            .build_epoch
            .checked_add(11_644_473_600)
            .and_then(|seconds| seconds.checked_mul(10_000_000))
            .ok_or_else(|| invalid("Invalid dataset build date"))?;
        let time = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut date = SYSTEMTIME::default();
        // SAFETY: Both records are initialized stack values. Native conversion
        // validates the metadata timestamp without loading a date/time framework.
        unsafe { FileTimeToSystemTime(&time, &mut date) }
            .map_err(|error| invalid(error.to_string()))?;
        let built = format!("{:04}-{:02}", date.wYear, date.wMonth).into_boxed_str();
        Ok(Self {
            reader,
            _file: file,
            path: path.into(),
            built,
        })
    }

    fn availability(&self) -> Availability {
        Availability::Ready {
            built: self.built.clone(),
        }
    }
}

fn invalid(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

impl Geolocation {
    /// All file/mapping access belongs on registered workers. UI code receives
    /// owned locations and this availability report, never a mapped reader.
    pub fn load(&mut self, assets: &Path, data: &Path) -> Report {
        self.countries = load(&assets.join("country.mmdb"), Kind::Country);
        self.cities = load_city(data);
        self.version += 1;
        self.report()
    }

    pub fn report(&self) -> Report {
        Report {
            version: self.version,
            country: self.countries.availability(),
            city: self.cities.availability(),
        }
    }

    pub fn find(&self, address: IpAddr) -> io::Result<Option<Location>> {
        let country = self
            .countries
            .database()
            .map(|db| {
                db.reader
                    .lookup(address)
                    .and_then(|record| record.decode::<geoip2::Country>())
            })
            .transpose()
            .map_err(invalid)?
            .flatten();
        let city = self
            .cities
            .database()
            .map(|db| {
                db.reader
                    .lookup(address)
                    .and_then(|record| record.decode::<geoip2::City>())
            })
            .transpose()
            .map_err(invalid)?
            .flatten();
        let country = country
            .as_ref()
            .map(|record| &record.country)
            .filter(|country| country.iso_code.is_some())
            .or_else(|| city.as_ref().map(|record| &record.country));
        let Some(country) = country else {
            return Ok(None);
        };
        let Some(code) = country.iso_code else {
            return Ok(None);
        };
        if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(invalid("Invalid country code"));
        }
        let point = city.as_ref().and_then(|city| {
            let latitude = city.location.latitude?;
            let longitude = city.location.longitude?;
            (latitude.is_finite()
                && longitude.is_finite()
                && (-90.0..=90.0).contains(&latitude)
                && (-180.0..=180.0).contains(&longitude))
            .then_some(Point {
                latitude,
                longitude,
            })
        });
        Ok(Some(Location {
            code: code.into(),
            country: country.names.english.unwrap_or(code).into(),
            city: city
                .as_ref()
                .and_then(|city| city.city.names.english)
                .map(Into::into),
            point,
        }))
    }
}

fn load(path: &Path, kind: Kind) -> Dataset {
    match Database::open(path, kind) {
        Ok(database) => Dataset::Ready(Box::new(database)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Dataset::Missing,
        Err(error) => Dataset::Failed(error.to_string()),
    }
}

fn load_city(directory: &Path) -> Dataset {
    let current = load(&directory.join("city.mmdb"), Kind::City);
    if matches!(current, Dataset::Ready(_)) {
        return current;
    }
    // A prior process can end between replacement and reader reopening. An
    // independently flushed backup remains usable even when its restore was
    // blocked; validate it through the same reader and retain its actual path.
    let Ok(entries) = std::fs::read_dir(directory) else {
        return current;
    };
    let mut backups = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let stamp = name
                .strip_prefix("city-")?
                .strip_suffix(".mmdb.backup")?
                .rsplit_once('-')?
                .1
                .parse::<u128>()
                .ok()?;
            Some((stamp, entry.path()))
        })
        .collect::<Vec<_>>();
    backups.sort_unstable_by_key(|(stamp, _)| std::cmp::Reverse(*stamp));
    for (_, path) in backups {
        if let ready @ Dataset::Ready(_) = load(&path, Kind::City) {
            return ready;
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_country_data_is_mapped_and_does_not_invent_city_coordinates() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/country.mmdb");
        let countries = Database::open(&path, Kind::Country).unwrap();
        assert_eq!(
            countries.reader.metadata().database_type,
            "DBIP-Country-Lite"
        );
        countries.reader.verify().unwrap();
        let geo = Geolocation {
            countries: Dataset::Ready(Box::new(countries)),
            cities: Dataset::Missing,
            version: 1,
        };
        let location = geo.find("8.8.8.8".parse().unwrap()).unwrap().unwrap();
        assert_eq!(&*location.code, "US");
        assert!(location.point.is_none() && location.city.is_none());
        // A live mapping must reject writable opens rather than merely trusting
        // every future caller to keep the backing bytes immutable.
        drop(geo);
        let temporary = std::env::temp_dir().join(format!(
            "netmap-country-{}-{}.mmdb",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::copy(&path, &temporary).unwrap();
        let mapped = Database::open(&temporary, Kind::Country).unwrap();
        assert!(OpenOptions::new().write(true).open(&temporary).is_err());
        drop(mapped);
        assert!(OpenOptions::new().write(true).open(&temporary).is_ok());
        std::fs::remove_file(temporary).unwrap();
    }
}
