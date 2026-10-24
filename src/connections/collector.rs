use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, mpsc::Receiver},
    time::Instant,
};

use super::{
    Bytes, Coverage, Event, Highlight, Model, ProcessId, Protocol, Snapshot, Table, TcpRow, Tuple,
    etw::{self, Clock, Kind},
    processes::{Census, Processes},
    tables,
};

#[derive(Clone, Debug)]
pub(crate) struct Publication {
    pub snapshot: Arc<Snapshot>,
    pub sampled: Instant,
    pub limitation: Option<Arc<str>>,
    pub names: Arc<HashMap<ProcessId, Box<str>>>,
}

/// One native source coordinator. Work launches its consumer separately; this
/// worker owns table reconciliation, identity evidence and measurement cadence.
pub(crate) struct Collector {
    clock: Clock,
    model: Model,
    processes: Processes,
    pending: VecDeque<etw::Record>,
    session: Option<etw::Session>,
    events: Option<Receiver<etw::Record>>,
    control: Option<Arc<etw::Control>>,
    limitation: Option<Arc<str>>,
    last_losses: (u64, u64),
    discarded: u64,
    loss_boundary: std::time::Duration,
    unresolved_system: Bytes,
}

impl Collector {
    #[cfg(test)]
    pub(crate) fn trace_name(&self) -> Option<String> {
        self.session.as_ref().map(etw::Session::trace_name)
    }
    #[cfg(test)]
    pub(crate) fn trace_statistics(&mut self) -> io::Result<etw::Statistics> {
        self.session
            .as_mut()
            .ok_or_else(|| io::Error::other("Owned trace unavailable"))?
            .statistics()
    }
    pub fn new() -> io::Result<(Self, Option<etw::Consumer>)> {
        let clock = Clock::new()?;
        let mut collector = Self {
            clock,
            model: Model::new(clock.now()?),
            processes: Processes::default(),
            pending: VecDeque::new(),
            session: None,
            events: None,
            control: None,
            limitation: None,
            last_losses: (0, 0),
            discarded: 0,
            loss_boundary: std::time::Duration::ZERO,
            unresolved_system: Bytes::default(),
        };
        let consumer = match etw::Session::start().and_then(|session| {
            let (consumer, events, control) = session.consumer(clock)?;
            collector.session = Some(session);
            collector.events = Some(events);
            collector.control = Some(control);
            Ok(consumer)
        }) {
            Ok(consumer) => Some(consumer),
            Err(error) => {
                collector.limitation = Some(format!("Traffic collection unavailable: {error}. Showing TCP table observations; rates and UDP are unavailable.").into());
                None
            }
        };
        Ok((collector, consumer))
    }

    pub fn source_failed(&mut self, error: String) {
        self.limitation = Some(format!("Traffic consumer stopped: {error}").into());
    }

    pub fn poll(&mut self) {
        for _ in 0..4096 {
            let Some(event) = self
                .events
                .as_ref()
                .and_then(|events| events.try_recv().ok())
            else {
                break;
            };
            if self.pending.len() == 32768
                && let Some(discarded) = self.pending.pop_front()
            {
                self.discarded += 1;
                if let etw::Record::Network(fact) = discarded
                    && let Kind::Transfer(bytes) = fact.kind
                {
                    self.unattributed(fact, bytes);
                }
            }
            self.pending.push_back(event);
        }
    }

