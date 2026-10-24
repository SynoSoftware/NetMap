//! Real Desktop discovery, preference saves and hosting race regression.

use super::*;
use crate::{
    app::{native_checks as facts, settings, work::native_desktop as discovery},
    map::{Command, Direction, viewport::Viewport},
};
use std::time::{Duration, Instant};

const TIMER: usize = 0x4e44;
thread_local! {
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static TICKS: Cell<u32> = const { Cell::new(0) };
}

pub(super) fn requested() -> bool {
    REQUESTED.get()
}

#[test]
#[ignore = "Uses real Explorer discovery and owned-map attachment with isolated preference saves; run alone with a process watchdog"]
fn native_desktop_disable_and_recovery_preserve_current_intent() -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(false);
            TICKS.set(0);
        }
    }
    REQUESTED.set(true);
    let _scope = Scope;
    facts::run()
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    // Reparent the owned map back before destroying the main HWND, including on
    // failure. No Explorer window is destroyed and no worker is joined here.
    host.open_map();
    // SAFETY: All fixture gates have dropped before cleanup of our timer/HWND.
    let cleanup = unsafe {
        let _ = KillTimer(Some(host.window.get()), TIMER);
        DestroyWindow(host.window.get())
    };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    // SAFETY: Only the owned main HWND is shown. A callback-free timer keeps the
    // ordinary message pump bounded while discovery workers wait at their gate.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
        if SetTimer(Some(host.window.get()), TIMER, 20, None) == 0 {
            return Err(Error::from_thread());
        }
    }
    let snapshot = facts::facts(&host.app, facts::observations(), 100, 1, None);
    let _model = facts::scene(&host.app, &snapshot, false);
    pump(host)?;
    for _ in 0..32 {
        if host.app.map.viewport().zoom == 128.0 {
            break;
        }
        command(host, ids::IDM_ZOOM_IN)?;
    }
    host.navigate(Command::Pan(Direction::Right));
    let viewport = host.app.map.viewport();
    require(
        viewport.zoom == 128.0 && !viewport.fitted(),
        "Desktop continuity requires a real 128x non-fitted view",
    )?;
    let map = host.map_window.get();
    normal(host, map, viewport)?;

    let disabled_gate = discovery::arm_next().map_err(native_error)?;
    command(host, ids::IDM_DESKTOP)?;
    let disabled = ready(host, &disabled_gate)?;
    require(
        saved(host, true) && host.background_requested.get(),
        "Enable Desktop did not save and request real discovery",
    )?;
    heartbeat(host)?;
    command(host, ids::IDM_DESKTOP)?;
    wait(
        host,
        "Disabling Desktop did not finish its actual preference save",
        || Ok(saved(host, false) && !host.background_requested.get()),
    )?;
    disabled_gate.release().map_err(native_error)?;
    wait(
        host,
        "Disabled discovery worker did not finish its ordinary reply",
        || Ok(disabled_gate.passed() && discovery::discovery_finished(&host.app)),
    )?;
    // The worker posts before returning. Drain its actual Endpoint reply and
    // Host refresh before treating an unchanged parent as rejection evidence.
    pump(host)?;
    normal(host, map, viewport)?;
    println!(
        "NETMAP_DESKTOP_CASE {{\"phase\":\"disabled\",\"id\":{},\"monitors\":{},\"map\":{},\"parent\":{},\"zoom\":{},\"ticks\":{}}}",
        disabled.id,
        disabled.monitors,
        map.0 as usize,
        host.window.get().0 as usize,
        viewport.zoom,
        TICKS.get()
    );
    drop(disabled_gate);

    // A new gated worker is positive proof that the disabled reply completed
    // Application's Discovery lifecycle rather than merely failing to attach.
    let obsolete_gate = discovery::arm_next().map_err(native_error)?;
    command(host, ids::IDM_DESKTOP)?;
    let obsolete = ready(host, &obsolete_gate)?;
    require(
        obsolete.id > disabled.id && saved(host, true),
        "Re-enable did not start a fresh actual discovery",
    )?;
    host.recover_desktop();
    require(
        host.background_requested.get(),
        "First recovery lost Desktop intent",
    )?;
    for _ in 0..2 {
        // SAFETY: Deliver only a numeric notification to our owned Host. The
        // handler rediscovers current monitors; no OS topology is changed.
        unsafe {
            SendMessageW(host.window.get(), WM_DISPLAYCHANGE, None, None);
        }
        require(
            host.background_requested.get(),
            "Repeated recovery lost pending Desktop intent",
        )?;
    }
    heartbeat(host)?;
    require(
        !host.on_desktop() && parent(map)? == host.window.get(),
        "Held obsolete discovery attached before delivery",
    )?;
    let current_gate = discovery::arm_next().map_err(native_error)?;
    obsolete_gate.release().map_err(native_error)?;
    let current = ready(host, &current_gate)?;
    require(
        obsolete_gate.passed()
            && current.id > obsolete.id + 1
            && host.background_requested.get()
            && !host.on_desktop()
            && parent(map)? == host.window.get(),
        "Obsolete result published or repeated recovery did not coalesce a fresh job",
    )?;
    continuity(host.app.map.viewport(), viewport)?;
    println!(
        "NETMAP_DESKTOP_CASE {{\"phase\":\"coalesced\",\"obsolete_id\":{},\"current_id\":{},\"recoveries\":3,\"monitors\":{},\"map\":{},\"parent\":{},\"zoom\":{},\"ticks\":{}}}",
        obsolete.id,
        current.id,
        current.monitors,
        map.0 as usize,
        host.window.get().0 as usize,
        viewport.zoom,
        TICKS.get()
    );
    drop(obsolete_gate);
    heartbeat(host)?;
    current_gate.release().map_err(native_error)?;
    wait(host, "Current real Desktop result did not attach", || {
        Ok(current_gate.passed() && host.on_desktop())
    })?;
    let valid = host
        .desktop_attachment
        .borrow()
        .as_ref()
        .is_some_and(desktop::Attached::valid);
    let attached_parent = parent(map)?;
    require(
        valid
            && attached_parent != host.window.get()
            && class(attached_parent)? == "WorkerW"
            && host.map_window.get() == map
            && !host.background_requested.get()
            && host.desktop_monitor.borrow().is_some(),
        "Current result did not establish the real owned-map/Explorer attachment",
    )?;
    continuity(host.app.map.viewport(), viewport)?;
    println!(
        "NETMAP_DESKTOP_CASE {{\"phase\":\"attached\",\"id\":{},\"monitors\":{},\"map\":{},\"parent\":{},\"zoom\":{},\"ticks\":{}}}",
        current.id,
        current.monitors,
        map.0 as usize,
        attached_parent.0 as usize,
        host.app.map.viewport().zoom,
        TICKS.get()
    );
    drop(current_gate);

    command(host, ids::IDM_DESKTOP)?;
    wait(
        host,
        "Final Desktop disable did not restore Window and save preferences",
        || {
            Ok(saved(host, false)
                && !host.on_desktop()
                && !host.background_requested.get()
                && parent(map)? == host.window.get())
        },
    )?;
    normal(host, map, viewport)?;
    heartbeat(host)?;
    println!(
        "NETMAP_DESKTOP_CASE {{\"phase\":\"restored\",\"desktop\":false,\"map\":{},\"parent\":{},\"zoom\":{},\"ticks\":{}}}",
        map.0 as usize,
        host.window.get().0 as usize,
        host.app.map.viewport().zoom,
        TICKS.get()
    );
    Ok(())
}

