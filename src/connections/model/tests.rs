use super::*;

const HIGHLIGHT: Highlight = Highlight {
    bright: Duration::from_secs(10),
    fade: Duration::from_secs(20),
};

fn time(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn tuple() -> Tuple {
    Tuple {
        process: ProcessId {
            pid: 42,
            generation: 1,
        },
        local: Endpoint::new("192.168.1.2:50000".parse().unwrap()),
        remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
    }
}

fn open(model: &mut Model, at: u64) {
    model.event(Event::TcpOpened {
        tuple: tuple(),
        at: time(at),
    });
}

fn close(model: &mut Model, at: u64) {
    model.event(Event::TcpClosed {
        tuple: tuple(),
        at: time(at),
    });
}

fn transfer(model: &mut Model, protocol: Protocol, at: u64, received: u64, sent: u64) {
    model.event(Event::Transfer {
        tuple: tuple(),
        protocol,
        at: time(at),
        bytes: Bytes { received, sent },
    });
}

fn publish(model: &mut Model, at: u64) -> Snapshot {
    model.publish(time(at), HIGHLIGHT, Coverage::Complete)
}

fn scan(begin: u64, end: u64, present: bool, complete: bool) -> Table {
    Table {
        capture: Window {
            begin: time(begin),
            end: time(end),
        },
        complete,
        rows: if present {
            vec![TcpRow {
                tuple: tuple(),
                state: TcpState::Established,
            }]
        } else {
            vec![]
        },
    }
}

#[test]
fn transferring_counts_positive_measured_observations_once_including_unplottable_endpoints() {
    let mut model = Model::new(time(0));
    let mut flows = Vec::new();
    for (index, (protocol, received, sent)) in [
        (Protocol::Tcp, 0, 0),
        (Protocol::Tcp, 0, 12),
        (Protocol::Udp, 13, 0),
        (Protocol::Tcp, 14, 15),
        (Protocol::Tcp, 16, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let flow = Tuple {
            local: Endpoint::new("127.0.0.1:50000".parse().unwrap()),
            remote: Endpoint::new(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                9000 + index as u16,
            ))),
            ..tuple()
        };
        if protocol == Protocol::Tcp {
            model.event(Event::TcpOpened {
                tuple: flow,
                at: time(1),
            });
        }
        model.event(Event::Transfer {
            tuple: flow,
            protocol,
            at: time(2),
            bytes: Bytes { received, sent },
        });
        flows.push(flow);
    }
    model.event(Event::TcpClosed {
        tuple: flows[3],
        at: time(3),
    });
    let unavailable = flows[4];
    model.unattributed(
        Protocol::Tcp,
        unavailable.local,
        unavailable.remote,
        time(2),
        Bytes {
            received: 1,
            sent: 0,
        },
    );
    let active = publish(&mut model, 4);
    assert_eq!(active.observations.len(), 5);
    assert_eq!(active.counts.transferring, 3);
    let unavailable = active
        .observations
        .iter()
        .find(|observation| observation.tuple == unavailable)
        .unwrap();
    assert!(unavailable.bytes.active() && unavailable.rates.is_none());
    assert_eq!(publish(&mut model, 5).counts.transferring, 0);
    model.event(Event::Transfer {
        tuple: flows[1],
        protocol: Protocol::Tcp,
        at: time(6),
        bytes: Bytes {
            received: 10,
            sent: 0,
        },
    });
    let restricted = model.publish(time(7), HIGHLIGHT, Coverage::Restricted);
    assert!(restricted.bytes.active());
    assert_eq!(restricted.counts.transferring, 0);
    assert!(
        restricted
            .observations
            .iter()
            .all(|row| row.rates.is_none())
    );
}

