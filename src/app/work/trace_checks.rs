use super::*;
use crate::connections::{
    Bytes, Collector, Coverage, Endpoint, Highlight, Life, Protocol,
    etw::{Clock, Fact, Kind, ProcessFact, ProcessKind, Record, Session},
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    os::windows::process::CommandExt,
    process::{Child, Command, Stdio},
};
use windows::Win32::{
    Foundation::FILETIME,
    System::{
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        Threading::{GetProcessHandleCount, GetProcessTimes},
    },
};

type Key = (Protocol, Endpoint, Endpoint);

#[derive(Clone, Copy)]
struct ProcessResources {
    private_bytes: usize,
    working_set_bytes: usize,
    cpu_100ns: u64,
    handles: u32,
}

impl ProcessResources {
    fn read() -> io::Result<Self> {
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let mut handles = 0;
        // SAFETY: The pseudo handle names this test process. EX extends the
        // counter prefix; cb describes its complete initialized writable storage.
        unsafe {
            let process = GetCurrentProcess();
            GetProcessMemoryInfo(
                process,
                (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
                memory.cb,
            )
            .map_err(|error| io::Error::other(error.to_string()))?;
            GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user)
                .map_err(|error| io::Error::other(error.to_string()))?;
            GetProcessHandleCount(process, &mut handles)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        let ticks =
            |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        Ok(Self {
            private_bytes: memory.PrivateUsage,
            working_set_bytes: memory.WorkingSetSize,
            cpu_100ns: ticks(kernel) + ticks(user),
            handles,
        })
    }

    fn report(self, phase: &str, baseline: Self, started: Instant) {
        eprintln!(
            "CollectorResource={{\"phase\":\"{phase}\",\"elapsed_ms\":{},\"private_bytes\":{},\"working_set_bytes\":{},\"cpu_100ns\":{},\"handles\":{},\"private_delta_bytes\":{},\"working_set_delta_bytes\":{},\"cpu_delta_100ns\":{},\"handles_delta\":{}}}",
            started.elapsed().as_millis(),
            self.private_bytes,
            self.working_set_bytes,
            self.cpu_100ns,
            self.handles,
            self.private_bytes as i128 - baseline.private_bytes as i128,
            self.working_set_bytes as i128 - baseline.working_set_bytes as i128,
            self.cpu_100ns - baseline.cpu_100ns,
            i64::from(self.handles) - i64::from(baseline.handles)
        );
    }
}

fn sample_resources(
    phase: &str,
    baseline: ProcessResources,
    peak: &mut ProcessResources,
    started: Instant,
) -> io::Result<()> {
    let sample = ProcessResources::read()?;
    peak.private_bytes = peak.private_bytes.max(sample.private_bytes);
    peak.working_set_bytes = peak.working_set_bytes.max(sample.working_set_bytes);
    peak.cpu_100ns = peak.cpu_100ns.max(sample.cpu_100ns);
    peak.handles = peak.handles.max(sample.handles);
    sample.report(phase, baseline, started);
    Ok(())
}

fn trace_resources(phase: &str, collector: &mut Collector, observations: usize) -> io::Result<()> {
    let statistics = collector.trace_statistics()?;
    eprintln!(
        "CollectorTraceResource={{\"phase\":\"{phase}\",\"retained_published_observations\":{observations},\"etw_buffers\":{},\"etw_buffer_kib\":{},\"etw_reported_buffer_bytes\":{}}}",
        statistics.buffers,
        statistics.buffer_kib,
        u64::from(statistics.buffers) * u64::from(statistics.buffer_kib) * 1024
    );
    Ok(())
}

fn external_enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

#[test]
fn external_network_work_requires_explicit_opt_in() {
    for value in [None, Some(""), Some("0"), Some("false"), Some("true")] {
        assert!(!external_enabled(value.map(std::ffi::OsStr::new)));
    }
    assert!(external_enabled(Some(std::ffi::OsStr::new("1"))));
}

fn external_workload() -> io::Result<Option<(Key, Bytes)>> {
    if !external_enabled(std::env::var_os("NETMAP_TRACE_EXTERNAL").as_deref()) {
        return Ok(None);
    }
    // A small, public HEAD request exercises the physical receive path without
    // TLS bookkeeping or DNS timing in the byte-conservation assertion.
    let mut socket =
        TcpStream::connect_timeout(&"1.1.1.1:80".parse().unwrap(), Duration::from_secs(2))?;
    let key = (
        Protocol::Tcp,
        Endpoint::new(socket.local_addr()?),
        Endpoint::new(socket.peer_addr()?),
    );
    let request = b"HEAD / HTTP/1.1\r\nHost: one.one.one.one\r\nConnection: close\r\n\r\n";
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    socket.write_all(request)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received = 0_u64;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "External TCP response timed out",
            ));
        }
        socket.set_read_timeout(Some(remaining))?;
        let mut buffer = [0; 1024];
        let length = socket.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        received += length as u64;
        if received > 16384 {
            return Err(io::Error::other(
                "External HEAD response exceeded its bound",
            ));
        }
    }
    if received == 0 {
        return Err(io::Error::other("External TCP response was empty"));
    }
    let bytes = Bytes {
        received,
        sent: request.len() as u64,
    };
    eprintln!("ExternalWorkload: bytes={bytes:?}");
    Ok(Some((key, bytes)))
}

