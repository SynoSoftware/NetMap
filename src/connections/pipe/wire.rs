use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::Arc,
    time::Duration,
};
use windows::core::GUID;

use super::super::{
    Bytes, Counts, Coverage, Endpoint, Life, Observation, ObservationId, ProcessId, Publication,
    Rates, Snapshot, TcpState, Tuple, Window,
    processes::{MAX_NAME_BYTES, valid_name},
};

pub(super) const MAX_FRAME: usize = 16 * 1024 * 1024;
const MAX_OBSERVATIONS: usize = 100_000;
const MAX_LIMITATION: usize = 16 * 1024;
const MAGIC: &[u8; 8] = b"NMAPPIPE";
pub(super) const VERSION: u32 = 2;
const TCP_STATES: [TcpState; 11] = [
    TcpState::Closed,
    TcpState::SynSent,
    TcpState::SynReceived,
    TcpState::Established,
    TcpState::FinWait1,
    TcpState::FinWait2,
    TcpState::CloseWait,
    TcpState::Closing,
    TcpState::LastAck,
    TcpState::TimeWait,
    TcpState::DeleteTcb,
];

pub(super) fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid collector snapshot frame",
    )
}

pub(super) struct Decoded {
    pub epoch: GUID,
    pub sequence: u64,
    pub sampled_qpc: i64,
    pub frequency: i64,
    pub snapshot: Snapshot,
    pub limitation: Option<Arc<str>>,
    pub names: HashMap<ProcessId, Box<str>>,
}

struct Writer(Vec<u8>);