#[test]
fn endpoint_ownership_requires_one_lifetime_and_retires_with_its_history() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(2, 3, true, true));
    let first = tuple();
    assert_eq!(model.tcp_owner(first.local, first.remote, time(2)), None);
    assert_eq!(
        model.tcp_owner(first.local, first.remote, time(4)),
        Some(first.process)
    );
    let second = Tuple {
        process: ProcessId {
            pid: 43,
            generation: 2,
        },
        ..first
    };
    model.event(Event::TcpOpened {
        tuple: second,
        at: time(4),
    });
    assert_eq!(model.tcp_owner(first.local, first.remote, time(5)), None);
    model.event(Event::TcpClosed {
        tuple: first,
        at: time(6),
    });
    assert_eq!(
        model.tcp_owner(first.local, first.remote, time(7)),
        Some(second.process)
    );
    model.event(Event::TcpClosed {
        tuple: second,
        at: time(8),
    });
    publish(&mut model, 9);
    publish(&mut model, 40);
    assert!(model.endpoints.is_empty());
    assert_eq!(model.tcp_owner(first.local, first.remote, time(7)), None);
}

#[test]
fn a_trace_gap_never_assigns_unseen_tcp_replacements_to_the_previous_incarnation() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    let before = publish(&mut model, 2);
    let previous = before.observations[0].id;
    model.gap(time(2));
    transfer(&mut model, Protocol::Tcp, 3, 90, 10);
    let missing = publish(&mut model, 4);
    assert_eq!(
        missing.unattributed,
        Bytes {
            received: 90,
            sent: 10
        }
    );
    assert_eq!(missing.observations[0].id, previous);
    assert!(missing.observations[0].rates.is_none());
    open(&mut model, 5);
    transfer(&mut model, Protocol::Tcp, 6, 500, 100);
    // Late evidence inside the old gap must not mask or change the successor.
    transfer(&mut model, Protocol::Tcp, 3, 70, 30);
    let next = publish(&mut model, 8);
    let successor = next
        .observations
        .iter()
        .find(|observation| observation.id != previous)
        .unwrap();
    assert_eq!(
        successor.bytes,
        Bytes {
            received: 500,
            sent: 100
        }
    );
    assert!(successor.rates.is_some());
    assert_eq!(
        next.unattributed,
        Bytes {
            received: 70,
            sent: 30
        }
    );
    assert_eq!(
        next.total_bytes,
        Bytes {
            received: 660,
            sent: 140
        }
    );
}

#[test]
fn a_connection_ended_before_sampling_gets_one_first_publication_and_no_replayed_bytes() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    transfer(&mut model, Protocol::Tcp, 2, 2048, 4096);
    close(&mut model, 3);
    let first = publish(&mut model, 4);
    assert_eq!(first.observations.len(), 1);
    let connection = &first.observations[0];
    assert_eq!(connection.introduced, time(4));
    assert_eq!(connection.life, Life::EndedTcp);
    assert_eq!(
        connection.rates,
        Some(Rates {
            received: 512.0,
            sent: 1024.0
        })
    );
    assert_eq!(
        HIGHLIGHT.opacity(connection.introduced, time(14), false),
        1.0
    );
    assert_eq!(
        HIGHLIGHT.opacity(connection.introduced, time(24), false),
        0.5
    );
    let next = publish(&mut model, 6);
    assert_eq!(next.observations[0].id, connection.id);
    assert_eq!(next.observations[0].introduced, time(4));
    assert_eq!(next.bytes, Bytes::default());
    assert_eq!(
        next.observations[0].rates,
        Some(Rates {
            received: 0.0,
            sent: 0.0
        })
    );
    let expired = publish(&mut model, 34);
    assert!(expired.observations.is_empty());
    assert_eq!(expired.total_bytes, first.total_bytes);
    let longer = Highlight {
        bright: time(60),
        fade: time(60),
    };
    assert!(
        model
            .publish(time(35), longer, Coverage::Complete)
            .observations
            .is_empty()
    );
    // A retained earlier snapshot cannot extend the collector's deadline.
    assert_eq!(first.observations[0].life, Life::EndedTcp);
}

