//! Registered, bounded jobs for the opt-in native shutdown checks.

use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::AtomicU64,
};
use windows::Win32::System::SystemInformation::GetTickCount64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    HttpRead,
    Uncooperative,
    PreferencesCommit,
    PreferencesFallback,
    DatasetFallback,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::HttpRead => "http",
            Self::Uncooperative => "uncooperative",
            Self::PreferencesCommit => "preferences-commit",
            Self::PreferencesFallback => "preferences-fallback",
            Self::DatasetFallback => "dataset-fallback",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Ready {
    PendingRead { bytes: usize },
    Uncooperative,
}

#[derive(Default)]
struct Proof {
    client_done: AtomicBool,
    client_ok: AtomicBool,
    server_done: AtomicBool,
    server_ok: AtomicBool,
    completed: AtomicBool,
    pending_tick: AtomicU64,
}

thread_local! {
    static ACTIVE: Cell<Option<Kind>> = const { Cell::new(None) };
    static FINISH: Cell<Option<(Instant, Instant)>> = const { Cell::new(None) };
    static PROOF: std::cell::RefCell<Option<Arc<Proof>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) struct Check {
    ready: mpsc::Receiver<io::Result<Ready>>,
}

impl Check {
    pub(crate) fn ready(&self) -> io::Result<Option<Ready>> {
        match self.ready.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::other(
                "Shutdown proof worker retired before readiness",
            )),
        }
    }
}

pub(crate) fn start(app: &Application, kind: Kind) -> io::Result<Check> {
    ACTIVE.set(Some(kind));
    let proof = Arc::new(Proof::default());
    PROOF.with(|slot| *slot.borrow_mut() = Some(Arc::clone(&proof)));
    let (ready, receiver) = mpsc::sync_channel(1);
    let mut work = app.work.borrow_mut();
    match kind {
        Kind::HttpRead => {
            let (address, addresses) = mpsc::sync_channel(1);
            let server_proof = Arc::clone(&proof);
            let cancelled = Arc::clone(&work.cancelled);
            work.spawn("NetMap native withheld HTTP server", move || {
                let result = server(&address, &cancelled, &server_proof);
                server_proof
                    .server_ok
                    .store(result.is_ok(), Ordering::Release);
                server_proof.server_done.store(true, Ordering::Release);
                report_result(kind, "server_finished", result.as_ref().err());
            })?;
            let cancelled = Arc::clone(&work.cancelled);
            work.spawn("NetMap native pending HTTP read", move || {
                let result = (|| -> io::Result<()> {
                    let port = addresses
                        .recv_timeout(Duration::from_secs(3))
                        .map_err(|error| io::Error::other(error.to_string()))??;
                    let report =
                        http::native_checks::pending_body_read(port, &cancelled, |bytes| {
                            proof.pending_tick.store(tick(), Ordering::Release);
                            let _ = ready.try_send(Ok(Ready::PendingRead { bytes }));
                        })
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    require(
                        report.bytes == 65536 && report.context_reclaimed,
                        "Request cancellation proof did not finish",
                    )?;
                    Ok(())
                })();
                if let Err(error) = &result {
                    let _ = ready.try_send(Err(io::Error::other(error.to_string())));
                }
                proof.client_ok.store(result.is_ok(), Ordering::Release);
                // The server may close only after Request::drop and callback
                // reclamation, including all error paths returned by the client.
                proof.client_done.store(true, Ordering::Release);
                report_result(kind, "client_finished", result.as_ref().err());
            })?;
        }
        Kind::Uncooperative => {
            let (held, gate) = mpsc::channel::<()>();
            work.spawn("NetMap native deliberately uncooperative gate", move || {
                // Keep a sender inside this job. Dropping Host/Check cannot
                // disconnect or release the cancellation-ignoring bounded wait.
                let _held = held;
                record(kind, "gate_waiting", "\"maximum_ms\":20000");
                let _ = ready.try_send(Ok(Ready::Uncooperative));
                let _ = gate.recv_timeout(Duration::from_secs(20));
                record(kind, "gate_returned", "");
            })?;
        }
        Kind::PreferencesCommit | Kind::PreferencesFallback | Kind::DatasetFallback => {
            return Err(io::Error::other(
                "Application shutdown observes its actual registered operation",
            ));
        }
    }
    Ok(Check { ready: receiver })
}

// Observe the existing finalization hooks without registering another job.
pub(crate) fn observe(kind: Kind) {
    ACTIVE.set(Some(kind));
}

