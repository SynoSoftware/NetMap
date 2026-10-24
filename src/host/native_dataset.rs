//! Real dataset transaction boundaries, restart readers and modal availability.

use super::*;
use crate::{
    app::{
        native_checks as facts,
        settings::{self, Page},
        work::{
            native_dataset as work,
            native_shutdown::{self, Kind},
        },
    },
    geolocation::{
        Availability,
        dataset::native_checks::{Action, Control},
    },
};
use std::time::{Duration, Instant};
use windows::{
    Win32::{System::Threading::GetCurrentThreadId, UI::Controls::*},
    core::BOOL,
};

const TIMER: usize = 0x4e44;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Rollback,
    Availability,
    Fallback,
    Restart,
}
thread_local! {
    static REQUESTED: Cell<Option<Mode>> = const { Cell::new(None) };
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
    static HELD: RefCell<Option<Control>> = const { RefCell::new(None) };
}

pub(super) fn requested() -> bool {
    REQUESTED.get().is_some()
}
struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        REQUESTED.set(None);
        DRIVER.with(|slot| slot.borrow_mut().take());
        HELD.with(|slot| slot.borrow_mut().take());
        work::clear();
    }
}

fn run(mode: Mode) -> Result<()> {
    REQUESTED.set(Some(mode));
    let _scope = Scope;
    if mode == Mode::Fallback {
        native_shutdown::observe(Kind::DatasetFallback);
    }
    facts::run()
}

#[test]
#[ignore = "Uses owned MMDB copies and real installer rollback; run with the dataset wrapper"]
fn native_dataset_reopen_failure_rolls_back_and_retry_installs() -> Result<()> {
    run(Mode::Rollback)
}
#[test]
#[ignore = "Creates owned Settings and changes actual local reader availability; use the dataset wrapper"]
fn native_settings_revalidates_cities_after_actual_dataset_changes() -> Result<()> {
    run(Mode::Availability)
}
#[test]
#[ignore = "Real fallback terminates this process after replacement and before reopen; use the dataset wrapper"]
fn native_dataset_replacement_survives_host_fallback() -> Result<()> {
    run(Mode::Fallback)?;
    Err(failure(
        "Held real dataset install did not reach process fallback",
    ))
}
#[test]
#[ignore = "Reads an interrupted owned profile and its flushed backup; use the dataset wrapper"]
fn native_dataset_restart_reads_replacement_and_recoverable_backup() -> Result<()> {
    run(Mode::Restart)
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    let window = host.window.get();
    if !window.is_invalid() {
        // SAFETY: The modal loop returned; retire only this owned Host timer.
        unsafe {
            let _ = KillTimer(Some(window), TIMER);
        }
    }
    DRIVER.with(|slot| slot.borrow_mut().take());
    if result.is_err() {
        HELD.with(|slot| slot.borrow_mut().take());
    }
    if window.is_invalid() {
        return result;
    }
    // SAFETY: Destroy only this fixture's still-live ordinary Host. Fallback
    // success already used production WM_CLOSE and retains its install gate.
    let cleanup = unsafe { DestroyWindow(window) };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    let mode = REQUESTED
        .get()
        .ok_or_else(|| failure("Dataset check was not requested"))?;
    // SAFETY: Callback-free owned timer bounds each native message wait.
    if unsafe { SetTimer(Some(host.window.get()), TIMER, 20, None) } == 0 {
        return Err(Error::from_thread());
    }
    let initial = read(host, work::Read::Load)?;
    if mode == Mode::Availability {
        require(
            !host.app.city_available(),
            "Availability profile must initially have no city dataset",
        )?;
        return availability(host, initial.report.version);
    }
    usable(&initial)?;
    match mode {
        Mode::Rollback => rollback(host, initial),
        Mode::Fallback => fallback(host, initial.report.version),
        Mode::Restart => {
            require(
                initial
                    .city
                    .as_ref()
                    .is_some_and(|path| path.ends_with("city.mmdb")),
                "Restart did not first open the actual replaced canonical file",
            )?;
            let recovered = read(host, work::Read::Withdraw)?;
            usable(&recovered)?;
            require(
                recovered.location == initial.location
                    && recovered.report.version > initial.report.version
                    && recovered.city.as_ref().is_some_and(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == "backup")
                    }),
                "Restart did not recover the flushed old backup after canonical withdrawal",
            )?;
            println!(
                "NETMAP_DATASET {{\"event\":\"restart_verified\",\"canonical_version\":{},\"backup_version\":{},\"same_lookup\":true,\"backup_reader\":true}}",
                initial.report.version, recovered.report.version
            );
            Ok(())
        }
        Mode::Availability => unreachable!(),
    }
}

