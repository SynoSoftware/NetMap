use super::settings::Origin as Choice;
use crate::{
    geography::{Point, World},
    geolocation::Location,
};
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

pub const PROVIDER: &str = "api64.ipify.org";
const NETWORK_QUIET: Duration = Duration::from_secs(2);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Discover {
        id: u64,
    },
    Locate {
        id: u64,
        address: IpAddr,
        version: u64,
    },
}

impl Request {
    pub fn id(self) -> u64 {
        match self {
            Self::Discover { id } | Self::Locate { id, .. } => id,
        }
    }
}

#[derive(Clone, Copy)]
enum Retry {
    Discover,
    Locate(IpAddr),
}

#[derive(Default)]
enum State {
    #[default]
    Waiting,
    Blocked(String),
    Manual(Point),
    Debouncing(Instant),
    Discovering {
        id: u64,
        until: Instant,
    },
    WaitingForData(IpAddr),
    Locating {
        id: u64,
        address: IpAddr,
        version: u64,
    },
    Ready {
        address: IpAddr,
        location: Option<Location>,
    },
    Failed {
        reason: String,
        retry: Retry,
        at: Instant,
    },
}

#[derive(Default)]
pub struct Origin {
    state: State,
    generation: u64,
    dataset: Option<u64>,
    failures: u32,
    watch_error: Option<String>,
}

impl Origin {
    pub fn needs_preferences(&self) -> bool {
        matches!(self.state, State::Waiting | State::Blocked(_))
    }

    pub fn configure(&mut self, choice: Choice, now: Instant) {
        self.failures = 0;
        self.generation += 1;
        self.state = match choice {
            Choice::Manual(point) => State::Manual(point),
            Choice::Automatic => State::Discovering {
                id: self.generation,
                until: now + DISCOVERY_TIMEOUT,
            },
        };
    }

    pub fn preferences_failed(&mut self) {
        self.state =
            State::Blocked("Origin unavailable: saved preferences could not be read.".into());
    }

    fn automatic(&self) -> bool {
        !matches!(
            self.state,
            State::Waiting | State::Blocked(_) | State::Manual(_)
        )
    }

    pub fn network_changed(&mut self, now: Instant) {
        if self.automatic() {
            // Stale egress facts stop supplying a point immediately, before
            // debounce. No late request may restore the old overview origin.
            self.generation += 1;
            self.failures = 0;
            self.state = State::Debouncing(now + NETWORK_QUIET);
        }
    }

    pub fn dataset_changed(&mut self, version: u64) {
        if self.dataset == Some(version) {
            return;
        }
        self.dataset = Some(version);
        let address = match &self.state {
            State::WaitingForData(address)
            | State::Locating { address, .. }
            | State::Ready { address, .. } => Some(*address),
            State::Failed {
                retry: Retry::Locate(address),
                ..
            } => Some(*address),
            _ => None,
        };
        if let Some(address) = address {
            self.locate(address);
        }
    }

    fn locate(&mut self, address: IpAddr) {
        self.generation += 1;
        self.state = match self.dataset {
            Some(version) => State::Locating {
                id: self.generation,
                address,
                version,
            },
            None => State::WaitingForData(address),
        };
    }

    pub fn request(&self) -> Option<Request> {
        match self.state {
            State::Discovering { id, .. } => Some(Request::Discover { id }),
            State::Locating {
                id,
                address,
                version,
            } => Some(Request::Locate {
                id,
                address,
                version,
            }),
            _ => None,
        }
    }

    pub fn discovered(&mut self, id: u64, result: Result<IpAddr, String>, now: Instant) {
        if !matches!(self.state, State::Discovering { id: current, .. } if current == id) {
            return;
        }
        if self.deadline().is_some_and(|until| now >= until) {
            self.tick(now);
            return;
        }
        match result {
            Ok(address) => {
                self.failures = 0;
                self.locate(address);
            }
            Err(reason) => self.fail(reason, Retry::Discover, now),
        }
    }

    pub fn located(
        &mut self,
        id: u64,
        version: u64,
        result: Result<Option<Location>, String>,
        now: Instant,
    ) {
        let State::Locating {
            id: current,
            address,
            version: expected,
        } = self.state
        else {
            return;
        };
        if current != id {
            return;
        }
        if version != expected || self.dataset != Some(version) {
            // Installation won the worker lock. Wait for its versioned report
            // instead of spinning requests against a dataset already replaced.
            self.state = State::WaitingForData(address);
            return;
        }
        match result {
            Ok(location) => {
                self.failures = 0;
                self.state = State::Ready { address, location };
            }
            Err(reason) => self.fail(reason, Retry::Locate(address), now),
        }
    }

