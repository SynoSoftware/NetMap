use std::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt,
    io::{self, Write},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use windows::{
    Win32::Networking::WinHttp::*,
    core::{Error as NativeError, HRESULT, HSTRING, PCWSTR, w},
};

const BUFFER_SIZE: usize = 64 * 1024;

#[cfg(test)]
pub(crate) mod native_checks;

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Status(u32),
    TooLarge,
    Io(io::Error),
    Native(NativeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Download cancelled"),
            Self::Status(status) => write!(f, "Server returned HTTP {status}"),
            Self::TooLarge => f.write_str("Download exceeds the supported file size"),
            Self::Io(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
impl From<NativeError> for Error {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

enum Event {
    Sent,
    Headers,
    Read(usize),
    Failed(u32),
    Closed,
}

struct Context {
    events: mpsc::Sender<Event>,
}

struct Handle(*mut c_void);
impl Handle {
    fn new(value: *mut c_void) -> Result<Self, Error> {
        if value.is_null() {
            Err(NativeError::from_thread().into())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: This registered worker owns the handle and closes it once.
        // Request has already drained its final callback before parent teardown.
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}

struct Request {
    handle: Option<Handle>,
    context: Arc<Context>,
    // Only WinHTTP writes during one outstanding ReadData. The worker reads
    // after completion. The callback never accesses these bytes, and closing
    // retains their stable allocation until native I/O has finished.
    buffer: Option<Box<UnsafeCell<[u8; BUFFER_SIZE]>>>,
    events: mpsc::Receiver<Event>,
}

impl Request {
    fn new(handle: Handle) -> Result<Self, Error> {
        let (sender, events) = mpsc::channel();
        let context = Arc::new(Context { events: sender });
        let address = Arc::as_ptr(&context) as usize;
        // SAFETY: WinHTTP copies the context value. No request is outstanding;
        // the stable Arc stays owned until HANDLE_CLOSING is received.
        unsafe {
            WinHttpSetOption(
                Some(handle.0),
                WINHTTP_OPTION_CONTEXT_VALUE,
                Some(&address.to_ne_bytes()),
            )?;
            let previous = WinHttpSetStatusCallback(
                handle.0,
                Some(callback),
                WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE
                    | WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE
                    | WINHTTP_CALLBACK_STATUS_READ_COMPLETE
                    | WINHTTP_CALLBACK_STATUS_REQUEST_ERROR
                    | WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING,
                0,
            );
            if previous.is_some_and(|callback| callback as usize == usize::MAX) {
                return Err(NativeError::from_thread().into());
            }
        }
        // Reserve one strong reference for native callbacks, released only by
        // HANDLE_CLOSING after its final use of the sender.
        let _ = Arc::into_raw(Arc::clone(&context));
        Ok(Self {
            handle: Some(handle),
            context,
            buffer: Some(Box::new(UnsafeCell::new([0; BUFFER_SIZE]))),
            events,
        })
    }

    fn handle(&self) -> *mut c_void {
        self.handle.as_ref().unwrap().0
    }

    fn wait(&self, cancelled: &AtomicBool) -> Result<Event, Error> {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::Cancelled);
            }
            match self.events.recv_timeout(Duration::from_millis(100)) {
                Ok(Event::Failed(error)) => {
                    return Err(NativeError::from_hresult(HRESULT::from_win32(error)).into());
                }
                Ok(event) => return Ok(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("HTTP callback channel closed").into());
                }
            }
        }
    }

    fn number_header(&self, header: u32) -> Result<Option<u32>, Error> {
        let mut number = 0u32;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: Header queries are synchronous after Headers, with no other
        // operation outstanding. The writable number has the advertised size.
        let result = unsafe {
            WinHttpQueryHeaders(
                self.handle(),
                header | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut number as *mut u32).cast()),
                &mut size,
                ptr::null_mut(),
            )
        };
        match result {
            Ok(()) => Ok(Some(number)),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_WINHTTP_HEADER_NOT_FOUND) => {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        // SAFETY: All calls, including close, happen on the same worker. Async
        // close cancels outstanding I/O; context and buffer remain alive here.
        let closed = unsafe { WinHttpCloseHandle(handle.0) };
        std::mem::forget(handle);
        if closed.is_err() {
            // Keep callback state valid if Windows cannot confirm handle closure.
            // Parent/session cleanup may still finish it; never free it early.
            std::mem::forget(self.buffer.take());
            return;
        }
        // This off-UI wait is covered by Work's process-finalization deadline.
        // HANDLE_CLOSING is the last notification for this request, including
        // cancellation/error paths and callbacks that ran inline with a call.
        while let Ok(event) = self.events.recv() {
            if matches!(event, Event::Closed) {
                break;
            }
        }
    }
}

