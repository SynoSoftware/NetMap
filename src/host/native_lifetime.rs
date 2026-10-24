//! Application-lived Work delivery across owned native presentation replacement.

use super::*;
use crate::app::{
    native_checks as facts,
    work::{self, native_checks as delivery},
};
use crate::{connections::ObservationId, geolocation::Availability};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Graphics::Gdi::UpdateWindow,
        UI::Controls::{LVIF_TEXT, LVITEMW, LVM_GETITEMCOUNT, LVM_GETITEMW},
    },
    core::PWSTR,
};

const TIMER: usize = 0x4e4c;
#[derive(Clone, Copy)]
enum Case {
    Replies,
    Locations,
}
thread_local! { static REQUESTED: Cell<Option<Case>> = const { Cell::new(None) }; }

pub(super) fn requested() -> bool {
    REQUESTED.get().is_some()
}

#[test]
#[ignore = "Creates owned native windows and a bounded registered Work barrier; run alone with a process watchdog"]
fn native_worker_replies_survive_target_gap_and_replacement() -> Result<()> {
    run(Case::Replies)
}

#[test]
#[ignore = "Loads an isolated real city MMDB on registered Work; creates only owned native windows; requires a process watchdog"]
fn native_local_locations_and_origin_survive_target_gap_and_replacement() -> Result<()> {
    run(Case::Locations)
}

