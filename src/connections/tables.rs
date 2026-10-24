use std::{
    io,
    mem::{offset_of, size_of},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    time::Duration,
};
use windows::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
    NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID,
        MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    },
    Networking::WinSock::{AF_INET, AF_INET6},
};

use super::{Endpoint, TcpState, Window};

// Native buffers are temporary. A changing or oversized table is a partial
// capture, never evidence that sockets in the unread family have disappeared.
const MAX_TABLE_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct Row {
    pub pid: u32,
    pub local: Endpoint,
    pub remote: Endpoint,
    pub state: TcpState,
}

pub(super) struct Capture {
    pub window: Window,
    pub rows: Vec<Row>,
    pub failures: Vec<(u32, io::Error)>,
}

pub(super) fn read(mut now: impl FnMut() -> Duration) -> Capture {
    let begin = now();
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for family in [AF_INET.0 as u32, AF_INET6.0 as u32] {
        match read_family(family) {
            Ok(mut family_rows) => rows.append(&mut family_rows),
            Err(error) => failures.push((family, error)),
        }
    }
    Capture {
        window: Window { begin, end: now() },
        rows,
        failures,
    }
}

fn read_family(family: u32) -> io::Result<Vec<Row>> {
    let mut buffer = vec![0_u64; 2048];
    for _ in 0..3 {
        let capacity = buffer.len() * size_of::<u64>();
        let mut size = capacity as u32;
        // SAFETY: This initialized, aligned allocation remains exclusively
        // borrowed until the native call returns; size is its byte capacity.
        let status = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr().cast()),
                &mut size,
                false,
                family,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status == ERROR_SUCCESS.0 {
            let length = size as usize;
            if length > capacity {
                return Err(invalid_table());
            }
            // SAFETY: Every byte of the u64 allocation is initialized. The
            // native reported length has been checked against that allocation.
            let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast(), length) };
            return decode(family, bytes);
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let required = size as usize;
        if required <= capacity || required > MAX_TABLE_BYTES {
            return Err(io::Error::other("TCP table exceeds its bounded capture"));
        }
        buffer.resize(required.div_ceil(size_of::<u64>()), 0);
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "TCP table kept changing during capture",
    ))
}

fn decode(family: u32, bytes: &[u8]) -> io::Result<Vec<Row>> {
    let count = u32::from_ne_bytes(
        bytes
            .get(..4)
            .ok_or_else(invalid_table)?
            .try_into()
            .unwrap(),
    );
    let (offset, stride) = if family == AF_INET.0 as u32 {
        (
            offset_of!(MIB_TCPTABLE_OWNER_PID, table),
            size_of::<MIB_TCPROW_OWNER_PID>(),
        )
    } else if family == AF_INET6.0 as u32 {
        (
            offset_of!(MIB_TCP6TABLE_OWNER_PID, table),
            size_of::<MIB_TCP6ROW_OWNER_PID>(),
        )
    } else {
        return Err(invalid_table());
    };
    if bytes.len() < offset || count as usize > (bytes.len() - offset) / stride {
        return Err(invalid_table());
    }
    let mut rows = Vec::new();
    for bytes in bytes[offset..].chunks_exact(stride).take(count as usize) {
        let (pid, local, remote, state) = if family == AF_INET.0 as u32 {
            // SAFETY: A full row was bounds-checked above. All fields are u32;
            // unaligned reads accept the byte slice's arbitrary alignment.
            let row = unsafe {
                bytes
                    .as_ptr()
                    .cast::<MIB_TCPROW_OWNER_PID>()
                    .read_unaligned()
            };
            (
                row.dwOwningPid,
                SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes()),
                    port(row.dwLocalPort),
                )),
                SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(row.dwRemoteAddr.to_ne_bytes()),
                    port(row.dwRemotePort),
                )),
                row.dwState,
            )
        } else {
            // SAFETY: A full row was bounds-checked above. This native row
            // contains only integer fields, which admit every bit pattern.
            let row = unsafe {
                bytes
                    .as_ptr()
                    .cast::<MIB_TCP6ROW_OWNER_PID>()
                    .read_unaligned()
            };
            (
                row.dwOwningPid,
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(row.ucLocalAddr),
                    port(row.dwLocalPort),
                    0,
                    u32::from_be(row.dwLocalScopeId),
                )),
                SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(row.ucRemoteAddr),
                    port(row.dwRemotePort),
                    0,
                    u32::from_be(row.dwRemoteScopeId),
                )),
                row.dwState,
            )
        };
        if let Some(state) = tcp_state(state)? {
            rows.push(Row {
                pid,
                local: Endpoint::new(local),
                remote: Endpoint::new(remote),
                state,
            });
        }
    }
    Ok(rows)
}

fn port(value: u32) -> u16 {
    u16::from_be(value as u16)
}

fn tcp_state(value: u32) -> io::Result<Option<TcpState>> {
    Ok(Some(match value {
        // A listener has no remote connection; terminal rows do not prove a
        // current lifetime. A complete capture reconciles their absence.
        1 | 2 | 12 => return Ok(None),
        3 => TcpState::SynSent,
        4 => TcpState::SynReceived,
        5 => TcpState::Established,
        6 => TcpState::FinWait1,
        7 => TcpState::FinWait2,
        8 => TcpState::CloseWait,
        9 => TcpState::Closing,
        10 => TcpState::LastAck,
        11 => TcpState::TimeWait,
        _ => return Err(invalid_table()),
    }))
}

fn invalid_table() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid native TCP table")
}

#[cfg(test)]
mod tests;