#[test]
fn live_idle_tcp_survives_its_highlight_and_duration_changes_do_not_restart_it() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    let first = publish(&mut model, 2);
    let current = publish(&mut model, 45);
    assert_eq!(current.observations[0].id, first.observations[0].id);
    assert_eq!(current.observations[0].introduced, time(2));
    assert_eq!(
        current.observations[0].life,
        Life::Tcp(TcpState::Established)
    );
    assert_eq!(HIGHLIGHT.opacity(time(2), time(45), false), 0.0);
    assert_eq!(HIGHLIGHT.deadline(time(2), time(3), false), Some(time(12)));
    assert_eq!(HIGHLIGHT.deadline(time(2), time(3), true), Some(time(32)));
    assert_eq!(HIGHLIGHT.opacity(time(2), time(31), true), 1.0);
    assert_eq!(HIGHLIGHT.opacity(time(2), time(32), true), 0.0);
    let immediate = Highlight {
        bright: time(10),
        fade: time(0),
    };
    assert_eq!(immediate.opacity(time(2), time(11), false), 1.0);
    assert_eq!(immediate.opacity(time(2), time(12), false), 0.0);
    assert_eq!(immediate.deadline(time(2), time(12), false), None);
    assert_eq!(current.observations[0].introduced, time(2));
}

#[test]
fn same_tuple_reconnect_keeps_ended_and_live_identities_and_routes_late_old_events() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    transfer(&mut model, Protocol::Tcp, 2, 100, 0);
    close(&mut model, 3);
    let old = publish(&mut model, 4).observations[0].clone();
    open(&mut model, 5);
    transfer(&mut model, Protocol::Tcp, 6, 0, 200);
    transfer(&mut model, Protocol::Tcp, 2, 70, 0);
    close(&mut model, 3);
    let snapshot = publish(&mut model, 8);
    assert_eq!(snapshot.observations.len(), 2);
    assert_eq!(snapshot.observations[0].id, old.id);
    assert_eq!(snapshot.observations[0].introduced, old.introduced);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(
        snapshot.observations[0].bytes,
        Bytes {
            received: 70,
            sent: 0
        }
    );
    assert_ne!(snapshot.observations[1].id, old.id);
    assert_eq!(
        snapshot.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
    assert_eq!(
        snapshot.observations[1].bytes,
        Bytes {
            received: 0,
            sent: 200
        }
    );
    assert_eq!(snapshot.unattributed, Bytes::default());
    let current = publish(&mut model, 34);
    assert_eq!(current.observations.len(), 1);
    assert_eq!(current.observations[0].id, snapshot.observations[1].id);
}

#[test]
fn equal_birth_timestamps_keep_measured_totals_but_cannot_claim_connection_rates() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    transfer(&mut model, Protocol::Tcp, 1, 10, 20);
    close(&mut model, 1);
    let snapshot = publish(&mut model, 2);
    assert_eq!(
        snapshot.bytes,
        Bytes {
            received: 10,
            sent: 20
        }
    );
    assert_eq!(snapshot.unattributed, snapshot.bytes);
    assert_eq!(
        snapshot.rates,
        Some(Rates {
            received: 5.0,
            sent: 10.0
        })
    );
    assert!(snapshot.observations[0].rates.is_none());
    assert_eq!(snapshot.ambiguous_events, 2);
    assert_eq!(
        snapshot.observations[0].life,
        Life::Tcp(TcpState::Established)
    );
}

#[test]
fn delayed_unique_opening_refines_a_startup_row_without_a_second_highlight() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(3, 4, true, true));
    let first = publish(&mut model, 5);
    open(&mut model, 2);
    open(&mut model, 2);
    transfer(&mut model, Protocol::Tcp, 3, 40, 80);
    let next = publish(&mut model, 6);
    assert_eq!(next.observations.len(), 1);
    assert_eq!(next.observations[0].id, first.observations[0].id);
    assert_eq!(
        next.observations[0].introduced,
        first.observations[0].introduced
    );
    assert_eq!(next.observations[0].bytes, next.bytes);
}

