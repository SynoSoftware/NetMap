//! Real modal Settings detachment/reopen while its accepted write is pending.

use super::*;
use crate::app::{
    native_checks,
    settings::{self, Page, Phase},
    work::{
        native_preferences,
        native_shutdown::{self, Kind},
    },
};
use std::time::{Duration, Instant};
use windows::{
    Win32::{System::Threading::GetCurrentThreadId, UI::Input::KeyboardAndMouse::IsWindowEnabled},
    core::BOOL,
};

const TIMER: usize = 0x4e50;
thread_local! {
    static REQUESTED: Cell<Option<Mode>> = const { Cell::new(None) };
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
}

pub(super) fn requested() -> bool {
    REQUESTED.get().is_some()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Reopen,
    CommitOnExit,
    FallbackOnExit,
}

impl Mode {
    fn shutdown(self) -> Option<Kind> {
        match self {
            Self::Reopen => None,
            Self::CommitOnExit => Some(Kind::PreferencesCommit),
            Self::FallbackOnExit => Some(Kind::PreferencesFallback),
        }
    }
}

struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        REQUESTED.set(None);
        // Error/normal-return paths release the accepted save. A real fallback
        // terminates first, while this UI-thread-owned controller stays alive.
        DRIVER.with(|slot| slot.borrow_mut().take());
    }
}

#[test]
#[ignore = "Creates owned modal Settings and holds one registered save; run alone with a process watchdog"]
fn native_pending_save_survives_settings_close_and_reopen() -> Result<()> {
    REQUESTED.set(Some(Mode::Reopen));
    let _scope = Scope;
    native_checks::run()
}

#[test]
#[ignore = "Closes an owned Host with an accepted save held pending; run alone with a process watchdog"]
fn native_host_close_commits_an_accepted_pending_save() -> Result<()> {
    REQUESTED.set(Some(Mode::CommitOnExit));
    let _scope = Scope;
    native_shutdown::observe(Kind::PreferencesCommit);
    native_checks::run()
}

#[test]
#[ignore = "The held accepted save reaches the real five-second process fallback; use the pending-Settings wrapper"]
fn native_host_close_preserves_preferences_when_pending_save_reaches_fallback() -> Result<()> {
    REQUESTED.set(Some(Mode::FallbackOnExit));
    let _scope = Scope;
    native_shutdown::observe(Kind::PreferencesFallback);
    native_checks::run()?;
    Err(failure(
        "Held accepted save did not reach the real process fallback",
    ))
}

enum Step {
    Edit,
    Entered,
    OldDesktop,
    OldClosed,
    Reopened,
    NewDesktop,
    Release,
    Committed,
    Hold(Instant),
    Done,
}

