use super::*;
use std::{
    net::{TcpListener, TcpStream},
    time::Instant,
};

fn ipv4(state: u32) -> Vec<u8> {
    [
        1,
        state,
        u32::from_ne_bytes([192, 0, 2, 1]),
        u32::from(50000_u16.to_be()),
        u32::from_ne_bytes([198, 51, 100, 2]),
        u32::from(443_u16.to_be()),
        42,
    ]
    .into_iter()
    .flat_map(u32::to_ne_bytes)
    .collect()
}

#[test]
fn ipv4_owner_and_network_order_endpoints_are_preserved() {
    let rows = decode(AF_INET.0 as u32, &ipv4(5)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pid, 42);
    assert_eq!(
        rows[0].local,
        Endpoint::new("192.0.2.1:50000".parse().unwrap())
    );
    assert_eq!(
        rows[0].remote,
        Endpoint::new("198.51.100.2:443".parse().unwrap())
    );
    assert_eq!(rows[0].state, TcpState::Established);
}

#[test]
fn ipv6_scopes_and_ports_are_decoded_from_network_order() {
    let mut bytes = 1_u32.to_ne_bytes().to_vec();
    bytes.extend("fe80::1".parse::<Ipv6Addr>().unwrap().octets());
    bytes.extend(3_u32.to_be_bytes());
    bytes.extend(u32::from(50000_u16.to_be()).to_ne_bytes());
    bytes.extend("fe80::2".parse::<Ipv6Addr>().unwrap().octets());
    bytes.extend(4_u32.to_be_bytes());
    bytes.extend(u32::from(443_u16.to_be()).to_ne_bytes());
    bytes.extend(5_u32.to_ne_bytes());
    bytes.extend(42_u32.to_ne_bytes());
    let rows = decode(AF_INET6.0 as u32, &bytes).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pid, 42);
    assert_eq!(
        rows[0].local,
        Endpoint::new("[fe80::1%3]:50000".parse().unwrap())
    );
    assert_eq!(
        rows[0].remote,
        Endpoint::new("[fe80::2%4]:443".parse().unwrap())
    );
}

#[test]
fn listeners_and_terminal_rows_do_not_create_remote_connections() {
    for state in [1, 2, 12] {
        assert!(decode(AF_INET.0 as u32, &ipv4(state)).unwrap().is_empty());
    }
    assert_eq!(tcp_state(3).unwrap(), Some(TcpState::SynSent));
    assert_eq!(tcp_state(11).unwrap(), Some(TcpState::TimeWait));
    assert!(tcp_state(13).is_err());
}

#[test]
fn truncated_or_impossible_native_counts_are_rejected() {
    let bytes = ipv4(5);
    for length in [0, 3, 4, bytes.len() - 1] {
        assert!(decode(AF_INET.0 as u32, &bytes[..length]).is_err());
    }
    let mut oversized = bytes;
    oversized[..4].copy_from_slice(&u32::MAX.to_ne_bytes());
    assert!(decode(AF_INET.0 as u32, &oversized).is_err());
    assert!(decode(0, &0_u32.to_ne_bytes()).is_err());
    assert!(
        decode(AF_INET6.0 as u32, &0_u32.to_ne_bytes())
            .unwrap()
            .is_empty()
    );
}

#[test]
#[ignore = "Reads Windows TCP tables with owned IPv4/IPv6 loopback sockets"]
fn native_capture_finds_both_idle_endpoints_and_excludes_their_listeners() {
    let mut sockets = Vec::new();
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let listener = TcpListener::bind(address).unwrap();
        let client =
            TcpStream::connect_timeout(&listener.local_addr().unwrap(), Duration::from_secs(2))
                .unwrap();
        let (server, _) = listener.accept().unwrap();
        sockets.push((listener, client, server));
    }
    let epoch = Instant::now();
    let capture = read(|| epoch.elapsed());
    assert!(capture.failures.is_empty(), "Native table capture failed");
    assert!(capture.window.end >= capture.window.begin);
    for (listener, client, server) in &sockets {
        for socket in [client, server] {
            let local = Endpoint::new(socket.local_addr().unwrap());
            let remote = Endpoint::new(socket.peer_addr().unwrap());
            assert_eq!(
                capture
                    .rows
                    .iter()
                    .filter(|row| row.pid == std::process::id()
                        && row.local == local
                        && row.remote == remote
                        && row.state == TcpState::Established)
                    .count(),
                1
            );
        }
        let local = Endpoint::new(listener.local_addr().unwrap());
        // The accepted server shares the listener's local endpoint. It is the
        // one connection that should remain after the listener is excluded.
        assert_eq!(
            capture
                .rows
                .iter()
                .filter(|row| row.pid == std::process::id() && row.local == local)
                .count(),
            1
        );
    }
}
