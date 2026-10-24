//! Read-only, same-logon transport for the explicit elevated collector role.
//! Construction, publish/poll, reads and Drop all belong on Work's workers.
mod wire;

use std::{
    io,
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING,
            ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_READ, HANDLE, HLOCAL, LocalFree,
            WIN32_ERROR,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_ELEVATION,
            TOKEN_GROUPS, TOKEN_QUERY, TokenElevation, TokenLogonSid,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING, PIPE_ACCESS_OUTBOUND,
            ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, WriteFile,
        },
        System::{
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
                GetNamedPipeServerProcessId, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                CreateEventW, GetCurrentProcess, OpenProcess, OpenProcessToken,
                PROCESS_QUERY_LIMITED_INFORMATION, ResetEvent, WaitForSingleObject,
            },
        },
    },
    core::{GUID, PCWSTR, PWSTR},
};

use super::Publication;

const CLIENTS: u32 = 4;
// Three publication intervals allow a briefly busy presentation to catch up.
// A client unable to drain one bounded frame by then must reconnect to latest.
const IO_DEADLINE: Duration = Duration::from_secs(6);
const CONNECT_DEADLINE: Duration = Duration::from_secs(2);
const MAX_AGE: Duration = Duration::from_secs(10);

fn native<T>(result: windows::core::Result<T>) -> io::Result<T> {
    result.map_err(|error| io::Error::other(error.to_string()))
}

fn is(error: &windows::core::Error, code: WIN32_ERROR) -> bool {
    error.code() == windows::core::HRESULT::from_win32(code.0)
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: This value exclusively owns a successful native open.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct LocalAllocation(HLOCAL);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: Windows returned this allocation with the LocalFree contract.
        unsafe {
            let _ = LocalFree(Some(self.0));
        }
    }
}

fn token(process: HANDLE) -> io::Result<Handle> {
    let mut value = HANDLE::default();
    // SAFETY: The synchronous output is exclusively borrowed initialized storage.
    native(unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut value) })?;
    Ok(Handle(value))
}

fn logon_sid(token: &Handle) -> io::Result<String> {
    let mut length = 0;
    // SAFETY: A null-buffer size query writes only the live length output.
    match unsafe { GetTokenInformation(token.0, TokenLogonSid, None, 0, &mut length) } {
        Err(error) if is(&error, ERROR_INSUFFICIENT_BUFFER) => (),
        other => {
            native(other)?;
            return Err(wire::invalid());
        }
    }
    if length == 0 || length > 64 * 1024 {
        return Err(wire::invalid());
    }
    let capacity = length;
    let mut buffer = vec![0_u64; (capacity as usize).div_ceil(size_of::<u64>())];
    // SAFETY: Aligned initialized output stays exclusively borrowed through this
    // call. Interior SID pointers are consumed before the allocation is dropped.
    native(unsafe {
        GetTokenInformation(
            token.0,
            TokenLogonSid,
            Some(buffer.as_mut_ptr().cast()),
            capacity,
            &mut length,
        )
    })?;
    if length > capacity {
        return Err(wire::invalid());
    }
    if size_of_val(buffer.as_slice()) < size_of::<TOKEN_GROUPS>() {
        return Err(wire::invalid());
    }
    // SAFETY: GetTokenInformation supplied a complete aligned TOKEN_GROUPS.
    let groups = unsafe { &*buffer.as_ptr().cast::<TOKEN_GROUPS>() };
    if groups.GroupCount != 1 {
        return Err(wire::invalid());
    }
    let mut text = PWSTR::null();
    // SAFETY: The SID remains backed by buffer; the output is LocalFree-owned.
    native(unsafe { ConvertSidToStringSidW(groups.Groups[0].Sid, &mut text) })?;
    let allocation = LocalAllocation(HLOCAL(text.0.cast()));
    // SAFETY: The native conversion returned a terminated UTF-16 string.
    let result = unsafe { text.to_string() }.map_err(|_| wire::invalid());
    drop(allocation);
    result
}