fn run(case: Case) -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(None);
        }
    }
    REQUESTED.set(Some(case));
    let _scope = Scope;
    let _local_origin = matches!(case, Case::Locations).then(delivery::restrict_origin_to_local);
    facts::run()
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = match REQUESTED.get() {
        Some(Case::Replies) => journey(host),
        Some(Case::Locations) => locations_journey(host),
        None => Err(failure("Native lifetime case was not requested")),
    };
    // SAFETY: The journey has returned outside FFI, dropping its barrier sender.
    // Retire only the fixture's timer and main HWND on their creating thread.
    let cleanup = unsafe {
        let _ = KillTimer(Some(host.window.get()), TIMER);
        DestroyWindow(host.window.get())
    };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    start(host)?;
    let app = &host.app;
    let snapshot = facts::facts(app, facts::observations(), 100, 1, None);
    let baseline = facts::scene(app, &snapshot, false);
    pump(host)?;
    let mut appearance = app.map.appearance();
    appearance.animations = false;
    app.map.set_appearance(appearance);
    host.navigate(crate::map::Command::ZoomIn);
    repaint(host.map_window.get())?;
    let viewport = app.map.viewport();
    require(
        !viewport.fitted(),
        "Viewport preservation requires a non-fitted baseline",
    )?;
    let baseline_frames = app.map.observe_presentations()?;
    repaint(host.map_window.get())?;
    require(
        baseline_frames
            .latest
            .get()
            .is_some_and(|frame| frame.model == Arc::as_ptr(&baseline) as usize),
        "Baseline native Present did not use the accepted model",
    )?;

    // Build both payloads with the existing fixture's authoritative scene/Rows
    // preparation. Only delivery is delayed; acceptance stays in Application.
    let (_, stale) = facts::prepared_scene(app, &snapshot, false);
    let stale_id = scene_id(&stale)?;
    let mut observations = facts::observations();
    observations.retain(|observation| observation.id.0 != 10);
    let previous = app.exploration_rows();
    let current_snapshot = facts::facts(app, observations, 200, 1, previous.as_deref());
    let (model, current) = facts::prepared_scene(app, &current_snapshot, true);
    let (current_id, rows, sampled) = match &current {
        work::Reply::Scene {
            id, rows, sampled, ..
        } => (*id, Arc::clone(rows), *sampled),
        _ => return Err(failure("Expected a prepared Scene reply")),
    };
    require(
        current_id != stale_id,
        "Controlled replies did not have distinct generations",
    )?;
    pump(host)?;
    let observer = facts::observe_scenes();
    let pending = delivery::delayed_replies(app, current, stale)
        .map_err(|error| failure(&error.to_string()))?;
    let endpoint = pending.endpoint;
    let old_map = remove_map(host)?;
    require(
        host.map_window.get().is_invalid() && app.map.graphics_mode().is_none(),
        "Old map destruction did not detach the native target",
    )?;
    // SAFETY: Query only our application-lived endpoint while map is absent.
    let endpoint_alive = unsafe { IsWindow(Some(pending.endpoint)).as_bool() };
    require(
        endpoint_alive,
        "Work endpoint retired with the map presentation",
    )?;

    deliver(host, &pending, &observer, 1, 0)?;
    require(
        facts::accepted_scene(app) == Some(current_id)
            && app
                .exploration_rows()
                .is_some_and(|accepted| Arc::ptr_eq(&accepted, &rows))
            && app.map.graphics_mode().is_none(),
        "Current worker reply was not accepted while its map target was absent",
    )?;
    deliver(host, &pending, &observer, 1, 1)?;
    let counts = observer.counts();
    require(
        counts.last_accepted == Some(current_id)
            && counts.last_discarded == Some(stale_id)
            && facts::accepted_scene(app) == Some(current_id)
            && app
                .exploration_rows()
                .is_some_and(|accepted| Arc::ptr_eq(&accepted, &rows)),
        "Stale worker reply replaced current accepted scene or rows",
    )?;
    for _ in 0..2 {
        delivery::wake(app);
    }
    pump(host)?;
    require(
        observer.counts() == counts,
        "An empty endpoint wake accepted a reply twice",
    )?;
    drop(pending);

    // Recreate only the map using its existing native class, context and owners.
    // Main Host, application, endpoint, accepted rows and viewport stay alive.
    let replacement = replace_map(host)?;
    // SAFETY: The same application-lived endpoint must survive replacement.
    let endpoint_alive = unsafe { IsWindow(Some(endpoint)).as_bool() };
    require(endpoint_alive, "Map replacement retired the Work endpoint")?;
    require(
        app.map.viewport() == viewport,
        "Map replacement reset the retained viewport",
    )?;
    let presentations = app.map.observe_presentations()?;
    app.set_presentation(Some(host.window.get()));
    // SAFETY: Numeric production refresh to this fixture's live main HWND.
    unsafe {
        PostMessageW(
            Some(host.window.get()),
            REFRESH_MESSAGE,
            WPARAM(1),
            LPARAM(0),
        )?;
    }
    pump(host)?;
    let minimum_age = current_snapshot.interval.end + sampled.elapsed();
    repaint(replacement)?;
    let frame = presentations
        .latest
        .get()
        .ok_or_else(|| failure("Replacement map had no successful Present"))?;
    require(
        presentations.attempts.get() > 0
            && frame.target == replacement
            && frame.model == Arc::as_ptr(&model) as usize
            && frame.at >= minimum_age,
        "Replacement Present did not use the current accepted model and original sample anchor",
    )?;
    let table = host
        .exploration
        .get()
        .ok_or_else(|| failure("Exploration control unavailable"))?;
    // SAFETY: Numeric query to our owned native virtual ListView.
    let row_count = unsafe { SendMessageW(table.window(), LVM_GETITEMCOUNT, None, None) }.0;
    require(
        row_count == current_snapshot.observations.len() as isize,
        "Native table did not resume from current accepted rows",
    )?;
    require(
        observer.counts() == counts,
        "Reattaching a map replayed an accepted worker reply",
    )?;
    println!(
        "NETMAP_LIFETIME_CASE {{\"accepted\":{},\"discarded\":{},\"current_id\":{},\"stale_id\":{},\"endpoint\":{},\"old_map\":{},\"replacement_map\":{},\"rows\":{},\"present_attempts\":{},\"age_ms\":{}}}",
        counts.accepted,
        counts.discarded,
        current_id,
        stale_id,
        endpoint.0 as usize,
        old_map.0 as usize,
        replacement.0 as usize,
        row_count,
        presentations.attempts.get(),
        frame.at.as_millis()
    );
    Ok(())
}

