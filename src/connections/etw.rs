use std::{
    io,
    mem::{size_of, size_of_val},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{
            ERROR_CANCELLED, ERROR_CTX_CLOSE_PENDING, ERROR_INVALID_HANDLE, ERROR_SUCCESS,
            GetLastError,
        },
        System::{
            Diagnostics::Etw::*,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
    },
    core::{GUID, PCWSTR, PWSTR},
};

use super::{Bytes, Endpoint, Protocol};

const TCP: GUID = GUID::from_u128(0x9a280ac0_c8e0_11d1_84e2_00c04fb998a2);
const UDP: GUID = GUID::from_u128(0xbf3a50c5_a9c9_4988_a005_2df0b7c80f80);
const PROCESS: GUID = GUID::from_u128(0x3d6fa8d0_fe05_11d0_9dda_00c04fd7ba7c);
const INVALID_TRACE: u64 = u64::MAX;
const QUEUED_EVENTS: usize = 4096;

#[derive(Clone, Copy)]
pub(crate) struct Clock {
    epoch: i64,
    frequency: i64,
}

impl Clock {
    pub(crate) fn new() -> io::Result<Self> {
        let (mut epoch, mut frequency) = (0, 0);
        // SAFETY: Both native outputs point to live exclusively borrowed i64s.
        unsafe {
            QueryPerformanceFrequency(&mut frequency)
                .map_err(|error| io::Error::other(error.to_string()))?;
            QueryPerformanceCounter(&mut epoch)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        if frequency <= 0 {
            return Err(io::Error::other("Invalid performance-counter frequency"));
        }
        Ok(Self { epoch, frequency })
    }

    pub(crate) fn now(self) -> io::Result<Duration> {
        let mut ticks = 0;
        // SAFETY: Native output points to a live exclusively borrowed i64.
        unsafe { QueryPerformanceCounter(&mut ticks) }
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.at(ticks)
            .ok_or_else(|| io::Error::other("Performance counter preceded its epoch"))
    }

    fn at(self, ticks: i64) -> Option<Duration> {
        let ticks = u64::try_from(i128::from(ticks) - i128::from(self.epoch)).ok()?;
        let frequency = self.frequency as u64;
        Some(Duration::new(
            ticks / frequency,
            ((u128::from(ticks % frequency) * 1_000_000_000) / u128::from(frequency)) as u32,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Opened,
    Closed,
    Transfer(Bytes),
}

enum Operation {
    Send,
    Receive,
    Open,
    Close,
}

/// Copied native evidence, before process-instance and IPv6-scope resolution.
/// ETW supplies no scope ID: link-local/scoped facts need unique native/table
/// evidence before they can match an existing scoped model endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fact {
    pub at: Duration,
    pub pid: u32,
    pub protocol: Protocol,
    pub local: Endpoint,
    pub remote: Endpoint,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessKind {
    Started,
    Ended,
    Existing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessFact {
    pub at: Duration,
    pub pid: u32,
    pub key: Option<u64>,
    pub kind: ProcessKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Record {
    Network(Fact),
    Process(ProcessFact),
}

impl Record {
    pub fn at(self) -> Duration {
        match self {
            Self::Network(fact) => fact.at,
            Self::Process(fact) => fact.at,
        }
    }
}

fn decode_process(
    version: u8,
    opcode: u8,
    flags: u16,
    ticks: i64,
    bytes: &[u8],
    clock: Clock,
) -> Result<Option<ProcessFact>, ()> {
    let kind = match opcode {
        1 => ProcessKind::Started,
        2 => ProcessKind::Ended,
        3 => ProcessKind::Existing,
        // DCEnd is shutdown rundown, not an actual process exit.
        _ => return Ok(None),
    };
    let width = match u32::from(flags)
        & (EVENT_HEADER_FLAG_32_BIT_HEADER | EVENT_HEADER_FLAG_64_BIT_HEADER)
    {
        EVENT_HEADER_FLAG_32_BIT_HEADER => 4,
        EVENT_HEADER_FLAG_64_BIT_HEADER => 8,
        _ => return Err(()),
    };
    // Versions 2–5 share this verified prefix: a process-object key followed
    // by the payload PID. Header PID can identify the parent/logger instead.
    // This key is neither a permanent identity nor the census sequence number.
    if !(2..=5).contains(&version) || bytes.len() < width + 4 {
        return Err(());
    }
    let key = if width == 8 {
        u64::from_le_bytes(bytes[..8].try_into().unwrap())
    } else {
        u64::from(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
    };
    Ok(Some(ProcessFact {
        at: clock.at(ticks).ok_or(())?,
        pid: u32::from_le_bytes(bytes[width..width + 4].try_into().unwrap()),
        key: (key != 0).then_some(key),
        kind,
    }))
}

fn decode(
    provider: GUID,
    version: u8,
    opcode: u8,
    ticks: i64,
    bytes: &[u8],
    clock: Clock,
) -> Result<Option<Fact>, ()> {
    let protocol = if provider == TCP {
        Protocol::Tcp
    } else if provider == UDP {
        Protocol::Udp
    } else {
        return Ok(None);
    };
    let operation = match (protocol, opcode) {
        (_, 10 | 26) => Operation::Send,
        (_, 11 | 27) => Operation::Receive,
        (Protocol::Tcp, 12 | 28 | 15 | 31) => Operation::Open,
        (Protocol::Tcp, 13 | 29) => Operation::Close,
        // Retransmissions, copies and unrelated lifecycle records are not
        // additional independent application transfers.
        _ => return Ok(None),
    };
    let ipv6 = opcode >= 26;
    if version != 2 || bytes.len() < if ipv6 { 44 } else { 20 } {
        return Err(());
    }
    let at = clock.at(ticks).ok_or(())?;
    let pid = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as u64;
    let (destination, source) = if ipv6 {
        let destination = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[8..24]).unwrap()),
            u16::from_be_bytes(bytes[40..42].try_into().unwrap()),
            0,
            0,
        ));
        let source = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[24..40]).unwrap()),
            u16::from_be_bytes(bytes[42..44].try_into().unwrap()),
            0,
            0,
        ));
        (Endpoint::new(destination), Endpoint::new(source))
    } else {
        let destination = SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(<[u8; 4]>::try_from(&bytes[8..12]).unwrap()),
            u16::from_be_bytes(bytes[16..18].try_into().unwrap()),
        ));
        let source = SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(<[u8; 4]>::try_from(&bytes[12..16]).unwrap()),
            u16::from_be_bytes(bytes[18..20].try_into().unwrap()),
        ));
        (Endpoint::new(destination), Endpoint::new(source))
    };
    let (local, remote) = if protocol == Protocol::Udp && matches!(operation, Operation::Receive) {
        (destination, source)
    } else {
        (source, destination)
    };
    let kind = match operation {
        Operation::Send => Kind::Transfer(Bytes {
            received: 0,
            sent: size,
        }),
        Operation::Receive => Kind::Transfer(Bytes {
            received: size,
            sent: 0,
        }),
        Operation::Open => Kind::Opened,
        Operation::Close => Kind::Closed,
    };
    Ok(Some(Fact {
        at,
        pid,
        protocol,
        local,
        remote,
        kind,
    }))
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Statistics {
    pub buffers: u32,
    pub buffer_kib: u32,
    pub events_lost: u32,
    pub buffers_lost: u32,
}