struct Driver {
    step: Step,
    deadline: Instant,
    gate: native_preferences::Control,
    old_window: HWND,
    new_window: HWND,
    save: u64,
    failure: Option<Error>,
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    // Modal loops have returned. Release on errors and ordinary completion;
    // only the deliberate fallback keeps its sole pending save gate alive.
    let window = host.window.get();
    if !window.is_invalid() {
        // SAFETY: Only this fixture's current Host timer is retired.
        unsafe {
            let _ = KillTimer(Some(window), TIMER);
        }
    }
    if result.is_err() || REQUESTED.get() != Some(Mode::FallbackOnExit) {
        DRIVER.with(|slot| slot.borrow_mut().take());
    }
    if window.is_invalid() {
        return result;
    }
    // SAFETY: Only this fixture's still-live normal Host is destroyed. Success
    // in either exit case already used the actual production WM_CLOSE path.
    let cleanup = unsafe { DestroyWindow(window) };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    require(
        host.app.preferences().committed.bright_seconds != 17,
        "Fixture baseline already has the requested value",
    )?;
    let mode = REQUESTED
        .get()
        .ok_or_else(|| failure("Settings check was not requested"))?;
    let gate = match mode.shutdown() {
        Some(kind) => native_preferences::arm_exit(kind),
        None => native_preferences::arm(),
    }
    .map_err(|error| failure(&error.to_string()))?;
    DRIVER.with(|slot| {
        *slot.borrow_mut() = Some(Driver {
            step: Step::Edit,
            deadline: Instant::now() + Duration::from_secs(8),
            gate,
            old_window: HWND::default(),
            new_window: HWND::default(),
            save: 0,
            failure: None,
        })
    });
    // SAFETY: Timer is scoped to this live UI-thread Host and pumps inside the
    // real production DialogBox loop. It never calls a worker or waits for one.
    if unsafe { SetTimer(Some(host.window.get()), TIMER, 20, Some(tick)) } == 0 {
        return Err(Error::from_thread());
    }
    settings::show(&host.app, host.window.get(), Page::Connections)?;
    if let Some(error) = DRIVER.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .and_then(|driver| driver.failure.take())
    }) {
        return Err(error);
    }
    let ready = DRIVER.with(|slot| {
        slot.borrow().as_ref().is_some_and(|driver| {
            driver.failure.is_none()
                && matches!(driver.step, Step::OldClosed)
                && driver.gate.entered()
        })
    });
    require(
        ready,
        "First dialog did not detach with its accepted save held pending",
    )?;
    require(
        settings_window(host.window.get())?.is_none(),
        "Old Settings registration is still visible",
    )?;
    require(
        matches!(host.app.preferences().phase, Phase::Saving { .. }),
        "Closing Settings cancelled or completed the held save",
    )?;
    if let Some(kind) = mode.shutdown() {
        return close_host(host, kind, mode == Mode::CommitOnExit);
    }
    DRIVER.with(|slot| {
        if let Some(driver) = slot.borrow_mut().as_mut() {
            driver.step = Step::Reopened;
        }
    });
    // A distinct show invocation after the old modal returned establishes the
    // new production dialog generation, even if Windows reuses its numeric HWND.
    settings::show(&host.app, host.window.get(), Page::Connections)?;
    let mut driver = DRIVER
        .with(|slot| slot.borrow_mut().take())
        .ok_or_else(|| failure("Native Settings driver was retired unexpectedly"))?;
    if let Some(error) = driver.failure.take() {
        return Err(error);
    }
    require(
        matches!(driver.step, Step::Done),
        "Reopened dialog closed before pending-save acceptance completed",
    )?;
    require(
        settings_window(host.window.get())?.is_none(),
        "Final Cancel did not close the owned Settings dialog",
    )?;
    require(
        host.app.preferences().committed.bright_seconds == 17,
        "Accepted save was not retained after final Cancel",
    )?;
    println!(
        "NETMAP_PENDING_SETTINGS {{\"old_window\":{},\"new_window\":{},\"save\":{},\"bright\":17,\"worker_entered\":true,\"old_detached\":true,\"new_observed_pending\":true,\"lazy_desktop_disabled\":true,\"new_survived_old_ok\":true,\"clean_baseline\":true}}",
        driver.old_window.0 as usize, driver.new_window.0 as usize, driver.save
    );
    Ok(())
}