fn workload(address: &str) -> io::Result<Vec<(Key, Bytes)>> {
    let listener = TcpListener::bind(address)?;
    let mut client = TcpStream::connect_timeout(&listener.local_addr()?, Duration::from_secs(2))?;
    let (mut server, _) = listener.accept()?;
    for socket in [&client, &server] {
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    }
    let tcp = (
        Protocol::Tcp,
        Endpoint::new(client.local_addr()?),
        Endpoint::new(client.peer_addr()?),
    );
    client.write_all(&[1; 4096])?;
    server.read_exact(&mut [0; 4096])?;
    server.write_all(&[2; 2048])?;
    client.read_exact(&mut [0; 2048])?;
    drop(client);
    drop(server);
    drop(listener);

    let server = UdpSocket::bind(address)?;
    let client = UdpSocket::bind(address)?;
    for socket in [&client, &server] {
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    }
    client.send_to(&[3; 777], server.local_addr()?)?;
    let (_, sender) = server.recv_from(&mut [0; 1024])?;
    server.send_to(&[4; 333], sender)?;
    client.recv_from(&mut [0; 1024])?;
    let udp = (
        Protocol::Udp,
        Endpoint::new(client.local_addr()?),
        Endpoint::new(server.local_addr()?),
    );
    Ok(vec![
        (
            tcp,
            Bytes {
                received: 2048,
                sent: 4096,
            },
        ),
        (
            (tcp.0, tcp.2, tcp.1),
            Bytes {
                received: 4096,
                sent: 2048,
            },
        ),
        (
            udp,
            Bytes {
                received: 333,
                sent: 777,
            },
        ),
        (
            (udp.0, udp.2, udp.1),
            Bytes {
                received: 777,
                sent: 333,
            },
        ),
    ])
}

#[derive(Default)]
struct Captured {
    external: Option<Key>,
    external_system_received: u64,
    system_endpoints: HashMap<(Endpoint, Endpoint), Bytes>,
    traffic: HashMap<(u32, Protocol), Bytes>,
    bytes: HashMap<Key, Bytes>,
    opened: HashSet<Key>,
    closed: HashSet<Key>,
    latest: Duration,
    processes: Vec<ProcessFact>,
}