// The Vec provides EVENT_TRACE_PROPERTIES alignment and room for its trailing
// UTF-16 name. Native pointers never escape the owning call.
struct Properties(Vec<u64>);

impl Properties {
    fn new(name: &[u16], guid: GUID) -> Self {
        let length = size_of::<EVENT_TRACE_PROPERTIES>() + size_of_val(name);
        let mut value = Self(vec![0; length.div_ceil(size_of::<u64>())]);
        let properties = value.get();
        properties.Wnode.BufferSize = length as u32;
        properties.Wnode.Guid = guid;
        properties.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        properties.Wnode.ClientContext = 1;
        properties.BufferSize = 16;
        properties.MinimumBuffers = 2;
        properties.MaximumBuffers = 64;
        properties.FlushTimer = 1;
        properties.LogFileMode = EVENT_TRACE_SYSTEM_LOGGER_MODE | EVENT_TRACE_REAL_TIME_MODE;
        properties.EnableFlags = EVENT_TRACE_FLAG_NETWORK_TCPIP
            | EVENT_TRACE_FLAG_PROCESS
            | EVENT_TRACE_FLAG_NO_SYSCONFIG;
        properties.LoggerNameOffset = size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        // SAFETY: The initialized allocation is aligned and contains sufficient
        // trailing room for every UTF-16 unit, including its null terminator.
        unsafe {
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                value
                    .0
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(size_of::<EVENT_TRACE_PROPERTIES>())
                    .cast::<u16>(),
                name.len(),
            )
        };
        value
    }

    fn get(&mut self) -> &mut EVENT_TRACE_PROPERTIES {
        // SAFETY: u64 storage has the native structure's alignment and size.
        // All bytes start initialized; native fields admit their zero values.
        unsafe { &mut *self.0.as_mut_ptr().cast() }
    }

    fn statistics(&mut self) -> Statistics {
        let value = self.get();
        Statistics {
            buffers: value.NumberOfBuffers,
            buffer_kib: value.BufferSize,
            events_lost: value.EventsLost,
            buffers_lost: value
                .RealTimeBuffersLost
                .saturating_add(value.LogBuffersLost),
        }
    }
}