    pub fn sample(&mut self, highlight: Highlight) -> io::Result<Publication> {
        let before = Census::read(self.clock);
        if let Ok(census) = &before {
            self.processes.observe(census);
        }
        let capture = tables::read(|| self.clock.now().unwrap_or_default());
        let after = Census::read(self.clock);
        if let Ok(census) = &after {
            self.processes.observe(census);
        }
        self.poll();
        let now = self.clock.now()?;
        let sampled = Instant::now();
        let (losses, malformed) = self.losses();
        let lost = losses != self.last_losses;
        if lost {
            self.loss_boundary = now;
            self.processes.invalidate_trace(now);
            self.model.gap(self.loss_boundary);
        }
        // Lifecycle facts and census evidence settle identity before any row or
        // network event escapes into a snapshot. Timestamp ties stay ambiguous.
        let mut pending: Vec<_> = self.pending.drain(..).collect();
        pending.sort_by_key(|record| record.at());
        for record in &pending {
            if let etw::Record::Process(fact) = record {
                self.processes.event(*fact);
            }
        }
        self.processes.link(now);
        let mut rows = Vec::with_capacity(capture.rows.len());
        let mut identities_complete = before.is_ok() && after.is_ok();
        for row in &capture.rows {
            let process = before
                .as_ref()
                .ok()
                .zip(after.as_ref().ok())
                .and_then(|(first, second)| first.stable(second, row.pid))
                .and_then(|sequence| self.processes.sequence(row.pid, sequence));
            if let Some(process) = process {
                rows.push(TcpRow {
                    tuple: Tuple {
                        process,
                        local: row.local,
                        remote: row.remote,
                    },
                    state: row.state,
                });
            } else {
                identities_complete = false;
            }
        }
        // A census closes a conservative continuity interval. Events newer than
        // it wait for the next census; old ambiguous events stay in byte totals
        // without being attached to an arbitrary current PID owner.
        let cutoff = before.as_ref().map_or(now, |census| census.window.begin);
        self.accept_network(pending, cutoff);
        let complete_table = capture.failures.is_empty() && identities_complete;
        self.model.reconcile(Table {
            capture: capture.window,
            complete: complete_table,
            rows,
        });
        if lost {
            // Include openings and table rows first seen in this lost batch.
            self.model.gap(self.loss_boundary);
        }
        let coverage = if lost {
            Coverage::Lost {
                events: losses.0.saturating_sub(self.last_losses.0),
                buffers: losses.1.saturating_sub(self.last_losses.1),
            }
        } else if self.session.is_none()
            || self.limitation.is_some()
            || !complete_table
            || malformed != 0
        {
            Coverage::Restricted
        } else {
            if self.processes.has_rundown() {
                Coverage::Complete
            } else {
                Coverage::WarmingUp
            }
        };
        self.last_losses = losses;
        let census_error = before
            .as_ref()
            .err()
            .or_else(|| after.as_ref().err())
            .map(|error| Arc::<str>::from(format!("Process identity unavailable: {error}")));
        let table_limitation = capture
            .failures
            .first()
            .map(|(family, error)| {
                Arc::<str>::from(format!("TCP table capture incomplete (family {family}): {error}"))
            })
            .or_else(|| {
                (!identities_complete).then(|| {
                    "TCP table process identities changed or were unavailable during capture; unmatched rows remain uncertain.".into()
                })
            });
        let attribution = self.take_attribution_limitation();
        let snapshot = Arc::new(self.model.publish(now, highlight, coverage));
        let names = self
            .processes
            .names(&snapshot.observations, after.as_ref().ok(), self.clock);
        Ok(Publication {
            snapshot,
            sampled,
            names,
            limitation: census_error.or_else(|| self.limitation.clone()).or(table_limitation).or_else(|| {
                if malformed != 0 { Some("Unsupported or malformed trace records; traffic coverage is incomplete.".into()) }
                else if self.session.is_some() && !self.processes.has_rundown() { Some("Waiting for process lifecycle rundown; traffic coverage is warming up.".into()) }
                else { attribution }
            }),
        })
    }