    pub fn submission_failed(&mut self, request: Request, reason: String, now: Instant) {
        if self.request() != Some(request) {
            return;
        }
        let retry = match request {
            Request::Discover { .. } => Retry::Discover,
            Request::Locate { address, .. } => Retry::Locate(address),
        };
        self.fail(reason, retry, now);
    }

    fn fail(&mut self, reason: String, retry: Retry, now: Instant) {
        let seconds = 5u64.saturating_mul(1u64 << self.failures.min(6)).min(300);
        self.failures = self.failures.saturating_add(1);
        self.state = State::Failed {
            reason,
            retry,
            at: now + Duration::from_secs(seconds),
        };
    }

    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::Debouncing(at)
            | State::Failed { at, .. }
            | State::Discovering { until: at, .. } => Some(at),
            _ => None,
        }
    }

    pub fn tick(&mut self, now: Instant) {
        if self.deadline().is_none_or(|at| now < at) {
            return;
        }
        match self.state {
            State::Debouncing(_)
            | State::Failed {
                retry: Retry::Discover,
                ..
            } => {
                self.generation += 1;
                self.state = State::Discovering {
                    id: self.generation,
                    until: now + DISCOVERY_TIMEOUT,
                };
            }
            State::Failed {
                retry: Retry::Locate(address),
                ..
            } => self.locate(address),
            State::Discovering { .. } => self.fail(
                "Public-address discovery timed out".into(),
                Retry::Discover,
                now,
            ),
            _ => {}
        }
    }

    pub fn watch_result(&mut self, result: Result<(), String>) {
        self.watch_error = result.err();
    }

    pub fn timer_failed(&mut self) {
        self.state = State::Blocked("Automatic origin refresh could not be scheduled.".into());
    }

    pub fn point(&self, world: Option<&World>) -> Option<Point> {
        match &self.state {
            State::Manual(point) => Some(*point),
            State::Ready {
                location: Some(location),
                ..
            } => location
                .point
                .or_else(|| world?.country(&location.code).map(|country| country.anchor)),
            _ => None,
        }
    }

    pub fn status(&self) -> String {
        let status = match &self.state {
            State::Waiting => "Waiting for saved origin preferences.".into(),
            State::Blocked(reason) => reason.clone(),
            State::Manual(point) => {
                format!("Manual origin: {}, {}.", point.latitude, point.longitude)
            }
            State::Debouncing(_) => {
                "Origin unknown: network changed; waiting for it to settle.".into()
            }
            State::Discovering { .. } => "Finding public egress address…".into(),
            State::WaitingForData(_) => "Origin unknown: waiting for local location data.".into(),
            State::Locating { .. } => "Locating the public egress address locally…".into(),
            State::Ready { location: None, .. } => {
                "Automatic origin is unknown in the installed location data.".into()
            }
            State::Ready {
                location: Some(location),
                ..
            } => match &location.city {
                Some(city) if location.point.is_some() => format!(
                    "Approximate automatic origin: {city}, {}.",
                    location.country
                ),
                _ => format!("Automatic origin: {} (country estimate).", location.country),
            },
            State::Failed { reason, .. } => {
                format!("Origin unknown: {reason}. Retrying with backoff.")
            }
        };
        if self.automatic()
            && let Some(error) = &self.watch_error
        {
            format!("{status} Network-change detection unavailable: {error}")
        } else {
            status
        }
    }
}