#[test]
fn table_seed_reconnect_does_not_assign_ambiguous_old_bytes_to_the_successor() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(1, 2, true, true));
    let old = publish(&mut model, 3).observations[0].id;
    open(&mut model, 10);
    transfer(&mut model, Protocol::Tcp, 5, 90, 0);
    close(&mut model, 5);
    transfer(&mut model, Protocol::Tcp, 11, 0, 100);
    let snapshot = publish(&mut model, 12);
    assert_eq!(snapshot.observations[0].id, old);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(snapshot.observations[0].bytes, Bytes::default());
    assert_eq!(
        snapshot.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
    assert_eq!(
        snapshot.observations[1].bytes,
        Bytes {
            received: 0,
            sent: 100
        }
    );
    assert_eq!(
        snapshot.unattributed,
        Bytes {
            received: 90,
            sent: 0
        }
    );
}

#[test]
fn stale_table_presence_never_resurrects_closed_tcp_or_retires_a_newer_opening() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    close(&mut model, 5);
    open(&mut model, 8);
    model.reconcile(scan(3, 4, true, true));
    model.reconcile(scan(6, 7, false, true));
    let snapshot = publish(&mut model, 9);
    assert_eq!(snapshot.observations.len(), 2);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(
        snapshot.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
}

#[test]
fn partial_tables_retire_nothing_and_reappearance_after_an_absence_gets_a_new_id() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(1, 2, true, true));
    let first = publish(&mut model, 3).observations[0].id;
    model.reconcile(scan(4, 5, false, false));
    assert_eq!(
        publish(&mut model, 6).observations[0].life,
        Life::Tcp(TcpState::Established)
    );
    model.reconcile(scan(7, 8, false, true));
    model.reconcile(scan(10, 11, true, true));
    let snapshot = publish(&mut model, 12);
    assert_eq!(snapshot.observations.len(), 2);
    assert_eq!(snapshot.observations[0].id, first);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_ne!(snapshot.observations[1].id, first);
    assert_eq!(
        snapshot.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
}

#[test]
fn a_close_seen_during_startup_without_its_opening_is_still_published_as_ended() {
    let mut model = Model::new(time(0));
    close(&mut model, 1);
    model.reconcile(scan(2, 3, false, true));
    let snapshot = publish(&mut model, 4);
    assert_eq!(snapshot.observations.len(), 1);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(snapshot.observations[0].introduced, time(4));
}

#[test]
fn udp_inactivity_creates_a_successor_without_claiming_a_tcp_socket_closed() {
    let mut model = Model::new(time(0));
    let long = Highlight {
        bright: time(90),
        fade: time(10),
    };
    transfer(&mut model, Protocol::Udp, 1, 20, 50);
    let first = model.publish(time(2), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 30, 30, 60);
    transfer(&mut model, Protocol::Udp, 60, 40, 70);
    transfer(&mut model, Protocol::Udp, 20, 7, 0);
    let next = model.publish(time(62), long, Coverage::Complete);
    assert_eq!(next.observations.len(), 2);
    assert_eq!(next.observations[0].id, first.observations[0].id);
    assert_eq!(
        next.observations[0].life,
        Life::ExpiredUdp {
            last_seen: time(30)
        }
    );
    assert_eq!(next.observations[0].introduced, time(2));
    assert_eq!(
        next.observations[0].bytes,
        Bytes {
            received: 37,
            sent: 60
        }
    );
    assert_eq!(
        next.observations[1].life,
        Life::Udp {
            last_seen: time(60)
        }
    );
    assert_ne!(next.observations[1].id, first.observations[0].id);
    assert_eq!(next.observations[1].introduced, time(62));
}