fn locations_journey(host: &Host) -> Result<()> {
    start(host)?;
    let app = &host.app;
    let mut observations = facts::observations();
    for observation in &mut observations {
        observation.introduced = Duration::from_secs(300);
    }
    let snapshot = facts::facts(app, observations, 300, 1, None);
    pump(host)?;
    let baseline = app.map.observe_presentations()?;
    repaint(host.map_window.get())?;
    let baseline_frame = baseline
        .latest
        .get()
        .ok_or_else(|| failure("Baseline Present unavailable"))?;
    let observer = facts::observe_scenes();
    let load = delivery::load_locations(app).map_err(|error| failure(&error.to_string()))?;
    let mut loaded = None;
    wait_for(host, "Local MMDB load did not complete", || {
        if let Some(result) = load.ready() {
            loaded = Some(result.map_err(|error| failure(&error))?);
        }
        Ok(loaded.is_some())
    })?;
    let loaded = loaded.ok_or_else(|| failure("Local MMDB completion missing"))?;
    require(
        loaded.previous.version == 0
            && loaded.current.version == 1
            && matches!(loaded.current.country, Availability::Ready { .. })
            && matches!(loaded.current.city, Availability::Ready { .. })
            && facts::location_report(app).is_none(),
        "Loaded dataset must remain unaccepted behind its gate",
    )?;
    let first = loaded
        .first
        .point
        .ok_or_else(|| failure("First controlled MMDB lookup has no coordinate"))?;
    let second = loaded
        .second
        .point
        .ok_or_else(|| failure("Second controlled MMDB lookup has no coordinate"))?;
    require(
        (first.longitude - second.longitude).abs() > 1.0,
        "Origin transition requires distinct real MMDB longitudes",
    )?;
    let old_map = remove_map(host)?;
    let endpoint = load.delivery.endpoint;
    // SAFETY: Query only the retained application-lived endpoint, not old HWNDs.
    let endpoint_alive = unsafe { IsWindow(Some(endpoint)).as_bool() };
    require(
        endpoint_alive && app.map.graphics_mode().is_none(),
        "Local result lost its live endpoint during target removal",
    )?;
    deliver_ready(host, &load.delivery)?;
    require(
        facts::location_report(app).is_some_and(|report| report.version == loaded.current.version),
        "Actual dataset availability did not reach the application during the gap",
    )?;

    let first_request = facts::seed_local_origin(app, "8.8.8.8".parse().unwrap())?;
    wait_for(
        host,
        "First actual local origin/Scene lookup did not complete",
        || Ok(facts::origin_point(app) == Some(first) && facts::scene_current(app)),
    )?;
    let fitted = app.map.viewport();
    require(
        fitted.fitted() && (f64::from(fitted.meridian) - first.longitude).abs() < 0.0001,
        "Fitted view did not adopt the locally resolved origin meridian",
    )?;

    require(
        app.zoom_inspection(1.0).changed,
        "Non-fitted origin check could not zoom",
    )?;
    app.prepare_scene();
    let before = app.map.viewport();
    let centre = before.geographic_center();
    let second_request = facts::seed_local_origin(app, "1.1.1.1".parse().unwrap())?;
    wait_for(
        host,
        "Superseding actual local origin/Scene lookup did not complete",
        || Ok(facts::origin_point(app) == Some(second) && facts::scene_current(app)),
    )?;
    let viewport = app.map.viewport();
    let after = viewport.geographic_center();
    require(
        !viewport.fitted()
            && viewport.zoom == before.zoom
            && (after.latitude - centre.latitude).abs() < 0.0001
            && (after.longitude - centre.longitude).abs() < 0.0001,
        "Local origin change moved the non-fitted geographic centre",
    )?;
    let accepted = facts::accepted_scene(app);
    let rows = app
        .exploration_rows()
        .ok_or_else(|| failure("Actual Scene supplied no ready rows"))?;
    for (id, location) in [
        (10, &loaded.first),
        (11, &loaded.first),
        (12, &loaded.second),
    ] {
        let place = rows
            .place(ObservationId(id))
            .ok_or_else(|| failure("Actual MMDB row is unlocated"))?;
        require(
            place.cities && place.contains(location) && app.map.has_place(&place),
            "Ready row location and map disagree with the real shared MMDB lookup",
        )?;
    }
    require(
        rows.place(ObservationId(13)).is_none(),
        "Loopback received a geographic place",
    )?;

    // Release the actual pre-load Report only after version 1 is established.
    // Its rejection must not trigger replacement/relookup or regress origin.
    let counts = observer.counts();
    deliver_ready(host, &load.delivery)?;
    require(
        facts::location_report(app).is_some_and(|report| report.version == 1)
            && facts::origin_point(app) == Some(second)
            && facts::accepted_scene(app) == accepted
            && observer.counts() == counts,
        "Old availability report replaced accepted local state",
    )?;
    drop(load);
    // These are late replies after Ready, not a simulation of an outstanding
    // lookup's id/version guards (covered by Origin's focused pure tests).
    let stale = delivery::delayed_replies(
        app,
        work::Reply::OriginLocation {
            id: first_request.id(),
            version: loaded.current.version,
            result: Ok(Some(loaded.first.clone())),
        },
        work::Reply::OriginLocation {
            id: second_request.id(),
            version: loaded.previous.version,
            result: Ok(Some(loaded.first.clone())),
        },
    )
    .map_err(|error| failure(&error.to_string()))?;
    for _ in 0..2 {
        deliver_ready(host, &stale)?;
        require(
            facts::origin_point(app) == Some(second)
                && facts::accepted_scene(app) == accepted
                && observer.counts() == counts
                && app.map.viewport() == viewport,
            "Stale origin identity/version overwrote current local state",
        )?;
    }
    drop(stale);
    require(
        app.map.graphics_mode().is_none(),
        "Local workers recreated a presentation during the gap",
    )?;

    let replacement = replace_map(host)?;
    require(
        app.map.viewport() == viewport,
        "Reattachment changed the resolved non-fitted view",
    )?;
    let presentations = app.map.observe_presentations()?;
    app.set_presentation(Some(host.window.get()));
    // SAFETY: Refresh only the fixture's current owned native presentation.
    unsafe {
        PostMessageW(
            Some(host.window.get()),
            REFRESH_MESSAGE,
            WPARAM(1),
            LPARAM(0),
        )?;
    }
    pump(host)?;
    let (at, sampled) =
        facts::sample_anchor(app).ok_or_else(|| failure("Accepted sample anchor missing"))?;
    let minimum_age = at + sampled.elapsed();
    repaint(replacement)?;
    let frame = presentations
        .latest
        .get()
        .ok_or_else(|| failure("No successful replacement Present"))?;
    require(
        presentations.attempts.get() > 0
            && frame.target == replacement
            && counts.last_accepted_model != 0
            && frame.model == counts.last_accepted_model
            && frame.revision > baseline_frame.revision
            && frame.at >= minimum_age
            && facts::scene_current(app),
        "Replacement did not present current local facts at their actual age",
    )?;
    let table = host
        .exploration
        .get()
        .ok_or_else(|| failure("Native ListView unavailable"))?;
    // SAFETY: Numeric count query to the owned virtual native ListView.
    let row_count = unsafe { SendMessageW(table.window(), LVM_GETITEMCOUNT, None, None) }.0;
    require(
        row_count == snapshot.observations.len() as isize,
        "Native table did not receive current real-location rows",
    )?;
    for index in [0, 2] {
        require(
            location_text(table.window(), index)?.contains("Approximate coordinate"),
            "Native row callback did not expose its actual city coordinate precision",
        )?;
    }
    for _ in 0..2 {
        delivery::wake(app);
    }
    pump(host)?;
    require(
        observer.counts() == counts && facts::accepted_scene(app) == accepted,
        "Empty endpoint wakes replayed accepted local Scene work",
    )?;
    println!(
        "NETMAP_LOCAL_LOCATIONS_CASE {{\"version\":{},\"previous_version\":{},\"origin_requests\":2,\"stale_origin_replies\":2,\"accepted_scenes\":{},\"discarded_scenes\":{},\"scene\":{},\"endpoint\":{},\"old_map\":{},\"replacement_map\":{},\"rows\":{},\"fitted_meridian\":{},\"centre_longitude\":{},\"present_attempts\":{},\"age_ms\":{}}}",
        loaded.current.version,
        loaded.previous.version,
        counts.accepted,
        counts.discarded,
        accepted.unwrap_or_default(),
        endpoint.0 as usize,
        old_map.0 as usize,
        replacement.0 as usize,
        row_count,
        fitted.meridian,
        after.longitude,
        presentations.attempts.get(),
        frame.at.as_millis()
    );
    Ok(())
}