fn elevated(token: &Handle) -> io::Result<bool> {
    let mut elevation = TOKEN_ELEVATION::default();
    let size = size_of::<TOKEN_ELEVATION>() as u32;
    let mut length = 0;
    // SAFETY: This fixed-size native record needs no size probe or padding.
    native(unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size,
            &mut length,
        )
    })?;
    if length != size {
        return Err(wire::invalid());
    }
    Ok(elevation.TokenIsElevated != 0)
}

fn own_logon() -> io::Result<(Handle, String)> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo handle, never closed.
    let token = token(unsafe { GetCurrentProcess() })?;
    let sid = logon_sid(&token)?;
    Ok((token, sid))
}

fn pipe_name(sid: &str) -> Vec<u16> {
    format!(r"\\.\pipe\NetMap-{sid}-v{}", wire::VERSION)
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

fn counter() -> io::Result<(i64, i64)> {
    let (mut now, mut frequency) = (0, 0);
    // SAFETY: Both outputs are live exclusively borrowed i64 storage.
    unsafe {
        native(QueryPerformanceFrequency(&mut frequency))?;
        native(QueryPerformanceCounter(&mut now))?;
    }
    if now < 0 || frequency <= 0 {
        return Err(wire::invalid());
    }
    Ok((now, frequency))
}

enum Buffer {
    None,
    Read(Vec<u8>),
    Write(Arc<[u8]>),
}

// Own all storage Windows may touch, including a stable OVERLAPPED allocation.
// CancelIoEx requests cancellation; only completion permits reusing/freeing it.
struct Operation {
    handle: Handle,
    event: Handle,
    overlapped: Box<OVERLAPPED>,
    pending: bool,
    buffer: Buffer,
    offset: usize,
}

impl Operation {
    fn new(handle: Handle) -> io::Result<Self> {
        // SAFETY: An unnamed manual-reset event has no borrowed inputs.
        let event = Handle(native(unsafe {
            CreateEventW(None, true, false, PCWSTR::null())
        })?);
        let overlapped = Box::new(OVERLAPPED {
            hEvent: event.0,
            ..Default::default()
        });
        Ok(Self {
            handle,
            event,
            overlapped,
            pending: false,
            buffer: Buffer::None,
            offset: 0,
        })
    }

    fn prepare(&mut self) -> io::Result<()> {
        debug_assert!(!self.pending);
        // SAFETY: No outstanding operation still refers to this event/record.
        native(unsafe { ResetEvent(self.event.0) })?;
        *self.overlapped = OVERLAPPED {
            hEvent: self.event.0,
            ..Default::default()
        };
        Ok(())
    }

    fn connect(&mut self) -> io::Result<()> {
        self.buffer = Buffer::None;
        self.offset = 0;
        self.prepare()?;
        // SAFETY: handle is an owned server instance and overlapped is retained.
        match unsafe { ConnectNamedPipe(self.handle.0, Some(&mut *self.overlapped)) } {
            Ok(()) => Ok(()),
            Err(error) if is(&error, ERROR_PIPE_CONNECTED) => Ok(()),
            Err(error) if is(&error, ERROR_IO_PENDING) => {
                self.pending = true;
                Ok(())
            }
            Err(error) => native(Err(error)),
        }
    }

    fn complete(&mut self) -> io::Result<bool> {
        if !self.pending {
            return Ok(true);
        }
        let mut transferred = 0;
        // SAFETY: No storage is released or reused before this operation ends.
        match unsafe {
            GetOverlappedResult(self.handle.0, &*self.overlapped, &mut transferred, false)
        } {
            Err(error) if is(&error, ERROR_IO_INCOMPLETE) => Ok(false),
            result => {
                self.pending = false;
                native(result)?;
                self.advance(transferred)?;
                Ok(true)
            }
        }
    }

    fn advance(&mut self, transferred: u32) -> io::Result<()> {
        if !matches!(self.buffer, Buffer::None) && transferred == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Collector pipe closed without progress",
            ));
        }
        self.offset = self
            .offset
            .checked_add(transferred as usize)
            .ok_or_else(wire::invalid)?;
        if self.offset > self.length() {
            return Err(wire::invalid());
        }
        Ok(())
    }

    fn length(&self) -> usize {
        match &self.buffer {
            Buffer::None => 0,
            Buffer::Read(buffer) => buffer.len(),
            Buffer::Write(buffer) => buffer.len(),
        }
    }

    fn transfer(&mut self) -> io::Result<()> {
        self.prepare()?;
        let mut transferred = 0;
        // SAFETY: The stable owned buffers and OVERLAPPED remain in self until
        // completion, including cancellation and any immediate-error path.
        let result = unsafe {
            match &mut self.buffer {
                Buffer::Read(buffer) => ReadFile(
                    self.handle.0,
                    Some(&mut buffer[self.offset..]),
                    Some(&mut transferred),
                    Some(&mut *self.overlapped),
                ),
                Buffer::Write(buffer) => WriteFile(
                    self.handle.0,
                    Some(&buffer[self.offset..]),
                    Some(&mut transferred),
                    Some(&mut *self.overlapped),
                ),
                Buffer::None => return Err(wire::invalid()),
            }
        };
        match result {
            Ok(()) => self.advance(transferred),
            Err(error) if is(&error, ERROR_IO_PENDING) => {
                self.pending = true;
                Ok(())
            }
            Err(error) => native(Err(error)),
        }
    }

    fn cancel(&self) {
        if self.pending {
            // SAFETY: Cancellation does not release the retained native inputs.
            unsafe {
                let _ = CancelIoEx(self.handle.0, Some(&*self.overlapped));
            }
        }
    }

    fn read(
        &mut self,
        length: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<Vec<u8>> {
        self.buffer = Buffer::Read(vec![0; length]);
        self.offset = 0;
        while self.offset < length {
            if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
                self.cancel();
                return Err(io::Error::new(
                    if cancelled.load(Ordering::Acquire) {
                        io::ErrorKind::Interrupted
                    } else {
                        io::ErrorKind::TimedOut
                    },
                    "Collector pipe read cancelled or timed out",
                ));
            }
            if self.complete()? && self.offset < length {
                self.transfer()?;
            }
            if self.pending {
                // SAFETY: The event is owned and remains valid throughout wait.
                unsafe {
                    WaitForSingleObject(self.event.0, 50);
                }
            }
        }
        match std::mem::replace(&mut self.buffer, Buffer::None) {
            Buffer::Read(buffer) => Ok(buffer),
            _ => Err(wire::invalid()),
        }
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        self.cancel();
        if self.pending {
            let mut transferred = 0;
            // SAFETY: Drop runs off UI. Retain all buffers/handles while waiting
            // for cancellation completion. Work's independent shutdown deadline
            // remains runnable if the native cancellation itself stalls.
            unsafe {
                let _ =
                    GetOverlappedResult(self.handle.0, &*self.overlapped, &mut transferred, true);
            }
        }
    }
}