fn close_host(host: &Host, kind: Kind, commit: bool) -> Result<()> {
    let (save, modal) = DRIVER.with(|slot| -> Result<(u64, HWND)> {
        let slot = slot.borrow();
        let driver = slot
            .as_ref()
            .ok_or_else(|| failure("Pending save controller retired"))?;
        require(
            driver.gate.entered(),
            "Save worker did not enter before Host close",
        )?;
        Ok((driver.save, driver.old_window))
    })?;
    let current = host.app.preferences();
    require(
        matches!(current.phase, Phase::Saving { id, ref preferences }
        if id == save && preferences.bright_seconds == 17)
            && current.committed.bright_seconds != 17,
        "Accepted save was not still pending after modal close",
    )?;
    native_shutdown::record(
        kind,
        "preferences_accepted",
        &format!(
            "\"save\":{save},\"desired_bright\":17,\"committed_bright\":{},\"modal_window\":{},\"modal_detached\":true",
            current.committed.bright_seconds, modal.0 as usize
        ),
    );
    let window = host.window.get();
    let since = native_shutdown::tick();
    native_shutdown::record(
        kind,
        "close_requested",
        &format!("\"window\":{}", window.0 as usize),
    );
    // SAFETY: Close the exact owned normal Host; no fixture cancellation or
    // destruction substitutes for the real production close behavior.
    unsafe {
        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: Query only the owned HWND captured before its close request.
        if !unsafe { IsWindow(Some(window)).as_bool() } {
            require(
                host.window.get().is_invalid(),
                "Closed Host did not detach its HWND",
            )?;
            native_shutdown::record(
                kind,
                "window_removed",
                &format!(
                    "\"window\":{},\"close_elapsed_ms\":{}",
                    window.0 as usize,
                    native_shutdown::tick().saturating_sub(since)
                ),
            );
            if commit {
                let mut driver = DRIVER
                    .with(|slot| slot.borrow_mut().take())
                    .ok_or_else(|| failure("Pending save gate retired before release"))?;
                native_shutdown::record(kind, "preferences_release_after_window", "");
                driver.gate.release();
            } else {
                native_shutdown::record(kind, "preferences_held_through_finish", "");
            }
            return Ok(());
        }
        require(
            Instant::now() < deadline,
            "Owned Host did not close during the pending save",
        )?;
        let mut message = MSG::default();
        // SAFETY: Same production dispatch order on this UI thread; the owned
        // timer bounds waiting until real close destroys the window and timer.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(failure(
                    "Host message loop retired before HWND removal was observed",
                ));
            }
            if !host.navigation_key(&message) && !IsDialogMessageW(window, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

unsafe extern "system" fn tick(window: HWND, _: u32, _: usize, _: u32) {
    // No RefCell borrow crosses a native call; reentrant delivery finds no driver.
    let Some(mut driver) = DRIVER.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    if !matches!(driver.step, Step::Done | Step::OldClosed) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: This scoped timer cannot outlive its Host's stable context.
            let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Host;
            // SAFETY: All driver calls occur on the Host's creating UI thread.
            let host =
                unsafe { pointer.as_ref() }.ok_or_else(|| failure("Host context retired"))?;
            driver.advance(host)
        }))
        .unwrap_or_else(|_| Err(failure("Native Settings driver panicked")));
        if let Err(error) = result {
            driver.failure = Some(error);
            driver.step = Step::Done;
        }
        if (driver.failure.is_some() || matches!(driver.step, Step::OldClosed))
            && let Ok(Some(dialog)) = settings_window(window)
        {
            // SAFETY: Close only the currently owned Settings dialog after
            // assertions/borrows have returned; accepted saves remain owned.
            unsafe {
                let _ = PostMessageW(Some(dialog), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
    DRIVER.with(|slot| *slot.borrow_mut() = Some(driver));
}

impl Driver {
    fn advance(&mut self, host: &Host) -> Result<()> {
        require(
            Instant::now() < self.deadline,
            "Pending Settings journey exceeded its bounded deadline",
        )?;
        let dialog = settings_window(host.window.get())?
            .ok_or_else(|| failure("Owned modal Settings window is unavailable"))?;
        match self.step {
            Step::Edit => {
                let bright = descendant(dialog, ids::IDC_BRIGHT)?;
                require(enabled(bright), "Initial Connections editor is disabled")?;
                self.old_window = dialog;
                controls::set_text(bright, "17");
                require(
                    enabled(descendant(dialog, ids::IDC_APPLY)?),
                    "Changed Connections draft was not dirty",
                )?;
                command(dialog, IDOK.0 as u16)?;
                self.step = Step::Entered;
            }
            Step::Entered => {
                if !self.gate.entered() {
                    return Ok(());
                }
                let current = host.app.preferences();
                let Phase::Saving { id, preferences } = current.phase else {
                    return Err(failure(
                        "Save worker entered without application Saving state",
                    ));
                };
                require(
                    preferences.bright_seconds == 17 && current.committed.bright_seconds != 17,
                    "Held save did not preserve separate committed/accepted preferences",
                )?;
                self.save = id;
                pending_controls(dialog, ids::IDC_BRIGHT)?;
                controls::set_text(descendant(dialog, ids::IDC_SEARCH)?, "desk");
                self.step = Step::OldDesktop;
            }
            Step::OldDesktop => {
                pending_controls(dialog, ids::IDC_DESKTOP_ENABLED)?;
                require(
                    !enabled(descendant(dialog, ids::IDC_MONITOR)?),
                    "New Desktop monitor editor was enabled during save",
                )?;
                self.step = Step::OldClosed;
            }
            Step::Reopened => {
                require(
                    self.gate.entered()
                        && matches!(host.app.preferences().phase, Phase::Saving { .. }),
                    "Save was no longer pending when the new dialog opened",
                )?;
                self.new_window = dialog;
                pending_controls(dialog, ids::IDC_BRIGHT)?;
                controls::set_text(descendant(dialog, ids::IDC_SEARCH)?, "desk");
                self.step = Step::NewDesktop;
            }
            Step::NewDesktop => {
                pending_controls(dialog, ids::IDC_DESKTOP_ENABLED)?;
                require(
                    !enabled(descendant(dialog, ids::IDC_MONITOR)?),
                    "Reopened lazy Desktop monitor editor was enabled during save",
                )?;
                controls::set_text(descendant(dialog, ids::IDC_SEARCH)?, "");
                // Existing-dialog navigation posts its production generation-tagged
                // SELECT_PAGE; its native modal loop applies the page change.
                settings::show(&host.app, host.window.get(), Page::Connections)?;
                self.step = Step::Release;
            }
            Step::Release => {
                pending_controls(dialog, ids::IDC_BRIGHT)?;
                self.gate.release();
                self.step = Step::Committed;
            }
            Step::Committed => {
                let current = host.app.preferences();
                if !matches!(current.phase, Phase::Ready) {
                    return Ok(());
                }
                require(
                    current.error.is_none() && current.committed.bright_seconds == 17,
                    "Real settings storage/report did not commit the held value",
                )?;
                let bright = descendant(dialog, ids::IDC_BRIGHT)?;
                if !enabled(bright) || controls::text(bright) != "17" {
                    return Ok(());
                }
                require(
                    dialog == self.new_window
                        && enabled(descendant(dialog, IDOK.0 as u16)?)
                        && !enabled(descendant(dialog, ids::IDC_APPLY)?),
                    "New dialog did not acquire a clean committed baseline",
                )?;
                require(
                    enabled(descendant(dialog, ids::IDC_DESKTOP_ENABLED)?)
                        && enabled(descendant(dialog, ids::IDC_MONITOR)?),
                    "Retained Desktop editors did not re-enable after commit",
                )?;
                self.step = Step::Hold(Instant::now());
            }
            Step::Hold(since) => {
                require(
                    dialog == self.new_window
                        && host.app.preferences().committed.bright_seconds == 17,
                    "Earlier OK intent closed the newer dialog or reverted the committed value",
                )?;
                if since.elapsed() >= Duration::from_millis(200) {
                    require(
                        !enabled(descendant(dialog, ids::IDC_APPLY)?),
                        "Committed observer dialog became dirty without an edit",
                    )?;
                    command(dialog, IDCANCEL.0 as u16)?;
                    self.step = Step::Done;
                }
            }
            Step::OldClosed | Step::Done => {}
        }
        Ok(())
    }
}

fn pending_controls(dialog: HWND, editor: u16) -> Result<()> {
    for id in [
        editor,
        ids::IDC_RESET_PAGE,
        ids::IDC_RESET_ALL,
        ids::IDC_APPLY,
        IDOK.0 as u16,
    ] {
        require(
            !enabled(descendant(dialog, id)?),
            "An editor, reset or commit control was enabled during accepted save",
        )?;
    }
    for id in [ids::IDC_SEARCH, ids::IDC_PAGES, IDCANCEL.0 as u16] {
        require(
            enabled(descendant(dialog, id)?),
            "Search, navigation or Cancel was disabled during save",
        )?;
    }
    Ok(())
}

fn enabled(window: HWND) -> bool {
    // SAFETY: Every queried HWND was found within this owned live dialog.
    unsafe { IsWindowEnabled(window).as_bool() }
}

fn command(dialog: HWND, id: u16) -> Result<()> {
    // SAFETY: Numeric command sent only to this UI-thread owned modal dialog.
    unsafe { PostMessageW(Some(dialog), WM_COMMAND, WPARAM(id as usize), LPARAM(0)) }
}

struct Search {
    owner: HWND,
    id: u16,
    found: HWND,
}
unsafe extern "system" fn find_dialog(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: Enumeration synchronously borrows this local search record.
    let search = unsafe { &mut *(parameter.0 as *mut Search) };
    // SAFETY: Only this UI thread's top-level HWNDs are enumerated; owner and
    // the resource-defined navigation control identify our Settings shell.
    unsafe {
        if GetWindow(window, GW_OWNER).is_ok_and(|owner| owner == search.owner)
            && GetDlgItem(Some(window), ids::IDC_PAGES as i32).is_ok()
        {
            search.found = window;
        }
    }
    BOOL(1)
}

fn settings_window(owner: HWND) -> Result<Option<HWND>> {
    let mut search = Search {
        owner,
        id: 0,
        found: HWND::default(),
    };
    // SAFETY: The search record outlives synchronous owned-thread enumeration.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(find_dialog),
            LPARAM((&mut search as *mut Search) as isize),
        )
        .ok()?;
    }
    Ok((!search.found.is_invalid()).then_some(search.found))
}

unsafe extern "system" fn find_control(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: Synchronous enumeration borrows the caller's stack record only.
    let search = unsafe { &mut *(parameter.0 as *mut Search) };
    // SAFETY: Only descendants of the current owned Settings shell are queried.
    if unsafe { GetDlgCtrlID(window) } == i32::from(search.id) {
        search.found = window;
    }
    BOOL(1)
}

fn descendant(dialog: HWND, id: u16) -> Result<HWND> {
    let mut search = Search {
        owner: dialog,
        id,
        found: HWND::default(),
    };
    // SAFETY: Control IDs are unique across these resource-created lazy pages;
    // no callback or record escapes this synchronous owned-window enumeration.
    unsafe {
        let _ = EnumChildWindows(
            Some(dialog),
            Some(find_control),
            LPARAM((&mut search as *mut Search) as isize),
        );
    }
    if search.found.is_invalid() {
        Err(failure(&format!(
            "Owned Settings control {id} was not created"
        )))
    } else {
        Ok(search.found)
    }
}

fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
