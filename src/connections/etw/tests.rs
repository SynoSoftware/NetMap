use super::*;

const CLOCK: Clock = Clock {
    epoch: 100,
    frequency: 10_000_000,
};

#[test]
fn process_prefix_uses_payload_pid_and_header_pointer_width_without_inventing_birth_from_rundown() {
    for (width, flags) in [
        (4, EVENT_HEADER_FLAG_32_BIT_HEADER),
        (8, EVENT_HEADER_FLAG_64_BIT_HEADER),
    ] {
        let mut bytes = 123_u64.to_le_bytes()[..width].to_vec();
        bytes.extend(42_u32.to_le_bytes());
        let existing = decode_process(4, 3, flags as u16, 110, &bytes, CLOCK)
            .unwrap()
            .unwrap();
        assert_eq!(
            (existing.pid, existing.key, existing.kind),
            (42, Some(123), ProcessKind::Existing)
        );
        for length in 0..bytes.len() {
            assert!(decode_process(4, 1, flags as u16, 110, &bytes[..length], CLOCK).is_err());
        }
        assert!(decode_process(1, 1, flags as u16, 110, &bytes, CLOCK).is_err());
        assert!(decode_process(6, 1, flags as u16, 110, &bytes, CLOCK).is_err());
        for version in 2..=5 {
            assert!(decode_process(version, 1, flags as u16, 110, &bytes, CLOCK).is_ok());
        }
        assert!(decode_process(4, 1, 0, 110, &bytes, CLOCK).is_err());
        assert_eq!(
            decode_process(4, 4, flags as u16, 110, &[], CLOCK),
            Ok(None)
        );
    }
}

fn payload(ipv6: bool) -> Vec<u8> {
    let mut bytes = 42_u32.to_le_bytes().to_vec();
    bytes.extend(777_u32.to_le_bytes());
    if ipv6 {
        bytes.extend("2001:db8::2".parse::<Ipv6Addr>().unwrap().octets());
        bytes.extend("2001:db8::1".parse::<Ipv6Addr>().unwrap().octets());
    } else {
        bytes.extend([198, 51, 100, 2, 192, 0, 2, 1]);
    }
    bytes.extend(443_u16.to_be_bytes());
    bytes.extend(50000_u16.to_be_bytes());
    bytes
}

#[test]
fn tcp_receive_keeps_source_local_while_udp_receive_uses_destination_local() {
    for ipv6 in [false, true] {
        let bytes = payload(ipv6);
        let receive = if ipv6 { 27 } else { 11 };
        let send = receive - 1;
        let tcp = decode(TCP, 2, receive, 110, &bytes, CLOCK)
            .unwrap()
            .unwrap();
        let udp = decode(UDP, 2, receive, 110, &bytes, CLOCK)
            .unwrap()
            .unwrap();
        assert_eq!(tcp.pid, 42);
        assert_eq!(tcp.local.port(), 50000);
        assert_eq!(tcp.remote.port(), 443);
        assert_eq!(
            tcp.kind,
            Kind::Transfer(Bytes {
                received: 777,
                sent: 0
            })
        );
        assert_eq!(udp.local, tcp.remote);
        assert_eq!(udp.remote, tcp.local);
        let sent = decode(TCP, 2, send, 110, &bytes, CLOCK).unwrap().unwrap();
        assert_eq!(sent.local, tcp.local);
        assert_eq!(sent.remote, tcp.remote);
        assert_eq!(
            sent.kind,
            Kind::Transfer(Bytes {
                received: 0,
                sent: 777
            })
        );
        assert_eq!(tcp.at, Duration::from_micros(1));
        let sent = decode(UDP, 2, send, 110, &bytes, CLOCK).unwrap().unwrap();
        assert_eq!(sent.local, tcp.local);
        assert_eq!(
            sent.kind,
            Kind::Transfer(Bytes {
                received: 0,
                sent: 777
            })
        );
    }
}