fn rollback(host: &Host, initial: work::Readback) -> Result<()> {
    let mut gate = work::arm(Some(Action::FailReopen))
        .map_err(native)?
        .ok_or_else(|| failure("Reopen-failure gate was not armed"))?;
    host.app.download_city();
    let checkpoint = wait_for(host, || gate.checkpoint().map_err(native))?;
    require(
        checkpoint.displaced.is_some()
            && checkpoint.target.ends_with("city.mmdb")
            && checkpoint
                .backup
                .extension()
                .is_some_and(|extension| extension == "backup"),
        "Real replacement did not reach its owned reopen-failure boundary",
    )?;
    gate.release();
    finish_city(host)?;
    require(
        host.app.city_status().is_some(),
        "Missing target did not produce the actual installer reopen error",
    )?;
    let restored = read(host, work::Read::Current)?;
    usable(&restored)?;
    require(
        restored.report.version == initial.report.version && restored.location == initial.location,
        "Failed reopen changed version or lost the previous usable lookup",
    )?;
    work::arm(None).map_err(native)?;
    host.app.download_city();
    finish_city(host)?;
    let retried = read(host, work::Read::Current)?;
    usable(&retried)?;
    require(
        host.app.city_status().is_none()
            && retried.report.version == initial.report.version + 1
            && retried.location == initial.location,
        "Real retry failed to install/reopen and advance the dataset version",
    )?;
    println!(
        "NETMAP_DATASET {{\"event\":\"rollback_retry_verified\",\"before_version\":{},\"rollback_version\":{},\"retry_version\":{},\"same_lookup\":true,\"actual_reopen_error\":true}}",
        initial.report.version, restored.report.version, retried.report.version
    );
    Ok(())
}

fn fallback(host: &Host, version: u64) -> Result<()> {
    let gate = work::arm(Some(Action::Hold))
        .map_err(native)?
        .ok_or_else(|| failure("Installer fallback gate was not armed"))?;
    host.app.download_city();
    let checkpoint = wait_for(host, || gate.checkpoint().map_err(native))?;
    require(
        checkpoint.displaced.is_none()
            && checkpoint.target.ends_with("city.mmdb")
            && host.app.city_available(),
        "Actual replacement boundary lost its previous ready report",
    )?;
    HELD.with(|slot| *slot.borrow_mut() = Some(gate));
    native_shutdown::record(
        Kind::DatasetFallback,
        "dataset_replaced_before_reopen",
        &format!(
            "\"previous_version\":{version},\"backup_flushed\":true,\"gate_maximum_ms\":15000,\"boundary_tick_ms\":{}",
            checkpoint.at
        ),
    );
    let window = host.window.get();
    let since = native_shutdown::tick();
    native_shutdown::record(
        Kind::DatasetFallback,
        "close_requested",
        &format!("\"window\":{}", window.0 as usize),
    );
    // SAFETY: Close only the exact owned ordinary Host using its actual handler.
    unsafe {
        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))?;
    }
    wait_for(host, || {
        // SAFETY: Native lifetime query of the exact HWND captured before close.
        let alive = unsafe { IsWindow(Some(window)).as_bool() };
        Ok((!alive).then_some(()))
    })?;
    require(
        host.window.get().is_invalid(),
        "Real Host close did not detach its native handle",
    )?;
    native_shutdown::record(
        Kind::DatasetFallback,
        "window_removed",
        &format!(
            "\"window\":{},\"close_elapsed_ms\":{}",
            window.0 as usize,
            native_shutdown::tick().saturating_sub(since)
        ),
    );
    native_shutdown::record(Kind::DatasetFallback, "dataset_held_through_finish", "");
    Ok(())
}

