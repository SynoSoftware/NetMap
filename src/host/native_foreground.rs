//! Real foreground delivery and presentation gating, absent from product builds.

use super::*;
use crate::{app::native_checks as facts, connections::Rates, map::Presentations};
use std::{
    ffi::OsString,
    io::Write,
    os::windows::io::AsRawHandle,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{FILETIME, HANDLE},
        Graphics::Gdi::UpdateWindow,
        System::Threading::{GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes},
    },
    core::BOOL,
};

const TIMER: usize = 0x4e46;
const HOLD: Duration = Duration::from_millis(600);
thread_local! {
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
}

pub(super) fn requested() -> bool {
    REQUESTED.get()
}

#[test]
#[ignore = "Creates owned Host/Settings and real /s child; requires an interactive desktop and process watchdog"]
fn native_host_pauses_presentations_for_actual_saver() -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(false);
        }
    }
    REQUESTED.set(true);
    let _scope = Scope;
    facts::run()
}

enum Phase {
    Baseline,
    Cover,
    Hold {
        since: Instant,
        attempts: u64,
    },
    Resume {
        attempts: u64,
        minimum_age: Duration,
    },
    Done,
}

struct Driver {
    modal: bool,
    deadline: Instant,
    phase: Phase,
    app_path: OsString,
    observations: Rc<Presentations>,
    child: Option<SaverChild>,
    expected_model: Option<std::sync::Arc<crate::map::model::Model>>,
    before_revision: u64,
    failure: Option<Error>,
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    // SAFETY: All timer callbacks and the modal dialog have returned. Destroy
    // only this fixture's normal presentation; ordinary Host cleanup follows.
    let cleanup = unsafe { DestroyWindow(host.window.get()) };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    let app_path = std::env::var_os("NETMAP_NATIVE_APP")
        .filter(|path| PathBuf::from(path).is_absolute())
        .ok_or_else(|| incomplete("NETMAP_NATIVE_APP must name the built production executable"))?;
    require(
        host.foreground.borrow().is_some(),
        "Native foreground hook is unavailable",
    )?;
    let observations = host.app.map.observe_presentations()?;
    // SAFETY: The first ShowWindow may honor the wrapper's hidden STARTUPINFO.
    // These explicit subsequent calls affect only the fixture's normal HWND.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
        let _ = ShowWindow(host.window.get(), SW_SHOWNORMAL);
    }
    for modal in [false, true] {
        let at = if modal { 300 } else { 100 };
        accept_scene(host, at, false);
        DRIVER.with(|state| {
            *state.borrow_mut() = Some(Driver {
                modal,
                deadline: Instant::now() + Duration::from_secs(15),
                phase: Phase::Baseline,
                app_path: app_path.clone(),
                observations: Rc::clone(&observations),
                child: None,
                expected_model: None,
                before_revision: 0,
                failure: None,
            });
        });
        // SAFETY: This test timer is scoped to the live Host UI thread and is
        // retired before its Driver or owner can be released.
        if unsafe { SetTimer(Some(host.window.get()), TIMER, 20, Some(tick)) } == 0 {
            DRIVER.with(|state| state.borrow_mut().take());
            return Err(Error::from_thread());
        }
        let result = if modal {
            crate::app::settings::show(
                &host.app,
                host.window.get(),
                crate::app::settings::Page::Map,
            )
        } else {
            pump(host)
        };
        // SAFETY: No callback can run concurrently on this same UI thread.
        unsafe {
            let _ = KillTimer(Some(host.window.get()), TIMER);
        }
        let mut driver = DRIVER
            .with(|state| state.borrow_mut().take())
            .ok_or_else(|| incomplete("Native foreground driver was retired unexpectedly"))?;
        result?;
        if let Some(error) = driver.failure.take() {
            return Err(error);
        }
        require(
            matches!(driver.phase, Phase::Done),
            "Foreground journey ended before its assertions",
        )?;
    }
    Ok(())
}