enum State {
    Connecting,
    Idle { sent: u64 },
    Writing { sequence: u64, deadline: Instant },
    Cancelling,
}

struct Slot {
    operation: Operation,
    state: State,
}

impl Slot {
    fn reset(&mut self) -> io::Result<()> {
        debug_assert!(!self.operation.pending);
        // SAFETY: No pending operation still uses this owned server instance.
        unsafe {
            let _ = DisconnectNamedPipe(self.operation.handle.0);
        }
        self.operation.connect()?;
        self.state = State::Connecting;
        Ok(())
    }

    fn poll(&mut self, latest: &Option<(u64, Arc<[u8]>)>, cancelled: bool) -> io::Result<()> {
        if cancelled
            || matches!(self.state, State::Writing { deadline, .. } if Instant::now() >= deadline)
        {
            self.operation.cancel();
            self.state = State::Cancelling;
        }
        match self.operation.complete() {
            Ok(false) => return Ok(()),
            Err(_) if !cancelled => return self.reset(),
            Err(error) => return Err(error),
            Ok(true) => (),
        }
        match self.state {
            State::Connecting => self.state = State::Idle { sent: 0 },
            State::Writing { sequence, .. } if self.operation.offset == self.operation.length() => {
                self.state = State::Idle { sent: sequence };
                self.operation.buffer = Buffer::None;
            }
            State::Writing { .. } => {
                return match self.operation.transfer() {
                    Ok(()) => Ok(()),
                    Err(_) => self.reset(),
                };
            }
            State::Cancelling if !cancelled => return self.reset(),
            State::Cancelling => return Ok(()),
            State::Idle { .. } => (),
        }
        if let State::Idle { sent } = self.state
            && let Some((sequence, frame)) = latest
            && *sequence > sent
        {
            self.operation.buffer = Buffer::Write(Arc::clone(frame));
            self.operation.offset = 0;
            self.state = State::Writing {
                sequence: *sequence,
                deadline: Instant::now() + IO_DEADLINE,
            };
            if self.operation.transfer().is_err() {
                return self.reset();
            }
        }
        Ok(())
    }
}