impl Writer {
    fn byte(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u32(&mut self, value: u32) {
        self.0.extend(value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend(value.to_le_bytes());
    }
    fn duration(&mut self, value: Duration) -> io::Result<()> {
        self.u64(u64::try_from(value.as_nanos()).map_err(|_| invalid())?);
        Ok(())
    }
    fn bytes(&mut self, value: Bytes) {
        self.u64(value.received);
        self.u64(value.sent);
    }
    fn rates(&mut self, value: Option<Rates>) {
        self.byte(u8::from(value.is_some()));
        if let Some(value) = value {
            self.u64(value.received.to_bits());
            self.u64(value.sent.to_bits());
        }
    }
    fn endpoint(&mut self, value: Endpoint) {
        match value.address() {
            IpAddr::V4(address) => {
                self.byte(4);
                self.0.extend(address.octets());
            }
            IpAddr::V6(address) => {
                self.byte(6);
                self.0.extend(address.octets());
            }
        }
        self.0.extend(value.port().to_le_bytes());
        self.u32(value.scope());
    }
}

// An explicit wire format, not a Rust struct dump: no pointers, usize, enum
// discriminants or ABI padding cross the process boundary.
pub(super) fn encode(
    publication: &Publication,
    epoch: GUID,
    sequence: u64,
    sampled_qpc: i64,
    frequency: i64,
) -> io::Result<Arc<[u8]>> {
    let snapshot = &publication.snapshot;
    let limitation = publication.limitation.as_deref().unwrap_or("");
    if snapshot.observations.len() > MAX_OBSERVATIONS || limitation.len() > MAX_LIMITATION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Collector snapshot exceeds transport bound",
        ));
    }
    let mut out = Writer(Vec::with_capacity(
        256 + snapshot.observations.len() * 128 + limitation.len(),
    ));
    out.u32(0); // Length excludes its own prefix.
    out.0.extend(MAGIC);
    out.u32(VERSION);
    out.0.extend(epoch.to_u128().to_le_bytes());
    out.u64(sequence);
    out.0.extend(sampled_qpc.to_le_bytes());
    out.0.extend(frequency.to_le_bytes());
    out.duration(snapshot.interval.begin)?;
    out.duration(snapshot.interval.end)?;
    match snapshot.coverage {
        Coverage::WarmingUp => out.byte(0),
        Coverage::Complete => out.byte(1),
        Coverage::Restricted => out.byte(2),
        Coverage::Lost { events, buffers } => {
            out.byte(3);
            out.u64(events);
            out.u64(buffers);
        }
    }
    out.bytes(snapshot.bytes);
    out.bytes(snapshot.unattributed);
    out.bytes(snapshot.total_bytes);
    out.rates(snapshot.rates);
    out.u64(snapshot.ambiguous_events);
    for count in [
        snapshot.counts.tcp,
        snapshot.counts.ended_tcp,
        snapshot.counts.udp,
        snapshot.counts.inactive_udp,
        snapshot.counts.transferring,
    ] {
        out.u32(u32::try_from(count).map_err(|_| invalid())?);
    }
    out.u32(limitation.len() as u32);
    out.0.extend(limitation.as_bytes());
    out.u32(snapshot.observations.len() as u32);
    for row in snapshot.observations.iter() {
        out.u64(row.id.0);
        out.u32(row.tuple.process.pid);
        out.u64(row.tuple.process.generation);
        out.endpoint(row.tuple.local);
        out.endpoint(row.tuple.remote);
        match row.life {
            Life::Tcp(state) => {
                out.byte(0);
                out.byte(TCP_STATES.iter().position(|value| *value == state).unwrap() as u8);
            }
            Life::EndedTcp => out.byte(1),
            Life::Udp { last_seen } => {
                out.byte(2);
                out.duration(last_seen)?;
            }
            Life::ExpiredUdp { last_seen } => {
                out.byte(3);
                out.duration(last_seen)?;
            }
        }
        out.duration(row.introduced)?;
        out.bytes(row.bytes);
        out.rates(row.rates);
    }
    let processes: HashSet<_> = snapshot
        .observations
        .iter()
        .map(|row| row.tuple.process)
        .collect();
    if publication.names.len() > processes.len() {
        return Err(invalid());
    }
    out.u32(publication.names.len() as u32);
    for (process, name) in publication.names.iter() {
        if !processes.contains(process)
            || !valid_name(name)
            || out
                .0
                .len()
                .checked_add(16 + name.len())
                .is_none_or(|length| length > MAX_FRAME + 4)
        {
            return Err(invalid());
        }
        out.u32(process.pid);
        out.u64(process.generation);
        out.u32(name.len() as u32);
        out.0.extend(name.as_bytes());
    }
    if out.0.len() - 4 > MAX_FRAME {
        return Err(invalid());
    }
    let length = (out.0.len() - 4) as u32;
    out.0[..4].copy_from_slice(&length.to_le_bytes());
    Ok(out.0.into())
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, length: usize) -> io::Result<&[u8]> {
        let (head, tail) = self.0.split_at_checked(length).ok_or_else(invalid)?;
        self.0 = tail;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| invalid())
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn duration(&mut self) -> io::Result<Duration> {
        Ok(Duration::from_nanos(self.u64()?))
    }
    fn bytes(&mut self) -> io::Result<Bytes> {
        Ok(Bytes {
            received: self.u64()?,
            sent: self.u64()?,
        })
    }
    fn rates(&mut self) -> io::Result<Option<Rates>> {
        match self.byte()? {
            0 => Ok(None),
            1 => {
                let value = Rates {
                    received: f64::from_bits(self.u64()?),
                    sent: f64::from_bits(self.u64()?),
                };
                if [value.received, value.sent]
                    .iter()
                    .all(|rate| rate.is_finite() && *rate >= 0.0)
                {
                    Ok(Some(value))
                } else {
                    Err(invalid())
                }
            }
            _ => Err(invalid()),
        }
    }
    fn endpoint(&mut self) -> io::Result<Endpoint> {
        let address = match self.byte()? {
            4 => IpAddr::V4(Ipv4Addr::from(self.array::<4>()?)),
            6 => IpAddr::V6(Ipv6Addr::from(self.array::<16>()?)),
            _ => return Err(invalid()),
        };
        let port = u16::from_le_bytes(self.array()?);
        let scope = self.u32()?;
        Ok(Endpoint::new(match address {
            IpAddr::V4(address) if scope == 0 => SocketAddr::from((address, port)),
            IpAddr::V6(address) => SocketAddr::V6(SocketAddrV6::new(address, port, 0, scope)),
            _ => return Err(invalid()),
        }))
    }
}