fn read(host: &Host, action: work::Read) -> Result<work::Readback> {
    let reading = work::read(&host.app, action).map_err(native)?;
    let mut result = None;
    wait_for(host, || {
        if result.is_none() {
            result = reading.ready().map_err(native)?;
        }
        if result.as_ref().is_some_and(|readback| {
            facts::location_report(&host.app)
                .is_some_and(|report| report.version == readback.report.version)
        }) {
            return Ok(result.take());
        }
        Ok(None)
    })
}

fn usable(read: &work::Readback) -> Result<()> {
    require(
        matches!(read.report.city, Availability::Ready { .. })
            && read
                .location
                .as_ref()
                .is_some_and(|location| location.point.is_some()),
        "Actual local reader/report did not provide a usable city lookup",
    )
}

fn finish_city(host: &Host) -> Result<()> {
    wait_for(host, || Ok(host.app.can_download_city().then_some(())))
}

fn wait_for<T>(host: &Host, mut ready: impl FnMut() -> Result<Option<T>>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(result) = ready()? {
            return Ok(result);
        }
        require(
            Instant::now() < deadline,
            "Registered dataset operation exceeded its native observation bound",
        )?;
        let mut message = MSG::default();
        // SAFETY: Preserve ordinary UI-thread dispatch while workers own all
        // disk/reader waits. The owned timer wakes this bounded observation loop.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(failure("Host retired during dataset observation"));
            }
            if !host.navigation_key(&message)
                && !IsDialogMessageW(host.window.get(), &message).as_bool()
            {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

enum Step {
    Location,
    Draft,
    Installed,
    Withdrawn,
    Done,
}
struct Driver {
    step: Step,
    deadline: Instant,
    initial: u64,
    installed: u64,
    withdrawn: u64,
    reading: Option<work::Reading>,
    failure: Option<Error>,
}

fn availability(host: &Host, initial: u64) -> Result<()> {
    DRIVER.with(|slot| {
        *slot.borrow_mut() = Some(Driver {
            step: Step::Location,
            deadline: Instant::now() + Duration::from_secs(60),
            initial,
            installed: 0,
            withdrawn: 0,
            reading: None,
            failure: None,
        })
    });
    // SAFETY: Replace the same owned timer only while the native modal loop runs.
    if unsafe { SetTimer(Some(host.window.get()), TIMER, 20, Some(tick)) } == 0 {
        return Err(Error::from_thread());
    }
    settings::show(&host.app, host.window.get(), Page::Location)?;
    let mut driver = DRIVER
        .with(|slot| slot.borrow_mut().take())
        .ok_or_else(|| failure("Dataset modal driver retired"))?;
    if let Some(error) = driver.failure.take() {
        return Err(error);
    }
    require(
        matches!(driver.step, Step::Done),
        "Settings closed before availability revalidation completed",
    )?;
    require(
        host.app.preferences().committed.detail == settings::Detail::Countries,
        "Rejected Cities draft was committed",
    )?;
    println!(
        "NETMAP_DATASET {{\"event\":\"settings_availability_verified\",\"missing_version\":{},\"ready_version\":{},\"withdrawn_version\":{},\"raw_latitude_retained\":true,\"stale_cities_rejected\":true}}",
        driver.initial,
        driver.installed,
        facts::location_report(&host.app).map_or(0, |report| report.version)
    );
    Ok(())
}

unsafe extern "system" fn tick(window: HWND, _: u32, _: usize, _: u32) {
    let Some(mut driver) = DRIVER.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    if !matches!(driver.step, Step::Done) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: The scoped timer and stable Host context share this UI thread.
            let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Host;
            // SAFETY: The Host outlives its owned timer and this callback.
            let host =
                unsafe { pointer.as_ref() }.ok_or_else(|| failure("Host context retired"))?;
            driver.advance(host)
        }))
        .unwrap_or_else(|_| Err(failure("Dataset modal driver panicked")));
        if let Err(error) = result {
            driver.failure = Some(error);
            driver.step = Step::Done;
        }
        if matches!(driver.step, Step::Done)
            && let Ok(Some(dialog)) = settings_window(window)
        {
            // SAFETY: Close only the owned current Settings dialog, after borrows.
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
            "Modal dataset availability journey timed out",
        )?;
        let dialog = settings_window(host.window.get())?
            .ok_or_else(|| failure("Owned Settings shell is missing"))?;
        match self.step {
            Step::Location => {
                controls::set_text(descendant(dialog, ids::IDC_LATITUDE)?, "unfinished");
                settings::show(&host.app, host.window.get(), Page::Map)?;
                self.step = Step::Draft;
            }
            Step::Draft => {
                let combo = descendant(dialog, ids::IDC_DETAIL)?;
                // SAFETY: Select Cities on the owned native combo, then send its
                // ordinary notification to the actual page/production draft owner.
                unsafe {
                    SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(1)), None);
                    let page = GetParent(combo)?;
                    SendMessageW(
                        page,
                        WM_COMMAND,
                        Some(WPARAM(
                            ids::IDC_DETAIL as usize | ((CBN_SELCHANGE as usize) << 16),
                        )),
                        Some(LPARAM(combo.0 as isize)),
                    );
                    SendMessageW(
                        dialog,
                        WM_COMMAND,
                        Some(WPARAM(ids::IDC_APPLY as usize)),
                        None,
                    );
                }
                require(
                    !host.app.city_available() && visible(descendant(dialog, ids::IDC_DETAIL)?),
                    "Unavailable Cities validation did not stay on Map",
                )?;
                require(
                    status(dialog)?.contains("Get city data"),
                    "Unavailable Cities did not report its actual validation reason",
                )?;
                retained(dialog)?;
                work::arm(None).map_err(native)?;
                host.app.download_city();
                self.step = Step::Installed;
            }
            Step::Installed => {
                if !host.app.can_download_city() {
                    return Ok(());
                }
                require(
                    host.app.city_available() && host.app.city_status().is_none(),
                    "Actual install did not update open Settings availability",
                )?;
                self.installed =
                    facts::location_report(&host.app).map_or(0, |report| report.version);
                require(
                    self.installed > self.initial,
                    "Installed report did not advance the dataset version",
                )?;
                retained(dialog)?;
                // Now Cities is available, validation must reach the retained
                // invalid latitude and reveal its real Location editor instead.
                // SAFETY: Actual Apply to the owned dialog; no draft borrow held.
                unsafe {
                    SendMessageW(
                        dialog,
                        WM_COMMAND,
                        Some(WPARAM(ids::IDC_APPLY as usize)),
                        None,
                    );
                }
                require(
                    visible(descendant(dialog, ids::IDC_LATITUDE)?),
                    "Available Cities did not let validation reach the retained invalid latitude",
                )?;
                require(
                    status(dialog)?.contains("finite latitude"),
                    "Ready availability did not revalidate the current raw latitude",
                )?;
                self.reading = Some(work::read(&host.app, work::Read::Withdraw).map_err(native)?);
                self.step = Step::Withdrawn;
            }
            Step::Withdrawn => {
                if let Some(reading) = self.reading.as_ref()
                    && let Some(read) = reading.ready().map_err(native)?
                {
                    require(
                        matches!(read.report.city, Availability::Missing)
                            && read.report.version > self.installed,
                        "Actual file withdrawal/reload did not report Missing",
                    )?;
                    self.withdrawn = read.report.version;
                    self.reading = None;
                }
                if self.withdrawn == 0
                    || host.app.city_available()
                    || facts::location_report(&host.app)
                        .is_none_or(|report| report.version != self.withdrawn)
                {
                    return Ok(());
                }
                retained(dialog)?;
                // SAFETY: Existing Cities draft must now be rejected by the
                // real current-availability validator before its invalid latitude.
                unsafe {
                    SendMessageW(
                        dialog,
                        WM_COMMAND,
                        Some(WPARAM(ids::IDC_APPLY as usize)),
                        None,
                    );
                }
                require(
                    visible(descendant(dialog, ids::IDC_DETAIL)?),
                    "Stale Cities draft was not rejected on its Map page",
                )?;
                require(
                    status(dialog)?.contains("Get city data"),
                    "Withdrawn availability did not report the stale Cities validation reason",
                )?;
                retained(dialog)?;
                require(
                    matches!(host.app.preferences().phase, settings::Phase::Ready),
                    "Rejected stale Cities started a preferences save",
                )?;
                self.step = Step::Done;
            }
            Step::Done => {}
        }
        Ok(())
    }
}