/// Worker-owned controller. It never adopts, enumerates or stops other sessions.
pub(crate) struct Session {
    handle: Option<CONTROLTRACE_HANDLE>,
    name: Vec<u16>,
    properties: Properties,
}

impl Session {
    #[cfg(test)]
    pub(crate) fn trace_name(&self) -> String {
        String::from_utf16_lossy(&self.name[..self.name.len() - 1])
    }
    pub(crate) fn start() -> io::Result<Self> {
        let guid = GUID::new().map_err(|error| io::Error::other(error.to_string()))?;
        let name: Vec<_> = format!("NetMap-{}-{guid:?}", std::process::id())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut properties = Properties::new(&name, guid);
        let mut handle = CONTROLTRACE_HANDLE::default();
        // SAFETY: Name and properties remain live throughout this synchronous
        // call. Ownership begins only on successful creation of this fresh name.
        let status = unsafe { StartTraceW(&mut handle, PCWSTR(name.as_ptr()), properties.get()) };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.0 as i32));
        }
        Ok(Self {
            handle: Some(handle),
            name,
            properties,
        })
    }

    pub(crate) fn consumer(
        &self,
        clock: Clock,
    ) -> io::Result<(Consumer, Receiver<Record>, Arc<Control>)> {
        if self.handle.is_none() {
            return Err(io::Error::other("Trace session has stopped"));
        }
        let (sender, receiver) = mpsc::sync_channel(QUEUED_EVENTS);
        let control = Arc::new(Control {
            handle: AtomicU64::new(INVALID_TRACE),
            dropped: AtomicU64::new(0),
            malformed: AtomicU64::new(0),
        });
        let mut context = Box::new(Context {
            clock,
            sender,
            control: Arc::clone(&control),
        });
        let mut name = self.name.clone();
        let mut logfile = Box::new(EVENT_TRACE_LOGFILEW {
            LoggerName: PWSTR(name.as_mut_ptr()),
            Context: (&mut *context as *mut Context).cast(),
            ..Default::default()
        });
        logfile.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME
            | PROCESS_TRACE_MODE_EVENT_RECORD
            | PROCESS_TRACE_MODE_RAW_TIMESTAMP;
        logfile.Anonymous2.EventRecordCallback = Some(on_record);
        // SAFETY: Consumer retains the stable logfile, Context and logger name
        // allocations through ProcessTrace's final return.
        let handle = unsafe { OpenTraceW(&mut *logfile) };
        if handle.Value == INVALID_TRACE {
            // SAFETY: Read immediately after the failing native call.
            return Err(io::Error::from_raw_os_error(
                unsafe { GetLastError() }.0 as i32,
            ));
        }
        control.handle.store(handle.Value, Ordering::Release);
        Ok((
            Consumer {
                handle,
                context,
                _name: name,
                _logfile: logfile,
            },
            receiver,
            control,
        ))
    }

    pub(crate) fn statistics(&mut self) -> io::Result<Statistics> {
        let handle = self
            .handle
            .ok_or_else(|| io::Error::other("Trace session has stopped"))?;
        // SAFETY: Only this worker accesses the owned controller and allocation.
        let status = unsafe {
            ControlTraceW(
                handle,
                PCWSTR::null(),
                self.properties.get(),
                EVENT_TRACE_CONTROL_QUERY,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.0 as i32));
        }
        Ok(self.properties.statistics())
    }

    pub(crate) fn stop(&mut self) -> io::Result<Statistics> {
        if let Some(handle) = self.handle {
            // SAFETY: This handle belongs exclusively to this successful
            // StartTrace call. Callback state is owned separately by Consumer.
            let status = unsafe {
                ControlTraceW(
                    handle,
                    PCWSTR::null(),
                    self.properties.get(),
                    EVENT_TRACE_CONTROL_STOP,
                )
            };
            if status != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
            self.handle = None;
        }
        Ok(self.properties.statistics())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) struct Control {
    handle: AtomicU64,
    dropped: AtomicU64,
    malformed: AtomicU64,
}