fn accept_scene(host: &Host, at: u64, newer: bool) -> std::sync::Arc<crate::map::model::Model> {
    let mut observations = facts::observations();
    for observation in &mut observations {
        observation.rates = Some(Rates {
            received: if newer { 8192.0 } else { 4096.0 },
            sent: 2048.0,
        });
    }
    if at >= 200 {
        observations.retain(|observation| observation.id.0 != 10);
    }
    let previous = host.app.exploration_rows();
    let snapshot = facts::facts(&host.app, observations, at, 1, previous.as_deref());
    facts::scene(&host.app, &snapshot, false)
}

fn pump(host: &Host) -> Result<()> {
    loop {
        if DRIVER.with(|state| {
            state
                .borrow()
                .as_ref()
                .is_some_and(|driver| matches!(driver.phase, Phase::Done))
        }) {
            return Ok(());
        }
        let mut message = MSG::default();
        // SAFETY: The normal Host's native message order is preserved. Its
        // 20ms scoped timer bounds progress; no worker or foreign HWND is pumped.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(incomplete("Host retired during foreground verification"));
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

unsafe extern "system" fn tick(window: HWND, _: u32, _: usize, _: u32) {
    // Taking the owned state releases its RefCell borrow before native calls.
    // Nested modal/capture delivery sees no active driver and cannot reenter it.
    let Some(mut driver) = DRIVER.with(|state| state.borrow_mut().take()) else {
        return;
    };
    if !matches!(driver.phase, Phase::Done) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: The timer belongs to Host's live HWND, whose stable context
            // is retired only after this callback returns on the same UI thread.
            let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const Host;
            // SAFETY: The same scoped timer cannot outlive this Host context.
            let host =
                unsafe { pointer.as_ref() }.ok_or_else(|| incomplete("Host context retired"))?;
            driver.step(host)
        }))
        .unwrap_or_else(|_| Err(incomplete("Foreground callback panicked")));
        if let Err(error) = result {
            driver.failure = Some(error);
            driver.phase = Phase::Done;
        }
        if matches!(driver.phase, Phase::Done) && driver.modal {
            // Queue Cancel only after assertions return, outside dialog borrows.
            if let Ok(Some(dialog)) = settings_window(window) {
                // SAFETY: Cancel targets only the still-owned Settings dialog.
                unsafe {
                    let _ = PostMessageW(Some(dialog), WM_COMMAND, WPARAM(2), LPARAM(0));
                }
            }
        }
    }
    DRIVER.with(|state| *state.borrow_mut() = Some(driver));
}