fn ready(host: &Host, gate: &discovery::Gate) -> Result<discovery::Ready> {
    let mut result = None;
    wait(
        host,
        "Real Desktop discovery did not reach its bounded reply gate",
        || {
            if let Some(ready) = gate.ready() {
                result = Some(ready);
            }
            Ok(result.is_some())
        },
    )?;
    let ready = result.ok_or_else(|| failure("Discovery readiness missing"))?;
    if let Some(error) = &ready.error {
        return Err(failure(&format!(
            "Actual Explorer discovery unavailable; acceptance incomplete: {error}"
        )));
    }
    require(
        ready.id != 0 && ready.monitors > 0,
        "Actual discovery did not include connected monitors",
    )?;
    Ok(ready)
}

fn saved(host: &Host, desktop: bool) -> bool {
    let preferences = host.app.preferences();
    matches!(preferences.phase, settings::Phase::Ready)
        && preferences.error.is_none()
        && preferences.committed.desktop == desktop
}

fn normal(host: &Host, map: HWND, viewport: Viewport) -> Result<()> {
    // SAFETY: Query only owned HWNDs. Explorer is never mutated or controlled.
    let visible = unsafe { IsWindowVisible(host.window.get()).as_bool() };
    require(
        host.map_window.get() == map
            && parent(map)? == host.window.get()
            && !host.on_desktop()
            && !host.background_requested.get()
            && visible,
        "Disabled Desktop did not preserve the ordinary owned map/window",
    )?;
    continuity(host.app.map.viewport(), viewport)
}

