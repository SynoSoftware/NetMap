use super::*;

fn at(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}
fn census(begin: u64, end: u64, entries: &[(u32, u64)]) -> Census {
    Census {
        window: Window {
            begin: at(begin),
            end: at(end),
        },
        sequences: entries.iter().copied().collect(),
    }
}
fn fact(time: u64, pid: u32, key: u64, kind: ProcessKind) -> ProcessFact {
    ProcessFact {
        at: at(time),
        pid,
        key: Some(key),
        kind,
    }
}

#[test]
fn names_require_unchanged_instance_evidence_and_retire_with_observation_interest() {
    use crate::connections::{Bytes, Endpoint, Life, ObservationId, Tuple};

    let mut processes = Processes::default();
    let first = census(1, 2, &[(42, 100), (43, 200)]);
    processes.observe(&first);
    let original = processes.sequence(42, 100).unwrap();
    let inaccessible = processes.sequence(43, 200).unwrap();
    let replaced = census(3, 4, &[(42, 101), (43, 200)]);
    processes.accept_names(
        &first,
        &replaced,
        vec![(original, 100, Some("wrong-owner.exe".into()))],
    );
    assert!(!processes.names.contains_key(&original));
    processes.accept_names(
        &first,
        &first,
        vec![
            (original, 100, Some("original.exe".into())),
            (inaccessible, 200, None),
        ],
    );
    processes.observe(&replaced);
    let successor = processes.sequence(42, 101).unwrap();
    assert_ne!(original, successor);
    processes.accept_names(
        &replaced,
        &replaced,
        vec![(successor, 101, Some("successor.exe".into()))],
    );
    let row = |id, process| Observation {
        id: ObservationId(id),
        tuple: Tuple {
            process,
            local: Endpoint::new("127.0.0.1:50000".parse().unwrap()),
            remote: Endpoint::new("127.0.0.1:443".parse().unwrap()),
        },
        life: Life::EndedTcp,
        introduced: at(5),
        bytes: Bytes::default(),
        rates: None,
    };
    let clock = Clock::new().unwrap();
    let known = processes.names(
        &[row(1, original), row(2, successor), row(3, inaccessible)],
        None,
        clock,
    );
    assert_eq!(known[&original].as_ref(), "original.exe");
    assert_eq!(known[&successor].as_ref(), "successor.exe");
    assert!(!known.contains_key(&inaccessible));
    assert!(
        processes
            .names
            .get(&inaccessible)
            .is_some_and(Option::is_none)
    );
    let retired = processes.names(&[row(2, successor)], None, clock);
    assert_eq!(retired.len(), 1);
    assert!(!retired.contains_key(&original));
    assert!(!processes.names.contains_key(&original));
    assert!(!processes.names.contains_key(&inaccessible));
}

#[test]
fn lifecycle_binding_preserves_the_already_published_census_identity() {
    let mut processes = Processes::default();
    processes.observe(&census(2, 3, &[(42, 100)]));
    let original = processes.sequence(42, 100).unwrap();
    processes.event(fact(1, 42, 900, ProcessKind::Existing));
    processes.link(at(4));
    assert_eq!(processes.resolve(42, at(3)), Some(original));
    processes.observe(&census(6, 7, &[(42, 100)]));
    processes.event(fact(8, 42, 900, ProcessKind::Ended));
    processes.event(fact(10, 42, 901, ProcessKind::Started));
    processes.observe(&census(11, 12, &[(42, 101)]));
    processes.link(at(13));
    let successor = processes.sequence(42, 101).unwrap();
    assert_ne!(successor, original);
    assert_eq!(processes.resolve(42, at(5)), Some(original));
    assert_eq!(processes.resolve(42, at(9)), None);
    assert_eq!(processes.resolve(42, at(11)), Some(successor));
}

#[test]
fn a_pid_replacement_gap_is_not_filled_with_the_current_census_owner() {
    let mut processes = Processes::default();
    processes.observe(&census(1, 2, &[(42, 100)]));
    processes.observe(&census(5, 6, &[(42, 101)]));
    assert!(processes.resolve(42, at(3)).is_none());
    processes.observe(&census(9, 10, &[(42, 101)]));
    assert_eq!(processes.resolve(42, at(7)), processes.sequence(42, 101));
}