#[test]
fn lifecycle_records_do_not_count_payload_sizes_as_transfers_or_read_the_pointer_suffix() {
    for ipv6 in [false, true] {
        let mut bytes = payload(ipv6);
        let base = if ipv6 { 16 } else { 0 };
        for (opcode, expected) in [(12, Kind::Opened), (15, Kind::Opened), (13, Kind::Closed)] {
            let fact = decode(TCP, 2, opcode + base, 110, &bytes, CLOCK)
                .unwrap()
                .unwrap();
            assert_eq!(fact.kind, expected);
            assert_eq!(fact.local.port(), 50000);
            bytes.extend([0xff; 4]);
            assert_eq!(
                decode(TCP, 2, opcode + base, 110, &bytes, CLOCK),
                Ok(Some(fact))
            );
        }
        assert_eq!(decode(TCP, 2, 14 + base, 110, &[], CLOCK), Ok(None));
        assert_eq!(decode(TCP, 2, 16 + base, 110, &[], CLOCK), Ok(None));
    }
}

#[test]
fn unsupported_relevant_schemas_and_short_payloads_are_errors_but_unrelated_events_are_ignored() {
    for ipv6 in [false, true] {
        let bytes = payload(ipv6);
        let opcode = if ipv6 { 26 } else { 10 };
        for length in 0..bytes.len() {
            assert!(decode(TCP, 2, opcode, 110, &bytes[..length], CLOCK).is_err());
        }
        assert!(decode(TCP, 1, opcode, 110, &bytes, CLOCK).is_err());
        assert!(decode(UDP, 3, opcode, 110, &bytes, CLOCK).is_err());
        assert!(decode(TCP, 2, opcode, 99, &bytes, CLOCK).is_err());
    }
    assert_eq!(decode(GUID::zeroed(), 0, 0, -1, &[], CLOCK), Ok(None));
    assert_eq!(decode(TCP, 0, 14, -1, &[], CLOCK), Ok(None));
}

#[test]
fn qpc_conversion_uses_integer_elapsed_ticks_and_rejects_pre_epoch_events() {
    assert_eq!(CLOCK.at(100), Some(Duration::ZERO));
    assert_eq!(CLOCK.at(101), Some(Duration::from_nanos(100)));
    assert_eq!(CLOCK.at(10_000_101), Some(Duration::new(1, 100)));
    assert_eq!(CLOCK.at(99), None);
    let extreme = Clock {
        epoch: i64::MIN,
        frequency: 1,
    };
    assert_eq!(extreme.at(i64::MAX), Some(Duration::from_secs(u64::MAX)));
    let clock = Clock::new().unwrap();
    let first = clock.now().unwrap();
    assert!(clock.now().unwrap() >= first);
}

#[test]
fn callback_copies_payload_pid_and_reports_queue_overflow_and_malformed_events() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let control = Arc::new(Control {
        handle: AtomicU64::new(INVALID_TRACE),
        dropped: AtomicU64::new(0),
        malformed: AtomicU64::new(0),
    });
    let mut context = Context {
        clock: CLOCK,
        sender,
        control: Arc::clone(&control),
    };
    let mut bytes = payload(false);
    let mut record = EVENT_RECORD {
        UserData: bytes.as_mut_ptr().cast(),
        UserDataLength: bytes.len() as u16,
        UserContext: (&mut context as *mut Context).cast(),
        ..Default::default()
    };
    record.EventHeader.ProviderId = TCP;
    record.EventHeader.EventDescriptor.Version = 2;
    record.EventHeader.EventDescriptor.Opcode = 10;
    record.EventHeader.TimeStamp = 110;
    record.EventHeader.ProcessId = u32::MAX;
    // SAFETY: Record, payload and Context remain live throughout these direct
    // callbacks; their native layout matches the callback's ETW obligations.
    unsafe {
        on_record(&mut record);
        on_record(&mut record);
    }
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Record::Network(Fact { pid: 42, .. })
    ));
    assert_eq!(control.losses(), (1, 0));
    record.EventHeader.EventDescriptor.Version = 1;
    // SAFETY: All callback inputs still refer to live allocations above.
    unsafe {
        on_record(&mut record);
    }
    assert_eq!(control.losses(), (1, 1));
    record.EventHeader.EventDescriptor.Opcode = 14;
    record.UserData = std::ptr::null_mut();
    record.UserDataLength = 0;
    // SAFETY: This ignored event has no payload, while its context remains live.
    unsafe {
        on_record(&mut record);
    }
    assert_eq!(control.losses(), (1, 1));
}