pub(crate) struct Server {
    epoch: GUID,
    sequence: u64,
    latest: Option<(u64, Arc<[u8]>)>,
    slots: Vec<Slot>,
}

impl Server {
    pub(crate) fn new() -> io::Result<Self> {
        let (token, sid) = own_logon()?;
        if !elevated(&token)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The explicit collector broker requires an elevated launch",
            ));
        }
        // Read-only logon access excludes FILE_CREATE_PIPE_INSTANCE, which is
        // aliased by FILE_APPEND_DATA inside GENERIC_WRITE. Only elevated
        // Admin/System may create instances; no client command channel exists.
        let sddl: Vec<_> = format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GR;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: The terminated descriptor is borrowed synchronously; Windows
        // returns LocalFree-owned storage retained through every pipe creation.
        native(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        })?;
        let _descriptor = LocalAllocation(HLOCAL(descriptor.0));
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let name = pipe_name(&sid);
        let mut slots = Vec::with_capacity(CLIENTS as usize);
        for index in 0..CLIENTS {
            let first = if index == 0 {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                FILE_FLAGS_AND_ATTRIBUTES(0)
            };
            // SAFETY: Native copies the name/descriptor during this call. Reject
            // foreign first instances; never open/adopt an existing pipe server.
            let handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(name.as_ptr()),
                    PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED | first,
                    PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    CLIENTS,
                    64 * 1024,
                    0,
                    0,
                    Some(&security),
                )
            };
            if handle.is_invalid() {
                return Err(io::Error::last_os_error());
            }
            let mut operation = Operation::new(Handle(handle))?;
            operation.connect()?;
            slots.push(Slot {
                operation,
                state: State::Connecting,
            });
        }
        Ok(Self {
            epoch: native(GUID::new())?,
            sequence: 0,
            latest: None,
            slots,
        })
    }

    pub(crate) fn publish(&mut self, publication: &Publication) -> io::Result<()> {
        let sequence = self.sequence.checked_add(1).ok_or_else(wire::invalid)?;
        let (now, frequency) = counter()?;
        let age = publication
            .sampled
            .elapsed()
            .as_nanos()
            .checked_mul(frequency as u128)
            .ok_or_else(wire::invalid)?
            / 1_000_000_000;
        let sampled_qpc = now
            .checked_sub(i64::try_from(age).map_err(|_| wire::invalid())?)
            .ok_or_else(wire::invalid)?;
        let frame = wire::encode(publication, self.epoch, sequence, sampled_qpc, frequency)?;
        self.sequence = sequence;
        self.latest = Some((sequence, frame));
        Ok(())
    }

    /// Call regularly on the collector worker; never waits for clients or holds
    /// borrowed snapshots. One latest frame plus at most four in-flight frames.
    pub(crate) fn poll(&mut self, cancelled: &AtomicBool) -> io::Result<()> {
        let cancelled = cancelled.load(Ordering::Acquire);
        for slot in &mut self.slots {
            slot.poll(&self.latest, cancelled)?;
        }
        Ok(())
    }

    /// Read after poll. Disconnects become visible on the next publication's
    /// write; Work owns any startup/reconnect grace before stopping collection.
    pub(crate) fn has_clients(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot.state, State::Idle { .. } | State::Writing { .. }))
    }
}