    fn accept_network(&mut self, pending: Vec<etw::Record>, cutoff: std::time::Duration) {
        // A late batch can describe a complete TCP lifetime before the latest
        // empty table capture. Settle its exact lifecycle bounds before bytes:
        // an inferred close interval alone cannot safely assign those transfers.
        for record in &pending {
            if let etw::Record::Network(fact) = record
                && fact.at < cutoff
                && !matches!(fact.kind, Kind::Transfer(_))
            {
                self.accept(*fact);
            }
        }
        for record in pending {
            let etw::Record::Network(fact) = record else {
                continue;
            };
            if fact.at >= cutoff {
                self.pending.push_back(record);
            } else if matches!(fact.kind, Kind::Transfer(_)) {
                self.accept(fact);
            }
        }
    }

    fn losses(&mut self) -> ((u64, u64), u64) {
        let mut losses = (self.discarded, 0);
        let mut malformed = 0;
        if let Some(control) = &self.control {
            let (dropped, invalid) = control.losses();
            losses.0 += dropped + invalid;
            malformed = invalid;
        }
        if let Some(session) = &mut self.session {
            match session.statistics() {
                Ok(stats) => {
                    losses.0 += u64::from(stats.events_lost);
                    losses.1 = u64::from(stats.buffers_lost);
                }
                Err(error) => {
                    self.limitation = Some(format!("Trace coverage unavailable: {error}").into())
                }
            }
        }
        (losses, malformed)
    }

    fn accept(&mut self, fact: etw::Fact) {
        // Loss has no event timestamp. Keep the boundary across publications:
        // some queued facts wait for a later census before reaching this path.
        let uncertain = fact.protocol == Protocol::Tcp && fact.at <= self.loss_boundary;
        if uncertain && let Kind::Transfer(bytes) = fact.kind {
            self.unattributed(fact, bytes);
            return;
        }
        // PID 4 does not prove another application's socket ownership. Accept
        // correlation only inside one independently observed TCP lifetime and
        // that same process episode. Unmatched native tuples stay unattributed;
        // neither traffic volume nor a UDP flow establishes TCP ownership.
        let process =
            if fact.protocol == Protocol::Tcp && fact.pid == 4 && fact.kind != Kind::Opened {
                self.model
                    .tcp_owner(fact.local, fact.remote, fact.at)
                    .filter(|owner| self.processes.resolve(owner.pid, fact.at) == Some(*owner))
            } else {
                self.processes.resolve(fact.pid, fact.at)
            };
        let scoped = fact.local.scope() == 0 && fact.remote.scope() == 0
            && [fact.local.address(), fact.remote.address()].iter().all(|address| {
                !matches!(address, std::net::IpAddr::V6(ip) if ip.is_unicast_link_local() || ip.is_multicast())
            });
        let Some(process) = process.filter(|_| scoped) else {
            if let Kind::Transfer(bytes) = fact.kind {
                self.unattributed(fact, bytes);
            }
            return;
        };
        let tuple = Tuple {
            process,
            local: fact.local,
            remote: fact.remote,
        };
        self.model.event(match fact.kind {
            Kind::Opened => Event::TcpOpened { tuple, at: fact.at },
            Kind::Closed => Event::TcpClosed { tuple, at: fact.at },
            Kind::Transfer(bytes) => Event::Transfer {
                tuple,
                protocol: fact.protocol,
                at: fact.at,
                bytes,
            },
        });
        if uncertain && fact.kind == Kind::Opened {
            self.model.gap(self.loss_boundary);
        }
    }

    fn unattributed(&mut self, fact: etw::Fact, bytes: Bytes) {
        if fact.pid == 4 && fact.protocol == Protocol::Tcp {
            self.unresolved_system.received += bytes.received;
            self.unresolved_system.sent += bytes.sent;
        }
        self.model
            .unattributed(fact.protocol, fact.local, fact.remote, fact.at, bytes);
    }