fn location_text(window: HWND, row: i32) -> Result<String> {
    let mut buffer = [0u16; 512];
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row,
        iSubItem: 6,
        pszText: PWSTR(buffer.as_mut_ptr()),
        cchTextMax: buffer.len() as i32,
        ..Default::default()
    };
    // SAFETY: The synchronous native GETDISPINFO call borrows these in-process
    // stack records; both remain valid until ListView finishes copying text.
    let read = unsafe {
        SendMessageW(
            window,
            LVM_GETITEMW,
            None,
            Some(LPARAM(&mut item as *mut _ as isize)),
        )
    }
    .0;
    require(read != 0, "Native location cell callback failed")?;
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| failure("Native location text is unterminated"))?;
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

fn start(host: &Host) -> Result<()> {
    // SAFETY: Owned windows and a callback-free timer bound the ordinary pump.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
        if SetTimer(Some(host.window.get()), TIMER, 20, None) == 0 {
            return Err(Error::from_thread());
        }
    }
    Ok(())
}

fn remove_map(host: &Host) -> Result<HWND> {
    let old_map = host.map_window.get();
    host.app.set_presentation(None);
    // SAFETY: Tooltip subclass references must be retired before the owned map.
    // Real NCDESTROY retires accessibility and detaches the native render target.
    unsafe {
        let tooltip = host.tooltip.replace(HWND::default());
        if !tooltip.is_invalid() {
            DestroyWindow(tooltip)?;
        }
        DestroyWindow(old_map)?;
    }
    Ok(old_map)
}