#[test]
fn coalescing_snapshots_preserves_cumulative_bytes_and_unavailable_rates_stay_missing() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    transfer(&mut model, Protocol::Tcp, 2, 100, 200);
    let first = publish(&mut model, 3);
    transfer(&mut model, Protocol::Tcp, 4, 50, 70);
    let skipped = publish(&mut model, 5);
    transfer(&mut model, Protocol::Tcp, 6, 20, 30);
    let latest = model.publish(
        time(7),
        HIGHLIGHT,
        Coverage::Lost {
            events: 1,
            buffers: 0,
        },
    );
    assert_eq!(
        latest.total_bytes,
        Bytes {
            received: 170,
            sent: 300
        }
    );
    assert_eq!(
        latest.total_bytes.received - first.total_bytes.received,
        skipped.bytes.received + latest.bytes.received
    );
    assert_eq!(
        latest.total_bytes.sent - first.total_bytes.sent,
        skipped.bytes.sent + latest.bytes.sent
    );
    assert!(latest.rates.is_none());
    assert!(latest.observations[0].rates.is_none());
    let next = model.publish(time(9), HIGHLIGHT, Coverage::Restricted);
    assert_eq!(next.bytes, Bytes::default());
    assert!(next.rates.is_none());
    assert!(next.observations[0].rates.is_none());
}

#[test]
fn mapped_ipv4_has_one_identity_and_ipv6_endpoint_scopes_stay_distinct() {
    let plain = Endpoint::new("192.0.2.1:443".parse().unwrap());
    let mapped = Endpoint::new("[::ffff:192.0.2.1]:443".parse().unwrap());
    assert_eq!(plain, mapped);
    assert_eq!(mapped.address(), "192.0.2.1".parse::<IpAddr>().unwrap());
    assert_eq!(mapped.port(), 443);
    assert_eq!(mapped.scope(), 0);
    assert_ne!(
        Endpoint::new("[fe80::1%3]:443".parse().unwrap()),
        Endpoint::new("[fe80::1%4]:443".parse().unwrap())
    );
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    let mut next = tuple();
    next.process.generation += 1;
    model.event(Event::TcpOpened {
        tuple: next,
        at: time(2),
    });
    let snapshot = publish(&mut model, 3);
    assert_eq!(snapshot.observations.len(), 2);
    assert_ne!(snapshot.observations[0].id, snapshot.observations[1].id);
    assert_eq!(
        snapshot.observations[0].tuple.process.pid,
        snapshot.observations[1].tuple.process.pid
    );
}

#[test]
fn many_connections_conserve_totals_and_keep_stable_publication_order() {
    let mut model = Model::new(time(0));
    for port in 1024..6024 {
        let mut tuple = tuple();
        tuple.local = Endpoint::new(SocketAddr::new(tuple.local.address(), port));
        model.event(Event::TcpOpened { tuple, at: time(1) });
        model.event(Event::Transfer {
            tuple,
            protocol: Protocol::Tcp,
            at: time(2),
            bytes: Bytes {
                received: port as u64,
                sent: 100,
            },
        });
    }
    let first = publish(&mut model, 3);
    let next = publish(&mut model, 5);
    assert_eq!(first.observations.len(), 5000);
    assert_eq!(first.bytes.sent, 500_000);
    assert_eq!(
        first.bytes.received,
        first
            .observations
            .iter()
            .map(|observation| observation.bytes.received)
            .sum()
    );
    assert!(
        first
            .observations
            .iter()
            .map(|observation| observation.id)
            .eq(next.observations.iter().map(|observation| observation.id))
    );
    assert_eq!(next.bytes, Bytes::default());
}

#[test]
fn delayed_open_after_table_absence_cannot_create_a_phantom_live_incarnation() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(1, 2, true, true));
    model.reconcile(scan(3, 4, false, true));
    model.reconcile(scan(5, 6, true, true));
    open(&mut model, 0);
    let snapshot = publish(&mut model, 7);
    assert_eq!(snapshot.observations.len(), 2);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(
        snapshot.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
    transfer(&mut model, Protocol::Tcp, 0, 100, 0);
    let later = publish(&mut model, 8);
    assert_eq!(later.observations[1].bytes, Bytes::default());
    assert_eq!(later.unattributed.received, 100);
}