fn retained(dialog: HWND) -> Result<()> {
    require(
        controls::text(descendant(dialog, ids::IDC_LATITUDE)?) == "unfinished",
        "Availability refresh overwrote invalid raw latitude",
    )?;
    let combo = descendant(dialog, ids::IDC_DETAIL)?;
    // SAFETY: Read selection only from this owned native combo.
    let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, None, None).0 };
    require(
        selected == 1,
        "Availability refresh overwrote the pending Cities draft",
    )
}

fn status(dialog: HWND) -> Result<String> {
    let status = descendant(dialog, ids::IDC_SETTINGS_STATUS)?;
    // SAFETY: Read one part of this owned native status bar. Its bounded text
    // length and destination remain local through synchronous control queries.
    unsafe {
        let length =
            SendMessageW(status, SB_GETTEXTLENGTHW, Some(WPARAM(1)), None).0 as usize & 0xffff;
        require(
            length < 4096,
            "Owned Settings status exceeded the observation bound",
        )?;
        let mut buffer = vec![0u16; length + 1];
        SendMessageW(
            status,
            SB_GETTEXTW,
            Some(WPARAM(1)),
            Some(LPARAM(buffer.as_mut_ptr() as isize)),
        );
        Ok(String::from_utf16_lossy(&buffer[..length]))
    }
}
fn visible(window: HWND) -> bool {
    // SAFETY: Query only a descendant of the live owned Settings shell.
    unsafe { IsWindowVisible(window).as_bool() }
}

struct Search {
    owner: HWND,
    id: u16,
    found: HWND,
}
unsafe extern "system" fn find_dialog(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: Synchronous enumeration borrows this caller-owned stack record.
    let search = unsafe { &mut *(parameter.0 as *mut Search) };
    // SAFETY: Enumerated windows belong to this UI thread; exact owner and
    // resource-defined navigation identify this fixture's actual Settings shell.
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
    // SAFETY: Record remains live for synchronous owned-thread enumeration.
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
    // SAFETY: Caller owns this record throughout synchronous child enumeration.
    let search = unsafe { &mut *(parameter.0 as *mut Search) };
    // SAFETY: Only descendants of the owned Settings shell are queried.
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
    // SAFETY: Control IDs used here are unique within this owned resource shell.
    unsafe {
        let _ = EnumChildWindows(
            Some(dialog),
            Some(find_control),
            LPARAM((&mut search as *mut Search) as isize),
        );
    }
    if search.found.is_invalid() {
        Err(failure(&format!("Owned Settings control {id} is missing")))
    } else {
        Ok(search.found)
    }
}
fn native(error: std::io::Error) -> Error {
    failure(&error.to_string())
}
fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