pub fn parse_address(bytes: &[u8]) -> Result<IpAddr, String> {
    let address: IpAddr = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .ok_or_else(|| "The discovery provider did not return an IP address".to_owned())?;
    let address = match address {
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(address)),
        _ => address,
    };
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return Err("The discovery provider returned an unusable egress address".into());
    }
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_choice_supersedes_discovery_and_ignores_network_events() {
        let now = Instant::now();
        let mut origin = Origin::default();
        assert!(origin.request().is_none());
        origin.configure(Choice::Automatic, now);
        let id = origin.request().unwrap().id();
        let point = Point {
            latitude: 4.0,
            longitude: -55.0,
        };
        origin.configure(Choice::Manual(point), now);
        origin.discovered(id, Ok("8.8.8.8".parse().unwrap()), now);
        origin.network_changed(now);
        origin.dataset_changed(2);
        assert_eq!(origin.point(None), Some(point));
        assert!(origin.request().is_none());
        assert!(origin.deadline().is_none());
    }

    #[test]
    fn route_bursts_invalidate_old_results_and_debounce_one_new_request() {
        let now = Instant::now();
        let mut origin = Origin::default();
        origin.configure(Choice::Automatic, now);
        let old = origin.request().unwrap().id();
        origin.network_changed(now);
        origin.network_changed(now + Duration::from_secs(1));
        origin.discovered(old, Ok("8.8.8.8".parse().unwrap()), now);
        origin.tick(now + Duration::from_secs(2));
        assert!(origin.request().is_none());
        origin.tick(now + Duration::from_secs(3));
        assert!(matches!(origin.request(), Some(Request::Discover { id }) if id != old));
    }

    #[test]
    fn dataset_replacement_reuses_address_without_a_second_public_request() {
        let now = Instant::now();
        let mut origin = Origin::default();
        origin.configure(Choice::Automatic, now);
        let id = origin.request().unwrap().id();
        let address = "8.8.8.8".parse().unwrap();
        origin.discovered(id, Ok(address), now);
        assert!(origin.request().is_none());
        origin.dataset_changed(1);
        let Request::Locate { id, .. } = origin.request().unwrap() else {
            panic!()
        };
        origin.dataset_changed(2);
        let current = origin.request();
        origin.located(id, 1, Ok(None), now);
        assert_eq!(origin.request(), current);
        assert!(
            matches!(current, Some(Request::Locate { address: found, version: 2, .. }) if found == address)
        );
    }

    #[test]
    fn failed_discovery_backs_off_and_invalid_public_addresses_are_rejected() {
        let now = Instant::now();
        let mut origin = Origin::default();
        origin.configure(Choice::Automatic, now);
        let id = origin.request().unwrap().id();
        origin.discovered(id, Err("offline".into()), now);
        assert_eq!(origin.deadline(), Some(now + Duration::from_secs(5)));
        origin.tick(now + Duration::from_secs(5));
        let id = origin.request().unwrap().id();
        origin.discovered(id, Err("offline".into()), now + Duration::from_secs(5));
        assert_eq!(origin.deadline(), Some(now + Duration::from_secs(15)));
        assert!(parse_address(b"127.0.0.1").is_err());
        assert!(parse_address(b"<html>failure</html>").is_err());
        assert_eq!(
            parse_address(b" 2001:4860:4860::8888\n").unwrap(),
            "2001:4860:4860::8888".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn discovery_deadline_rejects_late_completion_and_retries_with_a_new_identity() {
        let now = Instant::now();
        let mut origin = Origin::default();
        origin.configure(Choice::Automatic, now);
        let old = origin.request().unwrap().id();
        let deadline = now + DISCOVERY_TIMEOUT;
        assert_eq!(origin.deadline(), Some(deadline));
        origin.discovered(old, Ok("8.8.8.8".parse().unwrap()), deadline);
        assert!(origin.request().is_none());
        assert_eq!(origin.point(None), None);
        assert_eq!(origin.deadline(), Some(deadline + Duration::from_secs(5)));
        origin.tick(deadline + Duration::from_secs(5));
        let current = origin.request();
        assert!(matches!(current, Some(Request::Discover { id }) if id != old));
        origin.discovered(old, Ok("8.8.8.8".parse().unwrap()), deadline);
        assert_eq!(origin.request(), current);
        origin.tick(deadline + Duration::from_secs(35));
        assert!(origin.request().is_none());
        assert_eq!(origin.deadline(), Some(deadline + Duration::from_secs(45)));
    }

    #[test]
    fn local_country_anchor_is_estimated_but_an_unknown_origin_never_becomes_zero() {
        let point = Point {
            latitude: 5.0,
            longitude: -55.0,
        };
        let world = World {
            sections: Box::new([]),
            countries: vec![crate::geography::Country {
                code: "SR".into(),
                anchor: point,
                rings: Box::new([]),
                areas: Box::new([]),
            }]
            .into_boxed_slice(),
        };
        let mut origin = Origin::default();
        assert_eq!(origin.point(Some(&world)), None);
        origin.state = State::Ready {
            address: "8.8.8.8".parse().unwrap(),
            location: Some(Location {
                code: "SR".into(),
                country: "Suriname".into(),
                city: None,
                point: None,
            }),
        };
        assert_eq!(origin.point(Some(&world)), Some(point));
        origin.network_changed(Instant::now());
        assert_eq!(origin.point(Some(&world)), None);
        assert!(
            super::super::settings::Page::Location
                .caption("disclosure")
                .contains(PROVIDER)
        );
    }
}
