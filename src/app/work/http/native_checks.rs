//! Owned plain-loopback proof of the production asynchronous Request lifetime.

use super::*;
use crate::app::work::native_shutdown::{Kind, record};
use std::time::Instant;

pub(crate) struct Report {
    pub(crate) bytes: usize,
    pub(crate) context_reclaimed: bool,
}

pub(crate) fn pending_body_read(
    port: u16,
    cancelled: &AtomicBool,
    pending: impl FnOnce(usize),
) -> Result<Report, Error> {
    // SAFETY: This is a registered worker's owned ephemeral loopback server.
    // Only setup is test-specific; production Request callbacks/wait/drop own
    // all asynchronous reads, cancellation, context and buffer lifetime.
    let (session, connection, request) = unsafe {
        let session = Handle::new(WinHttpOpen(
            w!("NetMap native cancellation check"),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            WINHTTP_FLAG_ASYNC,
        ))?;
        WinHttpSetTimeouts(session.0, 2000, 2000, 2000, 30000)?;
        let connection = Handle::new(WinHttpConnect(session.0, w!("127.0.0.1"), port, 0))?;
        let handle = Handle::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            w!("/withheld"),
            PCWSTR::null(),
            PCWSTR::null(),
            ptr::null(),
            WINHTTP_OPEN_REQUEST_FLAGS(0),
        ))?;
        let request = Request::new(handle)?;
        WinHttpSendRequest(
            request.handle(),
            None,
            None,
            0,
            0,
            Arc::as_ptr(&request.context) as usize,
        )?;
        (session, connection, request)
    };
    require(
        matches!(request.wait(cancelled)?, Event::Sent),
        "Unexpected loopback send completion",
    )?;
    // SAFETY: Send completed on this same worker; Request owns the callback state.
    unsafe {
        WinHttpReceiveResponse(request.handle(), ptr::null_mut())?;
    }
    require(
        matches!(request.wait(cancelled)?, Event::Headers),
        "Unexpected loopback headers completion",
    )?;
    require(
        request.number_header(WINHTTP_QUERY_STATUS_CODE)? == Some(200)
            && request.number_header(WINHTTP_QUERY_CONTENT_LENGTH)? == Some(131072),
        "Owned server headers did not match the bounded proof",
    )?;

    let address = request.buffer.as_ref().unwrap().get().cast::<u8>() as usize;
    let mut received = 0;
    while received < BUFFER_SIZE {
        let requested = BUFFER_SIZE - received;
        // SAFETY: Exactly one read owns this stable allocation. Never inspect
        // the bytes until the production completion channel reports Read.
        unsafe {
            WinHttpReadData(
                request.handle(),
                address as *mut c_void,
                requested as u32,
                ptr::null_mut(),
            )?;
        }
        let Event::Read(count) = request.wait(cancelled)? else {
            return Err(io::Error::other("Unexpected body-prefix completion").into());
        };
        require(
            count > 0 && count <= requested,
            "Owned body prefix ended early or exceeded its requested buffer",
        )?;
        // SAFETY: The read completed; count is bounded by the stable allocation.
        let bytes = unsafe { std::slice::from_raw_parts(address as *const u8, count) };
        require(
            bytes.iter().all(|byte| *byte == 0x5a),
            "Owned body prefix was corrupted",
        )?;
        received += count;
    }
    // SAFETY: The entire sent prefix was consumed. The server holds the socket
    // open and withholds the declared remainder until this Request is dropped.
    unsafe {
        WinHttpReadData(
            request.handle(),
            address as *mut c_void,
            BUFFER_SIZE as u32,
            ptr::null_mut(),
        )?;
    }
    match request.events.recv_timeout(Duration::from_millis(200)) {
        Err(mpsc::RecvTimeoutError::Timeout) => {}
        _ => {
            return Err(io::Error::other(
                "Withheld-body read completed or failed before cancellation",
            )
            .into());
        }
    }
    require(
        request.buffer.as_ref().unwrap().get().cast::<u8>() as usize == address,
        "Outstanding read allocation moved",
    )?;
    record(
        Kind::HttpRead,
        "body_read_pending",
        &format!("\"prefix_bytes\":{received},\"buffer\":{address},\"no_completion_ms\":200"),
    );
    let context = Arc::downgrade(&request.context);
    pending(received);
    require(
        matches!(request.wait(cancelled), Err(Error::Cancelled)),
        "Production Request::wait did not return cancellation",
    )?;
    record(Kind::HttpRead, "request_wait_cancelled", "");
    drop(request);
    record(Kind::HttpRead, "request_drop_returned", "");
    // Closed can be received before the callback drops its own independent Arc.
    // Observe eventual reclamation off-UI rather than requiring immediate zero.
    let deadline = Instant::now() + Duration::from_secs(2);
    while context.upgrade().is_some() {
        require(
            Instant::now() < deadline,
            "WinHTTP callback context did not finish reclamation",
        )?;
        std::thread::sleep(Duration::from_millis(1));
    }
    record(Kind::HttpRead, "callback_context_reclaimed", "");
    drop(connection);
    drop(session);
    Ok(Report {
        bytes: received,
        context_reclaimed: true,
    })
}

fn require(value: bool, reason: &str) -> Result<(), Error> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason).into())
    }
}