#[test]
fn delayed_close_before_a_successor_preserves_its_separate_ended_startup_fact() {
    let mut model = Model::new(time(0));
    open(&mut model, 5);
    close(&mut model, 3);
    let snapshot = publish(&mut model, 6);
    assert_eq!(snapshot.observations.len(), 2);
    assert_eq!(
        snapshot.observations[0].life,
        Life::Tcp(TcpState::Established)
    );
    assert_eq!(snapshot.observations[1].life, Life::EndedTcp);
    assert_ne!(snapshot.observations[0].id, snapshot.observations[1].id);
}

#[test]
fn late_udp_evidence_updates_last_seen_and_expiry_from_one_timestamp() {
    let mut model = Model::new(time(0));
    let long = Highlight {
        bright: time(60),
        fade: time(20),
    };
    transfer(&mut model, Protocol::Udp, 1, 10, 20);
    let first = model.publish(time(2), long, Coverage::Complete);
    let expired = model.publish(time(32), long, Coverage::Complete);
    assert_eq!(
        expired.observations[0].life,
        Life::ExpiredUdp { last_seen: time(1) }
    );
    transfer(&mut model, Protocol::Udp, 20, 30, 40);
    let updated = model.publish(time(33), long, Coverage::Complete);
    assert_eq!(updated.observations[0].id, first.observations[0].id);
    assert_eq!(updated.observations[0].introduced, time(2));
    assert_eq!(
        updated.observations[0].life,
        Life::Udp {
            last_seen: time(20)
        }
    );
    assert_eq!(
        updated.observations[0].bytes,
        Bytes {
            received: 30,
            sent: 40
        }
    );
    assert_eq!(
        model
            .publish(time(50), long, Coverage::Complete)
            .observations[0]
            .life,
        Life::ExpiredUdp {
            last_seen: time(20)
        }
    );
}

#[test]
fn zero_elapsed_publication_keeps_pending_bytes_for_a_measurable_interval() {
    let mut model = Model::new(time(0));
    open(&mut model, 0);
    transfer(&mut model, Protocol::Tcp, 1, 40, 60);
    let zero = publish(&mut model, 0);
    assert_eq!(zero.interval, Window::point(time(0)));
    assert_eq!(zero.bytes, Bytes::default());
    assert_eq!(zero.observations[0].bytes, Bytes::default());
    assert!(zero.rates.is_none());
    let measured = publish(&mut model, 2);
    assert_eq!(
        measured.bytes,
        Bytes {
            received: 40,
            sent: 60
        }
    );
    assert_eq!(
        measured.observations[0].rates,
        Some(Rates {
            received: 20.0,
            sent: 30.0
        })
    );
    assert_eq!(measured.total_bytes, zero.total_bytes);
    assert_eq!(measured.coverage, Coverage::Complete);
}

#[test]
fn retired_old_source_bounds_cannot_recreate_or_change_a_live_successor() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    close(&mut model, 2);
    publish(&mut model, 3);
    open(&mut model, 10);
    let successor = publish(&mut model, 34).observations[0].id;
    open(&mut model, 1);
    close(&mut model, 2);
    transfer(&mut model, Protocol::Tcp, 35, 200, 400);
    transfer(&mut model, Protocol::Tcp, 1, 100, 200);
    let next = publish(&mut model, 36);
    assert_eq!(next.observations.len(), 1);
    assert_eq!(next.observations[0].id, successor);
    assert_eq!(next.observations[0].life, Life::Tcp(TcpState::Established));
    assert_eq!(
        next.observations[0].bytes,
        Bytes {
            received: 200,
            sent: 400
        }
    );
    assert_eq!(
        next.observations[0].rates,
        Some(Rates {
            received: 100.0,
            sent: 200.0
        })
    );
    assert_eq!(
        next.unattributed,
        Bytes {
            received: 100,
            sent: 200
        }
    );
}

