//! Opt-in checks of the real Host close path and registered Work finalization.

use super::*;
use crate::app::{
    native_checks as facts,
    work::native_shutdown::{self, Kind, Ready},
};
use std::time::{Duration, Instant};

const TIMER: usize = 0x4e53;
thread_local! { static REQUESTED: Cell<Option<Kind>> = const { Cell::new(None) }; }

pub(super) fn requested() -> bool {
    REQUESTED.get().is_some()
}

struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        REQUESTED.set(None);
    }
}

#[test]
#[ignore = "Creates owned native windows and loopback HTTP; run alone with a process watchdog"]
fn native_close_cancels_an_outstanding_http_body_read() -> Result<()> {
    REQUESTED.set(Some(Kind::HttpRead));
    let _scope = Scope;
    facts::run()?;
    native_shutdown::verify_http_completed().map_err(|error| failure(&error.to_string()))
}

#[test]
#[ignore = "The real five-second fallback terminates this test process with exit 1; use the shutdown wrapper"]
fn native_close_initiates_five_second_fallback_for_uncooperative_work() -> Result<()> {
    REQUESTED.set(Some(Kind::Uncooperative));
    let _scope = Scope;
    facts::run()?;
    Err(failure(
        "Uncooperative registered work did not invoke the real process fallback",
    ))
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let window = host.window.get();
    let result = journey(host, window);
    // SAFETY: Only this fixture's timer/window are touched, on their UI thread.
    // A successful real WM_CLOSE has already destroyed both; never destroy null.
    unsafe {
        if IsWindow(Some(window)).as_bool() {
            let _ = KillTimer(Some(window), TIMER);
            let cleanup = DestroyWindow(window);
            return result.and(cleanup);
        }
    }
    result
}

fn journey(host: &Host, window: HWND) -> Result<()> {
    let kind = REQUESTED
        .get()
        .ok_or_else(|| failure("Shutdown check was not requested"))?;
    // SAFETY: Owned normal Host only. A callback-free timer bounds the ordinary
    // UI wait while readiness is polled; no worker accesses a window or UI state.
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNORMAL);
        if SetTimer(Some(window), TIMER, 20, None) == 0 {
            return Err(Error::from_thread());
        }
    }
    let check =
        native_shutdown::start(&host.app, kind).map_err(|error| failure(&error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(ready) = check.ready().map_err(|error| failure(&error.to_string()))? {
            let expected = matches!(
                (kind, ready),
                (Kind::HttpRead, Ready::PendingRead { bytes: 65536 })
                    | (Kind::Uncooperative, Ready::Uncooperative)
            );
            require(
                expected,
                "Registered worker reported the wrong shutdown readiness",
            )?;
            break;
        }
        require(
            Instant::now() < deadline,
            "Registered shutdown job did not become ready",
        )?;
        next_message(host, false)?;
    }

    let close_tick = native_shutdown::tick();
    native_shutdown::record(
        kind,
        "close_requested",
        &format!("\"window\":{}", window.0 as usize),
    );
    // SAFETY: Numeric close to this check's exact owned main HWND. Production
    // WM_CLOSE performs cancellation and destruction; this fixture does neither.
    unsafe {
        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: Query only the exact HWND owned before the close request.
        let alive = unsafe { IsWindow(Some(window)).as_bool() };
        if !alive {
            require(
                host.window.get().is_invalid(),
                "Native window died without Host detachment",
            )?;
            native_shutdown::record(
                kind,
                "window_removed",
                &format!(
                    "\"window\":{},\"close_elapsed_ms\":{}",
                    window.0 as usize,
                    native_shutdown::tick().saturating_sub(close_tick)
                ),
            );
            return Ok(());
        }
        require(
            Instant::now() < deadline,
            "WM_CLOSE did not remove the owned Host HWND",
        )?;
        next_message(host, true)?;
    }
}

fn next_message(host: &Host, closing: bool) -> Result<()> {
    let mut message = MSG::default();
    // SAFETY: Ordinary UI-thread message wait, bounded by the owned timer until
    // WM_CLOSE destroys it. Preserve WM_QUIT for the outer production Host loop.
    unsafe {
        let result = GetMessageW(&mut message, None, 0, 0).0;
        if result == 0 {
            PostQuitMessage(message.wParam.0 as i32);
            return require(
                closing,
                "Host retired before the pending operation was ready",
            );
        }
        if result < 0 {
            return Err(Error::from_thread());
        }
        if !host.navigation_key(&message)
            && !IsDialogMessageW(host.window.get(), &message).as_bool()
        {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
