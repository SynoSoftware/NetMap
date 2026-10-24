use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
};

use super::{Reply, Reporter};
use crate::{
    app::exploration::Rows,
    connections::{ProcessId, Snapshot},
    geography::World,
    geolocation::{Geolocation, Location},
    map::model::{Located, Model},
};

pub(crate) struct Request {
    pub id: u64,
    pub world: Arc<World>,
    pub snapshot: Arc<Snapshot>,
    pub names: Arc<HashMap<ProcessId, Box<str>>>,
    pub sampled: std::time::Instant,
    pub cities: bool,
    pub meridian: f32,
    pub scale: f32,
    pub version: u64,
}

// Location/cache/model work stays on this registered worker. The cache is
// bounded by addresses in the latest facts and is invalidated as one dataset
// generation; neither paint nor pan reads MMDB or scans observations.
pub(super) fn run(
    requests: Receiver<Request>,
    reporter: Reporter,
    geolocation: Arc<Mutex<Geolocation>>,
    cancelled: Arc<AtomicBool>,
) {
    let mut cache: HashMap<IpAddr, Option<Location>> = HashMap::new();
    let mut version = None;
    let mut previous: Option<Request> = None;
    let mut prepared = Arc::new(Model::default());
    while !cancelled.load(Ordering::Acquire) {
        let request = match requests.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(request) => request,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        reporter.wake(); // A queued slot is available for the latest pending request.
        if cancelled.load(Ordering::Acquire) {
            break;
        }
        let changed = previous.as_ref().is_none_or(|old| {
            !Arc::ptr_eq(&old.world, &request.world)
                || old.cities != request.cities
                || old.meridian != request.meridian
                || old.scale != request.scale
                || old.version != request.version
                || old.snapshot.observations != request.snapshot.observations
        });
        if changed {
            let addresses: HashSet<_> = request
                .snapshot
                .observations
                .iter()
                .map(|observation| observation.tuple.remote.address())
                .collect();
            if version != Some(request.version) {
                cache.clear();
                version = Some(request.version);
            }
            cache.retain(|address, _| addresses.contains(address));
            let Ok(geo) = geolocation.lock() else {
                break;
            };
            let actual_version = geo.report().version;
            if actual_version != request.version {
                continue;
            }
            for address in addresses {
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                cache.entry(address).or_insert_with(|| {
                    // Private/local endpoints remain counted, without misleading
                    // geographic destinations even if a dataset supplies one.
                    if unplottable(address).is_none() {
                        geo.find(address).ok().flatten()
                    } else {
                        None
                    }
                });
            }
            drop(geo);
            prepared = Arc::new(Model::arrange(
                &request.world,
                request
                    .snapshot
                    .observations
                    .iter()
                    .map(|observation| Located {
                        observation,
                        location: cache
                            .get(&observation.tuple.remote.address())
                            .and_then(Option::as_ref),
                    }),
                request.cities,
                request.meridian,
                request.scale,
            ));
        }
        if !reporter.send(Reply::Scene {
            id: request.id,
            model: Arc::clone(&prepared),
            rows: Arc::new(Rows::prepare(
                &request.snapshot,
                Some(&prepared),
                &request.names,
            )),
            at: request.snapshot.interval.end,
            sampled: request.sampled,
        }) {
            break;
        }
        previous = Some(request);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unplottable {
    Loopback,
    Private,
    LinkLocal,
    Shared,
    Multicast,
    Broadcast,
    Unspecified,
    Reserved,
}

impl Unplottable {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Loopback => "Local loopback · no geographic destination",
            Self::Private => "Private / LAN address · no geographic destination",
            Self::LinkLocal => "Link-local address · no geographic destination",
            Self::Shared => "Shared address space · no public geographic destination",
            Self::Multicast => "Multicast · no single geographic destination",
            Self::Broadcast => "Broadcast · no single geographic destination",
            Self::Unspecified => "Unspecified address · no geographic destination",
            Self::Reserved => "Reserved address · no geographic destination",
        }
    }
}

/// One classification policy serves lookup admission and prepared row reasons.
pub(crate) fn unplottable(address: IpAddr) -> Option<Unplottable> {
    match address {
        IpAddr::V4(ip) => {
            if ip.is_loopback() {
                Some(Unplottable::Loopback)
            } else if ip.is_private() {
                Some(Unplottable::Private)
            } else if ip.is_link_local() {
                Some(Unplottable::LinkLocal)
            } else if ip.is_broadcast() {
                Some(Unplottable::Broadcast)
            } else if ip.is_unspecified() {
                Some(Unplottable::Unspecified)
            } else if ip.is_multicast() {
                Some(Unplottable::Multicast)
            } else if ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]) {
                Some(Unplottable::Shared)
            } else if ip.octets()[0] == 0 || ip.octets()[0] >= 240 {
                Some(Unplottable::Reserved)
            } else {
                None
            }
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                unplottable(IpAddr::V4(ip))
            } else if ip.is_loopback() {
                Some(Unplottable::Loopback)
            } else if ip.is_unspecified() {
                Some(Unplottable::Unspecified)
            } else if ip.is_unicast_link_local() {
                Some(Unplottable::LinkLocal)
            } else if ip.is_unique_local() {
                Some(Unplottable::Private)
            } else if ip.is_multicast() {
                Some(Unplottable::Multicast)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_endpoints_keep_distinct_reasons_without_a_geographic_lookup() {
        for (address, reason) in [
            ("127.0.0.1", Unplottable::Loopback),
            ("10.0.0.1", Unplottable::Private),
            ("169.254.1.2", Unplottable::LinkLocal),
            ("100.64.1.2", Unplottable::Shared),
            ("224.0.0.1", Unplottable::Multicast),
            ("255.255.255.255", Unplottable::Broadcast),
            ("0.0.0.0", Unplottable::Unspecified),
            ("240.0.0.1", Unplottable::Reserved),
            ("::1", Unplottable::Loopback),
            ("fc00::1", Unplottable::Private),
            ("fe80::1", Unplottable::LinkLocal),
            ("ff02::1", Unplottable::Multicast),
            ("::", Unplottable::Unspecified),
            ("::ffff:127.0.0.1", Unplottable::Loopback),
        ] {
            assert_eq!(unplottable(address.parse().unwrap()), Some(reason));
        }
        for address in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert_eq!(unplottable(address.parse().unwrap()), None);
        }
    }
}