#[test]
fn a_late_udp_bridge_preserves_identities_and_reports_overlap_without_guessing() {
    let mut model = Model::new(time(0));
    let long = Highlight {
        bright: time(90),
        fade: time(20),
    };
    transfer(&mut model, Protocol::Udp, 1, 10, 0);
    let first = model.publish(time(2), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 40, 20, 0);
    let successor = model.publish(time(41), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 20, 30, 0);
    transfer(&mut model, Protocol::Udp, 45, 40, 0);
    let ambiguous = model.publish(time(46), long, Coverage::Complete);
    assert_eq!(ambiguous.observations.len(), 2);
    assert_eq!(ambiguous.observations[0].id, first.observations[0].id);
    assert_eq!(ambiguous.observations[1].id, successor.observations[1].id);
    assert_eq!(ambiguous.unattributed.received, 40);
    assert_eq!(ambiguous.bytes.received, 70);
    assert!(
        ambiguous
            .observations
            .iter()
            .all(|observation| observation.rates.is_none())
    );
}

#[test]
fn a_delayed_open_is_ended_by_known_absence_even_without_a_successor() {
    let mut model = Model::new(time(0));
    model.reconcile(scan(5, 6, false, true));
    open(&mut model, 1);
    let first = publish(&mut model, 7);
    assert_eq!(first.observations.len(), 1);
    assert_eq!(first.observations[0].life, Life::EndedTcp);
    assert_eq!(first.observations[0].introduced, time(7));
    assert!(publish(&mut model, 37).observations.is_empty());
}

#[test]
fn udp_overlap_marks_rates_unavailable_in_each_affected_interval() {
    let mut model = Model::new(time(0));
    let long = Highlight {
        bright: time(90),
        fade: time(20),
    };
    transfer(&mut model, Protocol::Udp, 1, 10, 0);
    model.publish(time(2), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 40, 20, 0);
    model.publish(time(41), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 20, 30, 0);
    model.publish(time(42), long, Coverage::Complete);
    transfer(&mut model, Protocol::Udp, 45, 40, 0);
    let ambiguous = model.publish(time(46), long, Coverage::Complete);
    assert_eq!(ambiguous.observations.len(), 2);
    assert_eq!(ambiguous.unattributed.received, 40);
    assert_eq!(ambiguous.bytes.received, 40);
    assert_eq!(ambiguous.total_bytes.received, 100);
    assert_eq!(ambiguous.ambiguous_events, 1);
    assert!(ambiguous.observations.iter().all(|observation| {
        observation.rates.is_none() && observation.bytes == Bytes::default()
    }));
}

#[test]
fn a_delayed_close_refines_an_inferred_end_without_changing_identity_or_age() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    let first = publish(&mut model, 2);
    model.reconcile(scan(5, 6, false, true));
    close(&mut model, 4);
    transfer(&mut model, Protocol::Tcp, 3, 40, 60);
    open(&mut model, 7);
    let refined = publish(&mut model, 8);
    assert_eq!(refined.observations.len(), 2);
    assert_eq!(refined.observations[0].id, first.observations[0].id);
    assert_eq!(refined.observations[0].introduced, time(2));
    assert_eq!(refined.observations[0].life, Life::EndedTcp);
    assert_eq!(
        refined.observations[0].bytes,
        Bytes {
            received: 40,
            sent: 60
        }
    );
    assert_eq!(
        refined.observations[1].life,
        Life::Tcp(TcpState::Established)
    );
    assert_eq!(refined.observations[1].bytes, Bytes::default());
    assert_eq!(refined.unattributed, Bytes::default());
    assert_eq!(refined.ambiguous_events, 0);
}

#[test]
fn contradictory_closes_do_not_overwrite_an_explicit_end() {
    let mut model = Model::new(time(0));
    open(&mut model, 1);
    close(&mut model, 4);
    close(&mut model, 2);
    transfer(&mut model, Protocol::Tcp, 3, 40, 60);
    let snapshot = publish(&mut model, 5);
    assert_eq!(snapshot.observations.len(), 1);
    assert_eq!(snapshot.observations[0].life, Life::EndedTcp);
    assert_eq!(
        snapshot.observations[0].bytes,
        Bytes {
            received: 40,
            sent: 60
        }
    );
    assert_eq!(snapshot.unattributed, Bytes::default());
    assert!(snapshot.observations[0].rates.is_none());
    assert_eq!(snapshot.ambiguous_events, 1);
}