unsafe extern "system" fn callback(
    _handle: *mut c_void,
    context: usize,
    status: u32,
    info: *mut c_void,
    size: u32,
) {
    if context == 0 {
        return;
    }
    let pointer = context as *const Context;
    // SAFETY: The reserved native Arc exists until HANDLE_CLOSING. Each callback
    // takes its own strong reference, so a worker receiving Closed cannot free
    // the sender before this callback has returned from send.
    unsafe {
        Arc::increment_strong_count(pointer);
    }
    // SAFETY: Consume only the reference just incremented above.
    let context = unsafe { Arc::from_raw(pointer) };
    let event = match status {
        WINHTTP_CALLBACK_STATUS_SENDREQUEST_COMPLETE => Event::Sent,
        WINHTTP_CALLBACK_STATUS_HEADERS_AVAILABLE => Event::Headers,
        WINHTTP_CALLBACK_STATUS_READ_COMPLETE => Event::Read(size as usize),
        WINHTTP_CALLBACK_STATUS_REQUEST_ERROR
            if !info.is_null() && size as usize >= size_of::<WINHTTP_ASYNC_RESULT>() =>
        {
            // SAFETY: WinHTTP documents this callback's pointer and length as
            // WINHTTP_ASYNC_RESULT. Copy its value before returning to Windows.
            Event::Failed(unsafe { (*info.cast::<WINHTTP_ASYNC_RESULT>()).dwError })
        }
        WINHTTP_CALLBACK_STATUS_HANDLE_CLOSING => Event::Closed,
        _ => return,
    };
    // One worker issues at most one operation at a time. Only completion/error
    // and final close events enter this channel; body data is never queued.
    let closing = matches!(event, Event::Closed);
    let _ = context.events.send(event);
    if closing {
        // SAFETY: HANDLE_CLOSING occurs once; release the native reservation
        // after its final use. This callback still owns its independent Arc.
        unsafe {
            drop(Arc::from_raw(pointer));
        }
    }
}

/// HTTPS streaming for registered workers. Neither callbacks nor the UI perform
/// disk I/O or issue subsequent HTTP operations. Cancellation closes this async
/// request on the worker and retains its inputs until callbacks finish.
pub fn download(
    host: &str,
    path: &str,
    output: &mut impl Write,
    maximum: u64,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<(), Error> {
    if cancelled.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    // SAFETY: All handles are worker-local and nested in session/connection/
    // request order. HTTPS uses Windows certificate/proxy policy without bypasses.
    unsafe {
        let session = Handle::new(WinHttpOpen(
            w!("NetMap/0.1"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            WINHTTP_FLAG_ASYNC,
        ))?;
        WinHttpSetTimeouts(session.0, 10_000, 15_000, 30_000, 30_000)?;
        let connection = Handle::new(WinHttpConnect(session.0, &HSTRING::from(host), 443, 0))?;
        let handle = Handle::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            &HSTRING::from(path),
            PCWSTR::null(),
            PCWSTR::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        WinHttpSetOption(
            Some(handle.0),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_ne_bytes()),
        )?;
        let request = Request::new(handle)?;
        let buffer = request.buffer.as_ref().unwrap();
        // In async WinHTTP, TRUE accepts the operation; completion still arrives
        // through its callback. FALSE is an immediate failure, not IO_PENDING.
        WinHttpSendRequest(
            request.handle(),
            None,
            None,
            0,
            0,
            Arc::as_ptr(&request.context) as usize,
        )?;
        if !matches!(request.wait(cancelled)?, Event::Sent) {
            return Err(io::Error::other("Unexpected HTTP send completion").into());
        }
        WinHttpReceiveResponse(request.handle(), ptr::null_mut())?;
        if !matches!(request.wait(cancelled)?, Event::Headers) {
            return Err(io::Error::other("Unexpected HTTP headers completion").into());
        }
        let status = request
            .number_header(WINHTTP_QUERY_STATUS_CODE)?
            .unwrap_or(0);
        if status != 200 {
            return Err(Error::Status(status));
        }
        let total = request
            .number_header(WINHTTP_QUERY_CONTENT_LENGTH)?
            .map(u64::from);
        if total.is_some_and(|size| size > maximum) {
            return Err(Error::TooLarge);
        }
        let mut received = 0u64;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::Cancelled);
            }
            WinHttpReadData(
                request.handle(),
                buffer.get().cast(),
                BUFFER_SIZE as u32,
                ptr::null_mut(),
            )?;
            let Event::Read(count) = request.wait(cancelled)? else {
                return Err(io::Error::other("Unexpected HTTP read completion").into());
            };
            if count == 0 {
                break;
            }
            if count > BUFFER_SIZE || count as u64 > maximum.saturating_sub(received) {
                return Err(Error::TooLarge);
            }
            // Read completion ends native access to the buffer until the next
            // explicit ReadData. The write finishes before that next call.
            let chunk = std::slice::from_raw_parts(buffer.get().cast::<u8>(), count);
            output.write_all(chunk)?;
            received += count as u64;
            progress(received, total);
        }
        if total.is_some_and(|size| size != received) {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete download").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_download_does_not_open_a_request_or_write_output() {
        let cancelled = AtomicBool::new(true);
        let mut output = Vec::new();
        assert!(matches!(
            download(
                "invalid.invalid",
                "/",
                &mut output,
                128,
                &cancelled,
                |_, _| {}
            ),
            Err(Error::Cancelled)
        ));
        assert!(output.is_empty());
    }

    #[test]
    #[ignore = "Uses the documented public-address HTTPS endpoint"]
    fn native_https_completes_callbacks_before_releasing_request_state() {
        let cancelled = AtomicBool::new(false);
        let mut output = Vec::new();
        download(
            "api64.ipify.org",
            "/",
            &mut output,
            128,
            &cancelled,
            |_, _| {},
        )
        .unwrap();
        assert!(
            std::str::from_utf8(&output)
                .unwrap()
                .trim()
                .parse::<std::net::IpAddr>()
                .is_ok()
        );
    }

    #[test]
    #[ignore = "Starts a DB-IP HTTPS transfer and cancels after the first chunk"]
    fn cancellation_between_chunks_retains_inputs_until_handle_closing() {
        let cancelled = AtomicBool::new(false);
        let mut output = Vec::new();
        let result = download(
            "download.db-ip.com",
            "/free/dbip-country-lite-2026-10.mmdb.gz",
            &mut output,
            16 * 1024 * 1024,
            &cancelled,
            |_, _| cancelled.store(true, Ordering::Release),
        );
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!output.is_empty() && output.len() <= BUFFER_SIZE);
    }
}