fn server(
    address: &mpsc::SyncSender<io::Result<u16>>,
    cancelled: &AtomicBool,
    proof: &Proof,
) -> io::Result<()> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let _ = address.try_send(Ok(port));
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                require(
                    !cancelled.load(Ordering::Acquire) && Instant::now() < deadline,
                    "Owned HTTP accept was cancelled or timed out",
                )?;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    };
    read_headers(&mut stream, cancelled, deadline)?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 131072\r\nConnection: close\r\n\r\n")?;
    stream.write_all(&[0x5a; 65536])?;
    record(
        Kind::HttpRead,
        "server_withholding",
        "\"sent_bytes\":65536,\"declared_bytes\":131072",
    );
    while !proof.client_done.load(Ordering::Acquire) {
        // Deliberately ignore Work cancellation here: remote EOF must not
        // substitute for the client's actual outstanding-read cancellation.
        require(
            Instant::now() < deadline,
            "Client teardown did not release the owned HTTP server",
        )?;
        thread::sleep(Duration::from_millis(10));
    }
    record(Kind::HttpRead, "server_released_after_client", "");
    Ok(())
}

fn read_headers(
    stream: &mut TcpStream,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while !request.windows(4).any(|part| part == b"\r\n\r\n") {
        require(
            !cancelled.load(Ordering::Acquire) && Instant::now() < deadline && request.len() < 8192,
            "Owned HTTP request headers were cancelled, oversized or timed out",
        )?;
        match stream.read(&mut buffer) {
            Ok(0) => return Err(io::Error::other("Owned HTTP client closed before headers")),
            Ok(count) => request.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn finish_started(allowance: Duration, deadline: Instant) {
    if let Some(kind) = ACTIVE.get() {
        FINISH.set(Some((Instant::now(), deadline)));
        record(
            kind,
            "finish_started",
            &format!("\"allowance_ms\":{}", allowance.as_millis()),
        );
    }
}

pub(crate) fn fallback_initiated() {
    if let (Some(kind), Some((started, deadline))) = (ACTIVE.get(), FINISH.get()) {
        record(
            kind,
            "fallback_initiated",
            &format!(
                "\"elapsed_ms\":{},\"deadline_lateness_us\":{}",
                started.elapsed().as_millis(),
                Instant::now()
                    .saturating_duration_since(deadline)
                    .as_micros()
            ),
        );
    }
}

pub(crate) fn finish_completed() {
    if let (Some(kind), Some((started, _))) = (ACTIVE.get(), FINISH.get()) {
        PROOF.with(|slot| {
            if let Some(proof) = slot.borrow().as_ref() {
                proof.completed.store(true, Ordering::Release);
            }
        });
        record(
            kind,
            "finish_completed",
            &format!("\"elapsed_ms\":{}", started.elapsed().as_millis()),
        );
        ACTIVE.set(None);
        FINISH.set(None);
    }
}

pub(crate) fn verify_http_completed() -> io::Result<()> {
    PROOF.with(|slot| {
        let proof = slot
            .borrow_mut()
            .take()
            .ok_or_else(|| io::Error::other("No HTTP shutdown proof was registered"))?;
        require(
            proof.completed.load(Ordering::Acquire)
                && proof.client_ok.load(Ordering::Acquire)
                && proof.server_ok.load(Ordering::Acquire)
                && proof.server_done.load(Ordering::Acquire)
                && proof.pending_tick.load(Ordering::Acquire) != 0,
            "Registered HTTP workers did not complete their cancellation proof",
        )
    })
}

pub(crate) fn tick() -> u64 {
    // SAFETY: Read a monotonic system timestamp only; no system state changes.
    unsafe { GetTickCount64() }
}

pub(crate) fn record(kind: Kind, event: &str, fields: &str) {
    eprintln!(
        "NETMAP_SHUTDOWN {{\"kind\":\"{}\",\"event\":\"{}\",\"tick_ms\":{}{}{}}}",
        kind.name(),
        event,
        tick(),
        if fields.is_empty() { "" } else { "," },
        fields
    );
    let _ = io::stderr().flush();
}

fn report_result(kind: Kind, event: &str, error: Option<&io::Error>) {
    let detail = error.map(|error| {
        error
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
    });
    record(
        kind,
        event,
        &format!(
            "\"passed\":{}{}",
            error.is_none(),
            detail.map_or(String::new(), |error| format!(",\"error\":\"{error}\""))
        ),
    );
}

fn require(value: bool, reason: &str) -> io::Result<()> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason))
    }
}