fn deliver_ready(host: &Host, delivery: &delivery::Delivery) -> Result<()> {
    delivery
        .release()
        .map_err(|error| failure(&error.to_string()))?;
    wait_for(
        host,
        "Typed local result was not enqueued",
        || match delivery.sent() {
            Some(true) => Ok(true),
            Some(false) => Err(failure("Local result worker retired before delivery")),
            None => Ok(false),
        },
    )?;
    // Reporter queues and posts before acknowledging send. Drain after the
    // acknowledgement, so unchanged state proves rejection after real dispatch.
    pump(host)
}

fn wait_for(host: &Host, reason: &str, mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pump(host)?;
        if ready()? {
            return Ok(());
        }
        require(Instant::now() < deadline, reason)?;
        next_message(host)?;
    }
}

fn next_message(host: &Host) -> Result<()> {
    let mut message = MSG::default();
    // SAFETY: The fixture timer bounds this native UI wait. Endpoint messages
    // still run through the ordinary callback and Application acceptance owner.
    unsafe {
        let result = GetMessageW(&mut message, None, 0, 0).0;
        if result <= 0 {
            if result == 0 {
                PostQuitMessage(message.wParam.0 as i32);
            }
            return Err(failure("Host retired while waiting for local work"));
        }
    }
    dispatch(host, &message);
    Ok(())
}

fn scene_id(reply: &work::Reply) -> Result<u64> {
    match reply {
        work::Reply::Scene { id, .. } => Ok(*id),
        _ => Err(failure("Expected Scene reply")),
    }
}

fn deliver(
    host: &Host,
    pending: &delivery::Delivery,
    observer: &facts::SceneObserver,
    accepted: u32,
    discarded: u32,
) -> Result<()> {
    pending
        .release()
        .map_err(|error| failure(&error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut sent = false;
    loop {
        pump(host)?;
        if let Some(delivered) = pending.sent() {
            require(
                delivered,
                "Registered worker did not enqueue its typed reply",
            )?;
            sent = true;
        }
        let counts = observer.counts();
        require(
            counts.accepted <= accepted && counts.discarded <= discarded,
            "Typed worker reply was accepted or discarded more than once",
        )?;
        if sent && counts.accepted == accepted && counts.discarded == discarded {
            return Ok(());
        }
        require(
            Instant::now() < deadline,
            "Typed Work reply was not delivered before its bounded deadline",
        )?;
        let mut message = MSG::default();
        // SAFETY: The fixture timer bounds this ordinary UI message wait. Keep
        // production navigation/dialog order; endpoint dispatch owns acceptance.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(failure("Host retired while waiting for its Work endpoint"));
            }
        }
        dispatch(host, &message);
    }
}

fn pump(host: &Host) -> Result<()> {
    for _ in 0..512 {
        let mut message = MSG::default();
        // SAFETY: Drain only this UI thread's messages, preserving WM_QUIT.
        unsafe {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                return Ok(());
            }
            if message.message == WM_QUIT {
                PostQuitMessage(message.wParam.0 as i32);
                return Err(failure("Host retired during lifetime verification"));
            }
        }
        dispatch(host, &message);
    }
    Err(failure(
        "Native lifetime message queue did not quiesce within its bound",
    ))
}

fn dispatch(host: &Host, message: &MSG) {
    // SAFETY: Same UI thread and production dispatch order, with no global input
    // and no direct calls to the endpoint's callback or reply acceptance owner.
    unsafe {
        if !host.navigation_key(message) && !IsDialogMessageW(host.window.get(), message).as_bool()
        {
            let _ = TranslateMessage(message);
            DispatchMessageW(message);
        }
    }
}

fn replace_map(host: &Host) -> Result<HWND> {
    // SAFETY: Reuse the registered production class and the still-live Host
    // context. Native creation establishes/clears its normal GWLP_USERDATA.
    let (instance, map) = unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let map = CreateWindowExW(
            Default::default(),
            w!("NetMap.Map"),
            w!("Network map"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            Some(host.window.get()),
            Some(HMENU(ids::IDC_MAP as usize as *mut _)),
            Some(instance),
            Some((host as *const Host).cast()),
        )?;
        (instance, map)
    };
    host.map_window.set(map);
    host.accessibility.attach(map)?;
    host.create_tooltip(instance, host.window.get(), map)?;
    host.app.map.attach(map)?;
    host.layout(true)?;
    Ok(map)
}

fn repaint(window: HWND) -> Result<()> {
    // SAFETY: Deliver the owned map's real WM_PAINT/Present path synchronously.
    unsafe {
        InvalidateRect(Some(window), None, false).ok()?;
        UpdateWindow(window).ok()
    }
}

fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