pub(crate) struct Received {
    pub epoch: GUID,
    pub sequence: u64,
    pub publication: Publication,
}

pub(crate) struct Client {
    operation: Option<Operation>,
    epoch: Option<GUID>,
    sequence: u64,
    through: Duration,
}

impl Client {
    pub(crate) fn connect(cancelled: &AtomicBool) -> io::Result<Self> {
        let (_, sid) = own_logon()?;
        let name = pipe_name(&sid);
        let deadline = Instant::now() + CONNECT_DEADLINE;
        let handle = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Collector connection cancelled",
                ));
            }
            // SAFETY: The terminated local name is live through this call. Read
            // only, no inheritance, identification-only impersonation ceiling.
            match unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    GENERIC_READ.0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    None,
                )
            } {
                Ok(handle) => break Handle(handle),
                Err(error) if is(&error, ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(error) => return native(Err(error)),
            }
        };
        let mut pid = 0;
        // SAFETY: The connected local pipe and PID output stay live through call.
        native(unsafe { GetNamedPipeServerProcessId(handle.0, &mut pid) })?;
        // SAFETY: Open only the actual pipe server PID for metadata verification.
        let process = Handle(native(unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
        })?);
        let token = token(process.0)?;
        if logon_sid(&token)? != sid || !elevated(&token)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Collector pipe server is not elevated in this logon",
            ));
        }
        Ok(Self {
            operation: Some(Operation::new(handle)?),
            epoch: None,
            sequence: 0,
            through: Duration::ZERO,
        })
    }

    pub(crate) fn read(&mut self, cancelled: &AtomicBool) -> io::Result<Received> {
        let result = self.receive(cancelled);
        if result.is_err() {
            // A partial/cancelled frame cannot be resumed as a new frame. Drop
            // off UI retains its storage through native cancellation completion.
            self.operation.take();
        }
        result
    }

    fn receive(&mut self, cancelled: &AtomicBool) -> io::Result<Received> {
        let operation = self.operation.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "Collector pipe is disconnected",
            )
        })?;
        let deadline = Instant::now() + IO_DEADLINE;
        let length = operation.read(4, deadline, cancelled)?;
        let length = u32::from_le_bytes(length.try_into().map_err(|_| wire::invalid())?) as usize;
        if length == 0 || length > wire::MAX_FRAME {
            return Err(wire::invalid());
        }
        let frame = operation.read(length, deadline, cancelled)?;
        let decoded = wire::decode(&frame)?;
        if decoded.sequence <= self.sequence
            || self.epoch.is_some_and(|epoch| epoch != decoded.epoch)
            || decoded.snapshot.interval.end < self.through
        {
            return Err(wire::invalid());
        }
        let (now, frequency) = counter()?;
        if frequency != decoded.frequency || decoded.sampled_qpc > now {
            return Err(wire::invalid());
        }
        let elapsed = u64::try_from(now - decoded.sampled_qpc).map_err(|_| wire::invalid())?;
        let age = Duration::new(
            elapsed / frequency as u64,
            ((u128::from(elapsed % frequency as u64) * 1_000_000_000) / frequency as u128) as u32,
        );
        if age > MAX_AGE {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Collector snapshot is stale",
            ));
        }
        let sampled = Instant::now().checked_sub(age).ok_or_else(wire::invalid)?;
        self.sequence = decoded.sequence;
        self.epoch = Some(decoded.epoch);
        self.through = decoded.snapshot.interval.end;
        Ok(Received {
            epoch: decoded.epoch,
            sequence: decoded.sequence,
            publication: Publication {
                snapshot: Arc::new(decoded.snapshot),
                sampled,
                limitation: decoded.limitation,
                names: Arc::new(decoded.names),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_current_token_identity_and_elevation() -> io::Result<()> {
        let (token, _) = own_logon()?;
        elevated(&token)?;
        Ok(())
    }
}