impl Driver {
    fn step(&mut self, host: &Host) -> Result<()> {
        require(
            Instant::now() < self.deadline,
            "Real foreground/Present verification timed out; acceptance is incomplete",
        )?;
        let resume = if self.modal {
            settings_window(host.window.get())?
                .ok_or_else(|| incomplete("Settings modal HWND is unavailable"))?
        } else {
            host.window.get()
        };
        match self.phase {
            Phase::Baseline => {
                // SAFETY: Explicit activation concerns only our normal/Settings
                // HWND. Denied foreground permission is never bypassed.
                unsafe {
                    let _ = SetForegroundWindow(resume);
                }
                // SAFETY: Foreground identity is a non-messaging native query.
                if unsafe { GetForegroundWindow() } != resume
                    || host.saver_covered.get()
                    || !host.can_draw()
                {
                    return Ok(());
                }
                let before = self.observations.attempts.get();
                repaint(host.map_window.get())?;
                require(
                    self.observations.attempts.get() > before,
                    "Positive normal Present control failed; device state is inconclusive",
                )?;
                let frame = self
                    .observations
                    .latest
                    .get()
                    .ok_or_else(|| incomplete("No successful baseline Present was observed"))?;
                self.before_revision = frame.revision;
                self.child = Some(SaverChild::launch(&self.app_path)?);
                self.phase = Phase::Cover;
            }
            Phase::Cover => {
                let child = self.child.as_mut().unwrap();
                require(
                    !child.finished()?,
                    "Owned /s child exited before covering Host",
                )?;
                let windows = child.windows()?;
                let Some(&first) = windows.first() else {
                    return Ok(());
                };
                // SAFETY: This handle was verified against the exact owned child
                // PID and saver class. No input hooks or focus-policy bypasses.
                unsafe {
                    let _ = SetForegroundWindow(first);
                }
                // SAFETY: Compare foreground with the verified child windows.
                if !windows.contains(&unsafe { GetForegroundWindow() }) || !host.saver_covered.get()
                {
                    return Ok(());
                }
                require(
                    !host.can_draw(),
                    "Real saver foreground did not close Host's drawing gate",
                )?;
                let attempts = self.observations.attempts.get();
                self.expected_model =
                    Some(accept_scene(host, if self.modal { 400 } else { 200 }, true));
                repaint(host.map_window.get())?;
                require(
                    self.observations.attempts.get() == attempts,
                    "Host attempted Present after saver coverage",
                )?;
                self.phase = Phase::Hold {
                    since: Instant::now(),
                    attempts,
                };
            }
            Phase::Hold { since, attempts } => {
                let child = self.child.as_mut().unwrap();
                // SAFETY: Foreground identity never invokes a foreign procedure.
                let foreground = unsafe { GetForegroundWindow() };
                require(
                    !child.finished()? && child.windows()?.contains(&foreground),
                    "Owned saver did not remain foreground during the measurement",
                )?;
                require(
                    host.saver_covered.get() && !host.can_draw(),
                    "Covered Host reopened its drawing gate",
                )?;
                repaint(host.map_window.get())?;
                require(
                    self.observations.attempts.get() == attempts,
                    "Covered Host attempted another Present",
                )?;
                if since.elapsed() >= HOLD {
                    child.close()?;
                    self.phase = Phase::Resume {
                        attempts,
                        minimum_age: Duration::from_secs(if self.modal { 400 } else { 200 }) + HOLD,
                    };
                }
            }
            Phase::Resume {
                attempts,
                minimum_age,
            } => {
                let child = self.child.as_mut().unwrap();
                if !child.finished()? {
                    return Ok(());
                }
                // SAFETY: Resume activation addresses only our normal/Settings HWND.
                unsafe {
                    let _ = SetForegroundWindow(resume);
                }
                // SAFETY: Non-messaging foreground query on the fixture UI thread.
                if unsafe { GetForegroundWindow() } != resume
                    || host.saver_covered.get()
                    || !host.can_draw()
                {
                    return Ok(());
                }
                repaint(host.map_window.get())?;
                let frame = self
                    .observations
                    .latest
                    .get()
                    .ok_or_else(|| incomplete("No resumed successful frame was observed"))?;
                let expected =
                    std::sync::Arc::as_ptr(self.expected_model.as_ref().unwrap()) as usize;
                require(
                    self.observations.attempts.get() > attempts
                        && frame.target == host.map_window.get(),
                    "Positive resumed Present control failed; device state is inconclusive",
                )?;
                require(
                    frame.revision > self.before_revision
                        && frame.model == expected
                        && frame.at >= minimum_age,
                    "Resumed frame did not use the latest accepted model and current observation age",
                )?;
                println!(
                    "NETMAP_FOREGROUND_CASE {{\"modal\":{},\"covered_attempts\":{},\"resumed_attempts\":{},\"revision\":{},\"age_ms\":{}}}",
                    self.modal,
                    attempts,
                    self.observations.attempts.get(),
                    frame.revision,
                    frame.at.as_millis()
                );
                self.phase = Phase::Done;
            }
            Phase::Done => {}
        }
        Ok(())
    }
}

fn repaint(window: HWND) -> Result<()> {
    // SAFETY: A normal owned-window invalidation delivers the actual Host paint
    // path even while covered; it never fabricates activity/foreground messages.
    unsafe {
        InvalidateRect(Some(window), None, false).ok()?;
        UpdateWindow(window).ok()
    }
}