#[test]
fn short_lived_processes_and_reused_object_keys_keep_separate_episodes() {
    let mut processes = Processes::default();
    processes.event(fact(1, 42, 900, ProcessKind::Started));
    processes.event(fact(3, 42, 900, ProcessKind::Ended));
    processes.event(fact(5, 42, 900, ProcessKind::Started));
    // The old exit can arrive after the new start without closing it.
    processes.event(fact(3, 42, 900, ProcessKind::Ended));
    processes.link(at(8));
    let original = processes.resolve(42, at(2)).unwrap();
    let successor = processes.resolve(42, at(6)).unwrap();
    assert_ne!(original, successor);
    assert!(processes.resolve(42, at(1)).is_none());
    assert!(processes.resolve(42, at(3)).is_none());
    assert!(processes.resolve(42, at(4)).is_none());
}

#[test]
fn a_later_census_cannot_replace_an_identity_already_published_from_start_evidence() {
    let mut processes = Processes::default();
    processes.event(fact(1, 42, 900, ProcessKind::Started));
    processes.link(at(4));
    let original = processes.resolve(42, at(2)).unwrap();
    processes.observe(&census(5, 6, &[(42, 100)]));
    processes.link(at(7));
    assert_eq!(processes.sequence(42, 100), Some(original));
}

#[test]
fn rundown_is_observation_evidence_until_continuity_can_bind_it() {
    let mut processes = Processes::default();
    processes.observe(&census(1, 2, &[(42, 100)]));
    let original = processes.sequence(42, 100).unwrap();
    processes.event(fact(3, 42, 900, ProcessKind::Existing));
    processes.link(at(5));
    assert!(processes.resolve(42, at(4)).is_none());
    processes.observe(&census(6, 7, &[(42, 100)]));
    processes.link(at(8));
    assert_eq!(processes.resolve(42, at(4)), Some(original));
}

#[test]
fn trace_loss_retires_lifecycle_continuity_without_destroying_census_proof() {
    let mut processes = Processes::default();
    processes.event(fact(1, 42, 900, ProcessKind::Started));
    processes.observe(&census(2, 3, &[(42, 100)]));
    processes.link(at(4));
    let original = processes.sequence(42, 100).unwrap();
    processes.invalidate_trace(at(5));
    processes.event(fact(1, 42, 900, ProcessKind::Started));
    processes.link(at(7));
    assert!(processes.resolve(42, at(6)).is_none());
    processes.observe(&census(8, 9, &[(42, 100)]));
    assert_eq!(processes.resolve(42, at(6)), Some(original));
}

#[test]
fn an_exit_delivered_before_its_start_still_bounds_the_lifetime() {
    for ended in [1, 3] {
        let mut processes = Processes::default();
        processes.event(fact(ended, 42, 900, ProcessKind::Ended));
        processes.event(fact(1, 42, 900, ProcessKind::Started));
        processes.link(at(8));
        assert!(processes.resolve(42, at(6)).is_none());
        assert_eq!(processes.resolve(42, at(2)).is_some(), ended == 3);
        processes.event(fact(5, 42, 900, ProcessKind::Started));
        processes.link(at(8));
        assert!(processes.resolve(42, at(6)).is_some());
    }
}

#[test]
fn birth_during_census_binds_to_its_published_sequence() {
    let mut processes = Processes::default();
    processes.observe(&census(1, 3, &[(42, 100)]));
    let original = processes.sequence(42, 100).unwrap();
    processes.event(fact(2, 42, 900, ProcessKind::Started));
    processes.observe(&census(4, 5, &[(42, 100)]));
    processes.link(at(6));
    assert_eq!(
        processes.resolve(42, Duration::from_millis(2500)),
        Some(original)
    );
    assert_eq!(processes.sequence(42, 100), Some(original));
}