impl Captured {
    fn record(&mut self, record: Record) {
        match record {
            Record::Network(fact) => self.accept(fact),
            Record::Process(fact) => self.processes.push(fact),
        }
    }
    fn accept(&mut self, fact: Fact) {
        let key = (fact.protocol, fact.local, fact.remote);
        if let Kind::Transfer(bytes) = fact.kind {
            let total = self.traffic.entry((fact.pid, fact.protocol)).or_default();
            total.received += bytes.received;
            total.sent += bytes.sent;
            if fact.pid == 4 && fact.protocol == Protocol::Tcp {
                let total = self
                    .system_endpoints
                    .entry((fact.local, fact.remote))
                    .or_default();
                total.received += bytes.received;
                total.sent += bytes.sent;
            }
        }
        if fact.pid == 4
            && self.external == Some(key)
            && let Kind::Transfer(bytes) = fact.kind
        {
            self.external_system_received += bytes.received;
        }
        if fact.pid != std::process::id() && !(fact.pid == 4 && self.external == Some(key)) {
            return;
        }
        self.latest = self.latest.max(fact.at);
        match fact.kind {
            Kind::Opened => {
                self.opened.insert(key);
            }
            Kind::Closed => {
                self.closed.insert(key);
            }
            Kind::Transfer(bytes) => {
                let total = self.bytes.entry(key).or_default();
                total.received += bytes.received;
                total.sent += bytes.sent;
            }
        }
    }
}