pub(super) fn decode(bytes: &[u8]) -> io::Result<Decoded> {
    if bytes.len() > MAX_FRAME {
        return Err(invalid());
    }
    let mut input = Reader(bytes);
    if input.take(MAGIC.len())? != MAGIC || input.u32()? != VERSION {
        return Err(invalid());
    }
    let epoch = GUID::from_u128(u128::from_le_bytes(input.array()?));
    let sequence = input.u64()?;
    let sampled_qpc = i64::from_le_bytes(input.array()?);
    let frequency = i64::from_le_bytes(input.array()?);
    if epoch == GUID::zeroed() || sequence == 0 || sampled_qpc < 0 || frequency <= 0 {
        return Err(invalid());
    }
    let interval = Window {
        begin: input.duration()?,
        end: input.duration()?,
    };
    if interval.begin > interval.end {
        return Err(invalid());
    }
    let coverage = match input.byte()? {
        0 => Coverage::WarmingUp,
        1 => Coverage::Complete,
        2 => Coverage::Restricted,
        3 => Coverage::Lost {
            events: input.u64()?,
            buffers: input.u64()?,
        },
        _ => return Err(invalid()),
    };
    let bytes = input.bytes()?;
    let unattributed = input.bytes()?;
    let total_bytes = input.bytes()?;
    let rates = input.rates()?;
    let ambiguous_events = input.u64()?;
    let counts = Counts {
        tcp: input.u32()? as usize,
        ended_tcp: input.u32()? as usize,
        udp: input.u32()? as usize,
        inactive_udp: input.u32()? as usize,
        transferring: input.u32()? as usize,
    };
    let length = input.u32()? as usize;
    if length > MAX_LIMITATION {
        return Err(invalid());
    }
    let limitation = std::str::from_utf8(input.take(length)?).map_err(|_| invalid())?;
    let limitation = (!limitation.is_empty()).then(|| Arc::<str>::from(limitation));
    let length = input.u32()? as usize;
    if length > MAX_OBSERVATIONS
        || counts
            .tcp
            .checked_add(counts.ended_tcp)
            .and_then(|sum| sum.checked_add(counts.udp))
            .and_then(|sum| sum.checked_add(counts.inactive_udp))
            != Some(length)
        || counts.transferring > length
    {
        return Err(invalid());
    }
    let mut rows = Vec::with_capacity(length);
    let mut identities = HashSet::with_capacity(length);
    let mut processes = HashSet::new();
    for _ in 0..length {
        let id = ObservationId(input.u64()?);
        if id.0 == 0 || !identities.insert(id) {
            return Err(invalid());
        }
        let process = ProcessId {
            pid: input.u32()?,
            generation: input.u64()?,
        };
        if process.generation == 0 {
            return Err(invalid());
        }
        processes.insert(process);
        let tuple = Tuple {
            process,
            local: input.endpoint()?,
            remote: input.endpoint()?,
        };
        let life = match input.byte()? {
            0 => Life::Tcp(*TCP_STATES.get(input.byte()? as usize).ok_or_else(invalid)?),
            1 => Life::EndedTcp,
            2 => Life::Udp {
                last_seen: input.duration()?,
            },
            3 => Life::ExpiredUdp {
                last_seen: input.duration()?,
            },
            _ => return Err(invalid()),
        };
        let introduced = input.duration()?;
        if introduced > interval.end
            || matches!(life, Life::Udp { last_seen } | Life::ExpiredUdp { last_seen } if last_seen > interval.end)
        {
            return Err(invalid());
        }
        rows.push(Observation {
            id,
            tuple,
            life,
            introduced,
            bytes: input.bytes()?,
            rates: input.rates()?,
        });
    }
    let length = input.u32()? as usize;
    if length > processes.len() {
        return Err(invalid());
    }
    let mut names = HashMap::with_capacity(length);
    for _ in 0..length {
        let process = ProcessId {
            pid: input.u32()?,
            generation: input.u64()?,
        };
        let length = input.u32()? as usize;
        if length > MAX_NAME_BYTES || !processes.contains(&process) {
            return Err(invalid());
        }
        let name = std::str::from_utf8(input.take(length)?).map_err(|_| invalid())?;
        if !valid_name(name) || names.insert(process, Box::<str>::from(name)).is_some() {
            return Err(invalid());
        }
    }
    if !input.0.is_empty() {
        return Err(invalid());
    }
    Ok(Decoded {
        epoch,
        sequence,
        sampled_qpc,
        frequency,
        snapshot: Snapshot {
            counts,
            observations: rows.into(),
            interval,
            coverage,
            bytes,
            unattributed,
            total_bytes,
            rates,
            ambiguous_events,
        },
        limitation,
        names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn transport_preserves_identity_source_ages_scope_and_missing_rates() {
        let row = Observation {
            id: ObservationId(7),
            tuple: Tuple {
                process: ProcessId {
                    pid: 42,
                    generation: 9,
                },
                local: Endpoint::new("[fe80::1%12]:50000".parse().unwrap()),
                remote: Endpoint::new("[fe80::2%12]:443".parse().unwrap()),
            },
            life: Life::ExpiredUdp {
                last_seen: Duration::from_secs(4),
            },
            introduced: Duration::from_secs(3),
            bytes: Bytes {
                received: 12,
                sent: 17,
            },
            rates: None,
        };
        let publication = Publication {
            snapshot: Arc::new(Snapshot {
                counts: Counts {
                    inactive_udp: 1,
                    ..Default::default()
                },
                observations: Arc::from([row.clone()]),
                interval: Window {
                    begin: Duration::from_secs(4),
                    end: Duration::from_secs(6),
                },
                coverage: Coverage::Lost {
                    events: 3,
                    buffers: 2,
                },
                bytes: row.bytes,
                unattributed: Bytes::default(),
                total_bytes: row.bytes,
                rates: None,
                ambiguous_events: 2,
            }),
            sampled: Instant::now(),
            limitation: Some("source uncertainty".into()),
            names: Arc::new(HashMap::from([(
                row.tuple.process,
                Box::<str>::from("worker-Δ.exe"),
            )])),
        };
        let epoch = GUID::from_u128(1234);
        let frame = encode(&publication, epoch, 8, 100, 10).unwrap();
        let decoded = decode(&frame[4..]).unwrap();
        assert_eq!(
            (
                decoded.epoch,
                decoded.sequence,
                decoded.sampled_qpc,
                decoded.frequency
            ),
            (epoch, 8, 100, 10)
        );
        assert_eq!(decoded.snapshot.observations.as_ref(), &[row]);
        assert_eq!(decoded.snapshot.coverage, publication.snapshot.coverage);
        assert_eq!(decoded.snapshot.interval, publication.snapshot.interval);
        assert_eq!(decoded.snapshot.bytes, publication.snapshot.bytes);
        assert_eq!(decoded.limitation, publication.limitation);
        assert_eq!(decoded.names, *publication.names);
        assert!(decoded.snapshot.rates.is_none());
        for length in 0..frame.len() - 4 {
            assert!(decode(&frame[4..4 + length]).is_err());
        }
        let mut unknown_version = frame.to_vec();
        unknown_version[12] = (VERSION + 1) as u8;
        assert!(decode(&unknown_version[4..]).is_err());
        let mut duplicate = publication.clone();
        Arc::make_mut(&mut duplicate.snapshot).observations = Arc::from([
            publication.snapshot.observations[0].clone(),
            publication.snapshot.observations[0].clone(),
        ]);
        Arc::make_mut(&mut duplicate.snapshot).counts.inactive_udp = 2;
        let frame = encode(&duplicate, epoch, 8, 100, 10).unwrap();
        assert!(decode(&frame[4..]).is_err());
        let mut invalid_rates = publication.clone();
        Arc::make_mut(&mut invalid_rates.snapshot).rates = Some(Rates {
            received: f64::NAN,
            sent: 1.0,
        });
        let frame = encode(&invalid_rates, epoch, 8, 100, 10).unwrap();
        assert!(decode(&frame[4..]).is_err());
        let mut malformed_name = encode(&publication, epoch, 8, 100, 10).unwrap().to_vec();
        *malformed_name.last_mut().unwrap() = b'/';
        assert!(decode(&malformed_name[4..]).is_err());
        let mut foreign_name = publication.clone();
        foreign_name.names = Arc::new(HashMap::from([(
            ProcessId {
                pid: 42,
                generation: 999,
            },
            Box::<str>::from("successor.exe"),
        )]));
        assert!(encode(&foreign_name, epoch, 8, 100, 10).is_err());
    }

    #[test]
    fn wire_lifetimes_keep_tcp_state_and_available_zero_distinct_from_missing_rates() {
        let lives = TCP_STATES.into_iter().map(Life::Tcp).chain([
            Life::EndedTcp,
            Life::Udp {
                last_seen: Duration::ZERO,
            },
            Life::ExpiredUdp {
                last_seen: Duration::ZERO,
            },
        ]);
        let rows: Vec<_> = lives
            .enumerate()
            .map(|(index, life)| Observation {
                id: ObservationId(index as u64 + 1),
                tuple: Tuple {
                    process: ProcessId {
                        pid: 42,
                        generation: 1,
                    },
                    local: Endpoint::new("127.0.0.1:50000".parse().unwrap()),
                    remote: Endpoint::new("192.168.1.2:443".parse().unwrap()),
                },
                life,
                introduced: Duration::ZERO,
                bytes: Bytes::default(),
                rates: Some(Rates {
                    received: 0.0,
                    sent: 0.0,
                }),
            })
            .collect();
        let publication = Publication {
            snapshot: Arc::new(Snapshot {
                counts: Counts {
                    tcp: TCP_STATES.len(),
                    ended_tcp: 1,
                    udp: 1,
                    inactive_udp: 1,
                    transferring: 0,
                },
                observations: rows.clone().into(),
                interval: Window {
                    begin: Duration::ZERO,
                    end: Duration::from_secs(2),
                },
                coverage: Coverage::Complete,
                bytes: Bytes::default(),
                unattributed: Bytes::default(),
                total_bytes: Bytes::default(),
                rates: Some(Rates {
                    received: 0.0,
                    sent: 0.0,
                }),
                ambiguous_events: 0,
            }),
            sampled: Instant::now(),
            limitation: None,
            names: Arc::new(HashMap::new()),
        };
        let frame = encode(&publication, GUID::from_u128(1), 1, 0, 1).unwrap();
        let decoded = decode(&frame[4..]).unwrap();
        assert_eq!(decoded.snapshot.observations.as_ref(), rows.as_slice());
        assert_eq!(decoded.snapshot.rates, publication.snapshot.rates);
        assert_eq!(
            decoded.snapshot.total_bytes,
            publication.snapshot.total_bytes
        );
        assert!(decoded.limitation.is_none());
    }

    #[test]
    fn oversized_frames_and_declared_observation_counts_are_rejected() {
        assert!(decode(&vec![0; MAX_FRAME + 1]).is_err());
        let publication = Publication {
            snapshot: Arc::new(Snapshot {
                counts: Counts::default(),
                observations: Arc::from([]),
                interval: Window {
                    begin: Duration::ZERO,
                    end: Duration::ZERO,
                },
                coverage: Coverage::WarmingUp,
                bytes: Bytes::default(),
                unattributed: Bytes::default(),
                total_bytes: Bytes::default(),
                rates: None,
                ambiguous_events: 0,
            }),
            sampled: Instant::now(),
            limitation: None,
            names: Arc::new(HashMap::new()),
        };
        let mut frame = encode(&publication, GUID::from_u128(1), 1, 0, 1)
            .unwrap()
            .to_vec();
        // An empty frame ends with observation and name counts. Reject the claimed
        // count before allocating rows, even though the frame itself is tiny.
        let offset = frame.len() - 8;
        frame[offset..offset + 4].copy_from_slice(&(MAX_OBSERVATIONS as u32 + 1).to_le_bytes());
        assert!(decode(&frame[4..]).is_err());
    }
}