struct WindowSearch {
    process: u32,
    owner: Option<HWND>,
    windows: Vec<HWND>,
}

unsafe extern "system" fn find_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: Only the synchronous enumeration owns this stack search record.
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch) };
    let mut process = 0;
    // SAFETY: Read native metadata only after matching the exact owned PID.
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut process));
        if process != search.process {
            return BOOL(1);
        }
        let matches = if let Some(owner) = search.owner {
            GetWindow(window, GW_OWNER).is_ok_and(|actual| actual == owner)
                && GetDlgItem(Some(window), ids::IDC_PAGES as i32).is_ok()
        } else {
            let mut class = [0u16; 64];
            let length = GetClassNameW(window, &mut class).max(0) as usize;
            class[..length]
                .iter()
                .copied()
                .eq("NetMap.ScreenSaver".encode_utf16())
        };
        if matches {
            search.windows.push(window);
        }
    }
    BOOL(1)
}

fn settings_window(owner: HWND) -> Result<Option<HWND>> {
    let mut search = WindowSearch {
        // SAFETY: Query only this fixture's process identity.
        process: unsafe { GetCurrentProcessId() },
        owner: Some(owner),
        windows: Vec::new(),
    };
    // SAFETY: Enumerate only this UI thread's owned top-level windows.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(find_window),
            LPARAM((&mut search as *mut WindowSearch) as isize),
        )
        .ok()?;
    }
    Ok(search.windows.first().copied())
}

struct SaverChild {
    process: Child,
    exited: bool,
}

impl SaverChild {
    fn launch(path: &OsString) -> Result<Self> {
        let process = Command::new(path)
            .arg("/s")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| incomplete(&format!("Start owned production /s child: {error}")))?;
        let child = Self {
            process,
            exited: false,
        };
        let (mut started, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        // SAFETY: std owns this exact child process handle through child cleanup.
        unsafe {
            GetProcessTimes(
                HANDLE(child.process.as_raw_handle()),
                &mut started,
                &mut exited,
                &mut kernel,
                &mut user,
            )?;
        }
        let started = (u64::from(started.dwHighDateTime) << 32) | u64::from(started.dwLowDateTime);
        let path = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        eprintln!(
            "NETMAP_OWNED_SAVER {{\"pid\":{},\"started\":{},\"path\":\"{}\"}}",
            child.process.id(),
            started,
            path
        );
        let _ = std::io::stderr().flush();
        Ok(child)
    }

    fn windows(&self) -> Result<Vec<HWND>> {
        let mut search = WindowSearch {
            process: self.process.id(),
            owner: None,
            windows: Vec::new(),
        };
        // SAFETY: The callback reads metadata only after matching the owned PID.
        unsafe {
            EnumWindows(
                Some(find_window),
                LPARAM((&mut search as *mut WindowSearch) as isize),
            )?;
        }
        Ok(search.windows)
    }

    fn finished(&mut self) -> Result<bool> {
        if self.exited {
            return Ok(true);
        }
        if let Some(status) = self
            .process
            .try_wait()
            .map_err(|error| incomplete(&error.to_string()))?
        {
            self.exited = true;
            require(status.success(), "Owned production /s child failed")?;
        }
        Ok(self.exited)
    }

    fn close(&self) -> Result<()> {
        for window in self.windows()? {
            // SAFETY: Post only to verified windows of the exact owned child.
            unsafe {
                PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))?;
            }
        }
        Ok(())
    }
}

impl Drop for SaverChild {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.close();
            // No UI-thread join: terminate only this still-owned child if a
            // failed check cannot cooperatively dismiss it. Wrapper verifies
            // PID/path/creation time before its own timeout cleanup as well.
            let _ = self.process.kill();
        }
    }
}

fn require(value: bool, reason: &str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(incomplete(reason))
    }
}

fn incomplete(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}