#[test]
#[ignore = "Starts an owned eight-second system ETW trace; requires tracing rights"]
fn native_trace_preserves_loopback_bytes_raw_qpc_and_callback_lifetime() -> io::Result<()> {
    let started = Instant::now();
    let clock = Clock::new()?;
    let mut session = Session::start()?;
    eprintln!("OwnedTraceName={}", session.trace_name());
    let (consumer, events, control) = session.consumer(clock)?;
    let (finished, completion) = mpsc::sync_channel(1);
    let mut work = Work::new();
    work.spawn("NetMap trace check", move || {
        let _ = finished.try_send(consumer.run());
    })?;
    // Even failed workloads finish the owned session and wait off-UI for its
    // registered consumer before propagating the failure to the test harness.
    let mut captured = Captured::default();
    let mut owners = crate::connections::native_tcp_owners();
    let expected = (|| -> io::Result<_> {
        let mut expected = workload("127.0.0.1:0")?;
        expected.extend(workload("[::1]:0")?);
        if let Some(external) = external_workload()? {
            captured.external = Some(external.0);
            expected.push(external);
        }
        while started.elapsed() < Duration::from_secs(8) {
            match events.recv_timeout(Duration::from_millis(20)) {
                Ok(record) => captured.record(record),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        Ok(expected)
    })();
    let live_statistics = session.statistics();
    let closed = control.close();
    let stopped = session.stop();
    work.finish(Duration::from_secs(5));
    while let Ok(record) = events.try_recv() {
        captured.record(record);
    }
    owners.extend(crate::connections::native_tcp_owners());
    closed?;
    let statistics = stopped?;
    let live_statistics = live_statistics?;
    completion
        .try_recv()
        .map_err(|_| io::Error::other("Trace consumer did not report completion"))??;
    for (key, bytes) in expected? {
        assert_eq!(
            captured.bytes.get(&key),
            Some(&bytes),
            "Directional transport bytes differ"
        );
        if key.0 == Protocol::Tcp {
            assert!(captured.opened.contains(&key), "Missing TCP opening");
            assert!(captured.closed.contains(&key), "Missing TCP close");
        }
    }
    assert!(captured.latest > Duration::ZERO && captured.latest <= clock.now()?);
    assert!(
        captured
            .processes
            .iter()
            .any(|fact| fact.pid == std::process::id() && fact.kind == ProcessKind::Existing),
        "Missing startup process rundown"
    );
    assert_eq!(control.losses(), (0, 0));
    assert_eq!(statistics.events_lost, 0);
    assert_eq!(statistics.buffers_lost, 0);
    assert!(live_statistics.buffers > 0 && live_statistics.buffer_kib > 0);
    if captured.external.is_some() {
        eprintln!(
            "ExternalSystemReceived={}",
            captured.external_system_received
        );
    }
    let mut matches = HashMap::<&str, Bytes>::new();
    for ((local, remote), bytes) in &captured.system_endpoints {
        let same = owners.iter().any(|(_, l, r)| l == local && r == remote);
        let reverse = owners.iter().any(|(_, l, r)| l == remote && r == local);
        let name = match (same, reverse) {
            (true, false) => "same",
            (false, true) => "reversed",
            (true, true) => "both",
            (false, false) => "neither",
        };
        let total = matches.entry(name).or_default();
        total.received += bytes.received;
        total.sent += bytes.sent;
    }
    eprintln!("SystemTcpTableMatches={matches:?}");
    let mut traffic: Vec<_> = captured.traffic.iter().collect();
    traffic.sort_by_key(|(_, bytes)| std::cmp::Reverse(bytes.received + bytes.sent));
    for ((pid, protocol), bytes) in traffic.into_iter().take(6) {
        eprintln!("SourceTraffic: pid={pid}; protocol={protocol:?}; bytes={bytes:?}");
    }
    eprintln!(
        "Owned IPv4/IPv6 TCP+UDP facts verified; raw QPC valid; {} buffers x {} KiB; consumer returned, owned session stopped",
        live_statistics.buffers, live_statistics.buffer_kib
    );
    Ok(())
}

#[test]
#[ignore = "Child workload for the owned native collector check"]
fn native_trace_child_workload() -> io::Result<()> {
    if std::env::var_os("NETMAP_TRACE_CHILD").is_none() {
        return Ok(());
    }
    workload("127.0.0.1:0")?;
    workload("[::1]:0")?;
    Ok(())
}

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
#[ignore = "Starts an owned eight-second system ETW collector; requires tracing rights"]
fn native_collector_preserves_short_processes_directional_bytes_and_rates() -> io::Result<()> {
    let started = Instant::now();
    let baseline = ProcessResources::read()?;
    let mut peak = baseline;
    eprintln!(
        "CollectorResourceScope={{\"process_id\":{},\"process\":\"Current test process: harness, collector, consumer and controlled workload; owned child excluded\",\"excluded\":\"UI, MMDB and GPU are not loaded or measured\",\"baseline\":\"Before Collector construction\",\"peak\":\"Independent maxima at startup, after workload and publications; not a continuous or isolated allocation peak\",\"observations\":\"Whole-PC retained published observations, not allocation bytes\",\"etw\":\"Owned session live buffer count times reported buffer size; not kernel-private memory\"}}",
        std::process::id()
    );
    baseline.report("baseline", baseline, started);
    let (mut collector, consumer) = Collector::new()?;
    let consumer = consumer.ok_or_else(|| io::Error::other("Tracing rights unavailable"))?;
    eprintln!("OwnedTraceName={}", collector.trace_name().unwrap());
    let (finished, completion) = mpsc::sync_channel(1);
    let mut work = Work::new();
    work.spawn("NetMap collector check", move || {
        let _ = finished.try_send(consumer.run());
    })?;
    // Test assertions can panic too; finish native consumption before unwinding
    // the harness, just as we do for an I/O failure.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> io::Result<()> {
        let highlight = Highlight {
            bright: Duration::from_secs(10),
            fade: Duration::from_secs(20),
        };
        let initial = collector.sample(highlight)?;
        trace_resources(
            "startup",
            &mut collector,
            initial.snapshot.observations.len(),
        )?;
        drop(initial);
        sample_resources("startup", baseline, &mut peak, started)?;
        let mut expected = workload("127.0.0.1:0")?;
        expected.extend(workload("[::1]:0")?);
        expected.extend(external_workload()?);
        let mut child = OwnedChild(
            Command::new(std::env::current_exe()?)
                .args([
                    "--ignored",
                    "--exact",
                    "app::work::trace_checks::native_trace_child_workload",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("NETMAP_TRACE_CHILD", "1")
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let child_pid = child.0.id();
        eprintln!("OwnedChildPid={child_pid}");
        sample_resources("after_workload", baseline, &mut peak, started)?;
        let mut totals = HashMap::<Key, Bytes>::new();
        let mut child_bytes = HashMap::<Protocol, Bytes>::new();
        let mut identities = HashMap::new();
        let mut generations = HashSet::new();
        let mut ended = HashSet::new();
        let mut measured = false;
        let mut next = Duration::from_secs(2);
        let mut coverage = Coverage::WarmingUp;
        while started.elapsed() < Duration::from_secs(8) {
            collector.poll();
            if started.elapsed() >= next {
                let publication = collector.sample(highlight)?;
                let snapshot = &publication.snapshot;
                trace_resources("publication", &mut collector, snapshot.observations.len())?;
                sample_resources("publication", baseline, &mut peak, started)?;
                coverage = snapshot.coverage;
                eprintln!(
                    "CollectorSample={coverage:?}; bytes={:?}; unattributed={:?}; ambiguous={}; limitation={:?}",
                    snapshot.bytes,
                    snapshot.unattributed,
                    snapshot.ambiguous_events,
                    publication.limitation
                );
                let mut traffic = HashMap::<u32, Bytes>::new();
                for row in snapshot.observations.iter() {
                    let bytes = traffic.entry(row.tuple.process.pid).or_default();
                    bytes.received += row.bytes.received;
                    bytes.sent += row.bytes.sent;
                }
                let mut traffic: Vec<_> = traffic.into_iter().collect();
                traffic.sort_by_key(|(_, bytes)| std::cmp::Reverse(bytes.received + bytes.sent));
                for (pid, bytes) in traffic.into_iter().take(6) {
                    eprintln!("AttributedTraffic: pid={pid}; bytes={bytes:?}");
                }
                for row in snapshot.observations.iter() {
                    let protocol = match row.life {
                        Life::Tcp(_) | Life::EndedTcp => Protocol::Tcp,
                        Life::Udp { .. } | Life::ExpiredUdp { .. } => Protocol::Udp,
                    };
                    if row.tuple.process.pid == std::process::id() {
                        let key = (protocol, row.tuple.local, row.tuple.remote);
                        if expected.iter().any(|(expected, _)| *expected == key) {
                            let total = totals.entry(key).or_default();
                            total.received += row.bytes.received;
                            total.sent += row.bytes.sent;
                            assert_eq!(
                                *identities.entry(key).or_insert(row.id),
                                row.id,
                                "One connection changed identity"
                            );
                            generations.insert(row.tuple.process);
                            if row.life == Life::EndedTcp {
                                ended.insert(key);
                            }
                            measured |= row.bytes.active() && row.rates.is_some();
                        }
                    } else if row.tuple.process.pid == child_pid {
                        let total = child_bytes.entry(protocol).or_default();
                        total.received += row.bytes.received;
                        total.sent += row.bytes.sent;
                    }
                }
                next += Duration::from_secs(2);
            }
            thread::sleep(Duration::from_millis(20));
        }
        let status = child
            .0
            .try_wait()?
            .ok_or_else(|| io::Error::other("Child workload did not exit"))?;
        assert!(status.success(), "Child workload failed");
        assert_eq!(coverage, Coverage::Complete);
        for (key, bytes) in expected {
            assert_eq!(
                totals.get(&key),
                Some(&bytes),
                "Collector transport byte conservation failed: {key:?}"
            );
            if key.0 == Protocol::Tcp {
                assert!(ended.contains(&key), "Short TCP lifetime disappeared");
            }
        }
        assert_eq!(
            generations.len(),
            1,
            "One process split into multiple generations"
        );
        assert!(
            measured,
            "No measured directional rates reached publication"
        );
        assert_eq!(
            child_bytes.get(&Protocol::Tcp),
            Some(&Bytes {
                received: 12288,
                sent: 12288
            })
        );
        assert_eq!(
            child_bytes.get(&Protocol::Udp),
            Some(&Bytes {
                received: 2220,
                sent: 2220
            })
        );
        Ok(())
    }));
    drop(collector);
    work.finish(Duration::from_secs(5));
    drop(work);
    peak.report("sampled_peak", baseline, started);
    let after_cleanup = ProcessResources::read().map(|sample| {
        sample.report("after_cleanup", baseline, started);
    });
    completion
        .try_recv()
        .map_err(|_| io::Error::other("Collector consumer did not return"))??;
    after_cleanup?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