impl Control {
    // Off-UI cancellation closes at most once; completion is ProcessTrace's
    // return, not this call or ERROR_CTX_CLOSE_PENDING.
    pub(crate) fn close(&self) -> io::Result<()> {
        let value = self.handle.swap(INVALID_TRACE, Ordering::AcqRel);
        if value == INVALID_TRACE {
            return Ok(());
        }
        // SAFETY: The successful swap uniquely owns this native close. It does
        // not release Consumer's context, even when callbacks are still running.
        let status = unsafe { CloseTrace(PROCESSTRACE_HANDLE { Value: value }) };
        if status == ERROR_SUCCESS || status == ERROR_CTX_CLOSE_PENDING {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status.0 as i32))
        }
    }

    pub(crate) fn losses(&self) -> (u64, u64) {
        (
            self.dropped.load(Ordering::Acquire),
            self.malformed.load(Ordering::Acquire),
        )
    }
}

struct Context {
    clock: Clock,
    sender: SyncSender<Record>,
    control: Arc<Control>,
}

pub(crate) struct Consumer {
    handle: PROCESSTRACE_HANDLE,
    context: Box<Context>,
    _name: Vec<u16>,
    _logfile: Box<EVENT_TRACE_LOGFILEW>,
}

// SAFETY: Moving this sole owner into its consuming worker preserves the Box
// and Vec allocations addressed by the native logfile. No other thread accesses
// that logfile or Context; callbacks use them only while run owns ProcessTrace.
// The separately shared Control contains only atomic counters and a trace handle.
unsafe impl Send for Consumer {}

impl Consumer {
    /// Blocking worker body; Work is the only thread launcher. This owner and
    /// its callback allocation cannot be dropped until the native call returns.
    pub(crate) fn run(self) -> io::Result<()> {
        if self.context.control.handle.load(Ordering::Acquire) == INVALID_TRACE {
            return Ok(());
        }
        // SAFETY: OpenTrace supplied this handle. Self owns its callback inputs
        // throughout consumption, including concurrent asynchronous CloseTrace.
        let status = unsafe { ProcessTrace(&[self.handle], None, None) };
        if status == ERROR_SUCCESS
            || status == ERROR_CANCELLED
            || (status == ERROR_INVALID_HANDLE
                && self.context.control.handle.load(Ordering::Acquire) == INVALID_TRACE)
        {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status.0 as i32))
        }
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        // Either ProcessTrace has returned or consumption never started. This
        // close cannot free callback inputs while ProcessTrace is executing.
        let _ = self.context.control.close();
    }
}

unsafe extern "system" fn on_record(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    // SAFETY: ETW supplies this record and its UserContext during ProcessTrace.
    // Consumer retains the Box until ProcessTrace has fully returned.
    let record = unsafe { &*record };
    if record.UserContext.is_null() {
        return;
    }
    // SAFETY: Only the Context pointer supplied to OpenTrace is used here.
    let context = unsafe { &*record.UserContext.cast::<Context>() };
    if record.EventHeader.ProviderId != TCP
        && record.EventHeader.ProviderId != UDP
        && record.EventHeader.ProviderId != PROCESS
    {
        return;
    }
    let length = record.UserDataLength as usize;
    // SAFETY: ETW owns UserDataLength initialized bytes until this callback
    // returns. decode copies accepted facts; no slice or pointer is retained.
    let bytes = if record.UserData.is_null() {
        &[]
    } else {
        // SAFETY: Nonnull native payload remains live for this callback only.
        unsafe { std::slice::from_raw_parts(record.UserData.cast(), length) }
    };
    let descriptor = record.EventHeader.EventDescriptor;
    let decoded = if record.EventHeader.ProviderId == PROCESS {
        decode_process(
            descriptor.Version,
            descriptor.Opcode,
            record.EventHeader.Flags,
            record.EventHeader.TimeStamp,
            bytes,
            context.clock,
        )
        .map(|fact| fact.map(Record::Process))
    } else {
        decode(
            record.EventHeader.ProviderId,
            descriptor.Version,
            descriptor.Opcode,
            record.EventHeader.TimeStamp,
            bytes,
            context.clock,
        )
        .map(|fact| fact.map(Record::Network))
    };
    match decoded {
        Ok(Some(fact)) => {
            if context.sender.try_send(fact).is_err() {
                context.control.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(None) => (),
        Err(()) => {
            context.control.malformed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests;