fn continuity(current: Viewport, previous: Viewport) -> Result<()> {
    let current_centre = current.geographic_center();
    let previous_centre = previous.geographic_center();
    require(
        current.zoom == previous.zoom
            && current.meridian == previous.meridian
            && (current_centre.latitude - previous_centre.latitude).abs() < 0.0001
            && (current_centre.longitude - previous_centre.longitude).abs() < 0.0001,
        "Desktop transition reset the high-zoom geographic view",
    )
}

fn parent(map: HWND) -> Result<HWND> {
    // SAFETY: Map is the same live HWND owned by this fixture's UI thread.
    unsafe { GetParent(map) }
}

fn class(window: HWND) -> Result<String> {
    let mut buffer = [0u16; 256];
    // SAFETY: Non-messaging metadata query of the discovered parent HWND.
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    require(length > 0, "Attached parent class unavailable")?;
    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}

fn command(host: &Host, id: u16) -> Result<()> {
    // SAFETY: Numeric command through the actual owned Host menu routing path.
    unsafe {
        SendMessageW(
            host.window.get(),
            WM_COMMAND,
            Some(WPARAM(id as usize)),
            None,
        );
    }
    pump(host)
}

fn heartbeat(host: &Host) -> Result<()> {
    let before = TICKS.get();
    wait(
        host,
        "Native UI timer did not dispatch while discovery was held",
        || Ok(TICKS.get() > before),
    )
}

fn wait(host: &Host, reason: &str, mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        pump(host)?;
        if ready()? {
            return Ok(());
        }
        require(Instant::now() < deadline, reason)?;
        let mut message = MSG::default();
        // SAFETY: The owned timer bounds this ordinary UI message wait.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(failure("Owned Host retired before Desktop acceptance"));
            }
        }
        dispatch(host, &message);
    }
}

fn pump(host: &Host) -> Result<()> {
    for _ in 0..512 {
        let mut message = MSG::default();
        // SAFETY: Drain this UI thread only, preserving its normal quit signal.
        unsafe {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                return Ok(());
            }
            if message.message == WM_QUIT {
                PostQuitMessage(message.wParam.0 as i32);
                return Err(failure("Host retired during Desktop race acceptance"));
            }
        }
        dispatch(host, &message);
    }
    Err(failure(
        "Desktop fixture queue did not quiesce within its bound",
    ))
}

fn dispatch(host: &Host, message: &MSG) {
    if message.hwnd == host.window.get() && message.message == WM_TIMER && message.wParam.0 == TIMER
    {
        TICKS.set(TICKS.get() + 1);
    }
    // SAFETY: Production navigation/dialog dispatch order on the owning thread.
    unsafe {
        if !host.navigation_key(message) && !IsDialogMessageW(host.window.get(), message).as_bool()
        {
            let _ = TranslateMessage(message);
            DispatchMessageW(message);
        }
    }
}

fn native_error(error: std::io::Error) -> Error {
    failure(&error.to_string())
}
fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