    fn take_attribution_limitation(&mut self) -> Option<Arc<str>> {
        let bytes = std::mem::take(&mut self.unresolved_system);
        bytes.active().then(|| {
            format!(
                "System-context traffic has no verified socket owner ({} received / {} sent bytes this interval); it remains in measured totals.",
                bytes.received, bytes.sent
            )
            .into()
        })
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // This worker closes consumption and its own session. Consumer retains
        // callback inputs independently until ProcessTrace has actually returned.
        if let Some(control) = &self.control {
            let _ = control.close();
        }
        if let Some(session) = &mut self.session {
            let _ = session.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::{Bytes, Endpoint, Window};
    use std::{collections::HashMap, time::Duration};

    fn source() -> Collector {
        Collector {
            clock: Clock::new().unwrap(),
            model: Model::new(Duration::ZERO),
            processes: Processes::default(),
            pending: VecDeque::new(),
            session: None,
            events: None,
            control: None,
            limitation: None,
            last_losses: (0, 0),
            discarded: 0,
            loss_boundary: Duration::ZERO,
            unresolved_system: Bytes::default(),
        }
    }

    #[test]
    fn delayed_completed_tcp_lifetimes_keep_their_bytes_after_a_later_empty_table() {
        let at = Duration::from_secs;
        let mut source = source();
        for (begin, end) in [(1, 2), (20, 21)] {
            source.processes.observe(&Census {
                window: Window {
                    begin: at(begin),
                    end: at(end),
                },
                sequences: HashMap::from([(42, 100)]),
            });
        }
        source.model.reconcile(Table {
            capture: Window {
                begin: at(10),
                end: at(11),
            },
            complete: true,
            rows: vec![],
        });
        let fact = |time, kind| {
            etw::Record::Network(etw::Fact {
                at: at(time),
                pid: 42,
                protocol: Protocol::Tcp,
                local: Endpoint::new("127.0.0.1:50000".parse().unwrap()),
                remote: Endpoint::new("127.0.0.1:50001".parse().unwrap()),
                kind,
            })
        };
        source.accept_network(
            vec![
                fact(3, Kind::Opened),
                fact(
                    4,
                    Kind::Transfer(Bytes {
                        received: 10,
                        sent: 20,
                    }),
                ),
                fact(5, Kind::Closed),
                fact(6, Kind::Opened),
                fact(
                    7,
                    Kind::Transfer(Bytes {
                        received: 30,
                        sent: 40,
                    }),
                ),
                fact(8, Kind::Closed),
            ],
            at(20),
        );
        let snapshot = source.model.publish(
            at(22),
            Highlight {
                bright: at(10),
                fade: at(20),
            },
            Coverage::Complete,
        );
        assert_eq!(
            snapshot.bytes,
            Bytes {
                received: 40,
                sent: 60
            }
        );
        assert_eq!(snapshot.unattributed, Bytes::default());
        let mut assigned: Vec<_> = snapshot
            .observations
            .iter()
            .map(|row| {
                assert_eq!(row.life, super::super::Life::EndedTcp);
                row.bytes.received
            })
            .collect();
        assigned.sort_unstable();
        assert_eq!(assigned, [10, 30]);
        assert_ne!(snapshot.observations[0].id, snapshot.observations[1].id);
    }

    #[test]
    fn system_context_traffic_uses_socket_lifetimes_without_inheriting_reused_or_lost_ownership() {
        let at = Duration::from_secs;
        let mut source = source();
        for (begin, end) in [(1, 2), (20, 21)] {
            source.processes.observe(&Census {
                window: Window {
                    begin: at(begin),
                    end: at(end),
                },
                sequences: HashMap::from([(4, 1), (42, 100), (43, 101)]),
            });
        }
        let fact = |pid, time, kind| etw::Fact {
            at: at(time),
            pid,
            protocol: Protocol::Tcp,
            local: Endpoint::new("192.168.1.2:50000".parse().unwrap()),
            remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
            kind,
        };
        let received = |bytes| {
            Kind::Transfer(Bytes {
                received: bytes,
                sent: 0,
            })
        };
        source.accept(fact(4, 3, received(7))); // No ownership evidence.
        assert!(source.take_attribution_limitation().is_some());
        assert!(source.take_attribution_limitation().is_none());
        source.accept(fact(42, 4, Kind::Opened));
        source.accept(fact(4, 5, received(100)));
        source.accept(fact(4, 6, Kind::Closed));
        source.accept(fact(43, 8, Kind::Opened));
        source.accept(fact(4, 5, received(50))); // Late delivery belongs to 42.
        source.accept(fact(4, 9, received(200)));
        source.accept(fact(4, 8, received(30))); // Opening boundary is ambiguous.
        source.model.gap(at(10));
        source.accept(fact(4, 11, received(40)));
        source.accept(fact(43, 12, Kind::Opened));
        source.accept(fact(4, 13, received(300)));
        source.accept(fact(43, 14, Kind::Closed));
        source.accept(fact(4, 16, Kind::Opened)); // A genuinely System-owned socket.
        source.accept(fact(4, 17, received(20)));
        let snapshot = source.model.publish(
            at(19),
            Highlight {
                bright: at(10),
                fade: at(20),
            },
            Coverage::Complete,
        );
        assert_eq!(snapshot.bytes.received, 747);
        assert_eq!(snapshot.unattributed.received, 77);
        assert!(source.take_attribution_limitation().is_some());
        assert!(source.take_attribution_limitation().is_none());
        let mut assigned: Vec<_> = snapshot
            .observations
            .iter()
            .map(|row| (row.tuple.process.pid, row.bytes.received))
            .collect();
        assigned.sort_unstable();
        assert_eq!(assigned, [(4, 20), (42, 150), (43, 200), (43, 300)]);
        assert_eq!(
            snapshot.bytes.received,
            snapshot.unattributed.received + assigned.iter().map(|(_, bytes)| bytes).sum::<u64>()
        );
    }

    #[test]
    fn queued_openings_cannot_escape_a_loss_boundary_in_a_later_publication() {
        let at = Duration::from_secs;
        let mut source = source();
        source.loss_boundary = at(5);
        for (begin, end) in [(1, 2), (12, 13)] {
            source.processes.observe(&Census {
                window: Window {
                    begin: at(begin),
                    end: at(end),
                },
                sequences: HashMap::from([(42, 100)]),
            });
        }
        let fact = |time, kind| etw::Fact {
            at: at(time),
            pid: 42,
            protocol: Protocol::Tcp,
            local: Endpoint::new("192.168.1.2:50000".parse().unwrap()),
            remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
            kind,
        };
        source.accept(fact(3, Kind::Opened));
        source.accept(fact(
            4,
            Kind::Transfer(Bytes {
                received: 90,
                sent: 10,
            }),
        ));
        // The missed successor opening happened inside the gap. Its later
        // traffic must not attach to the queued old opening either.
        source.accept(fact(
            6,
            Kind::Transfer(Bytes {
                received: 40,
                sent: 20,
            }),
        ));
        let highlight = Highlight {
            bright: at(10),
            fade: at(20),
        };
        let missing = source.model.publish(at(7), highlight, Coverage::Complete);
        assert_eq!(
            missing.unattributed,
            Bytes {
                received: 130,
                sent: 30
            }
        );
        assert!(
            missing
                .observations
                .iter()
                .all(|row| !row.bytes.active() && row.rates.is_none())
        );
        source.accept(fact(7, Kind::Opened));
        // A later delivery of an old opening must not taint the independently
        // established successor that is already present after the boundary.
        source.accept(fact(4, Kind::Opened));
        source.accept(fact(
            8,
            Kind::Transfer(Bytes {
                received: 500,
                sent: 100,
            }),
        ));
        let next = source.model.publish(at(10), highlight, Coverage::Complete);
        assert_eq!(
            next.total_bytes,
            Bytes {
                received: 630,
                sent: 130
            }
        );
        assert!(next.observations.iter().any(|row| row.bytes
            == Bytes {
                received: 500,
                sent: 100
            }
            && row.rates.is_some()));
    }
}
