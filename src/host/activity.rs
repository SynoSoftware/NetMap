use windows::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WPARAM},
    System::{
        Power::{
            HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
            UnregisterPowerSettingNotification,
        },
        RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
            WTSUnRegisterSessionNotification,
        },
        SystemServices::GUID_SESSION_DISPLAY_STATUS,
    },
    UI::WindowsAndMessaging::*,
};

pub(super) const FOREGROUND_CHANGED: u32 = WM_APP + 14;

thread_local! {
    // WinEvent has no callback context. This is the borrowed delivery HWND for
    // this UI thread's one registration, not application or feature state.
    static FOREGROUND_TARGET: std::cell::Cell<HWND> = const { std::cell::Cell::new(HWND(std::ptr::null_mut())) };
}

/// Out-of-context accessibility notifications return on the registering UI
/// thread. This observes foreground changes; it never intercepts input.
pub(super) struct Foreground {
    hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    window: HWND,
    _ui_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Foreground {
    pub fn new(window: HWND) -> Option<Self> {
        if FOREGROUND_TARGET.with(|target| !target.get().is_invalid()) {
            return None;
        }
        FOREGROUND_TARGET.with(|target| target.set(window));
        // SAFETY: The callback contains no application pointer, posts only to
        // its registering thread and is removed before the message loop ends.
        let hook = unsafe {
            windows::Win32::UI::Accessibility::SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            FOREGROUND_TARGET.with(|target| target.set(HWND::default()));
            return None;
        }
        Some(Self {
            hook,
            window,
            _ui_thread: std::marker::PhantomData,
        })
    }
}

impl Drop for Foreground {
    fn drop(&mut self) {
        // SAFETY: Retire this thread's own notification registration.
        unsafe {
            let _ = windows::Win32::UI::Accessibility::UnhookWinEvent(self.hook);
        }
        FOREGROUND_TARGET.with(|target| {
            if target.get() == self.window {
                target.set(HWND::default());
            }
        });
    }
}

unsafe extern "system" fn foreground_changed(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    _window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    // Window delivery survives Settings' nested native modal message pump.
    FOREGROUND_TARGET.with(|target| {
        let window = target.get();
        if !window.is_invalid() {
            // SAFETY: This registration retires delivery before its owned HWND.
            unsafe {
                let _ = PostMessageW(Some(window), FOREGROUND_CHANGED, WPARAM(0), LPARAM(0));
            }
        }
    });
}

pub(super) fn saver_foreground() -> bool {
    let mut class = [0u16; 64];
    // SAFETY: Non-messaging identity queries; no foreign process data or input.
    let length = unsafe { GetClassNameW(GetForegroundWindow(), &mut class) }.max(0) as usize;
    class[..length]
        .iter()
        .copied()
        .eq("NetMap.ScreenSaver".encode_utf16())
}

/// Native registrations and independent reasons the presentation cannot draw.
/// Collection and monotonic observation ages are unaffected by these facts.
pub(super) struct Activity {
    window: HWND,
    power: Option<HPOWERNOTIFY>,
    registered_session: bool,
    display_on: bool,
    session_available: bool,
    suspended: bool,
    warning: Option<String>,
}

impl Activity {
    pub fn new(window: HWND) -> Self {
        let mut activity = Self {
            window,
            power: None,
            registered_session: false,
            display_on: true,
            session_available: true,
            suspended: false,
            warning: None,
        };
        // SAFETY: This UI-thread owner unregisters before its HWND retires.
        // Power registration delivers the session's current display state.
        unsafe {
            match WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) {
                Ok(()) => activity.registered_session = true,
                Err(error) => {
                    activity.warning =
                        Some(format!("Session change monitoring unavailable: {error}."))
                }
            }
            match RegisterPowerSettingNotification(
                HANDLE(window.0),
                &GUID_SESSION_DISPLAY_STATUS,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            ) {
                Ok(power) => activity.power = Some(power),
                Err(error) => {
                    let warning = activity.warning.get_or_insert_with(String::new);
                    if !warning.is_empty() {
                        warning.push(' ');
                    }
                    warning.push_str(&format!("Display change monitoring unavailable: {error}."));
                }
            }
        }
        activity
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub fn available(&self) -> bool {
        self.display_on && self.session_available && !self.suspended
    }

    pub fn message(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) {
        match message {
            WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
                WTS_SESSION_LOCK
                | WTS_CONSOLE_DISCONNECT
                | WTS_REMOTE_DISCONNECT
                | WTS_SESSION_LOGOFF => self.session_available = false,
                WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT
                | WTS_SESSION_LOGON => self.session_available = true,
                _ => {}
            },
            WM_POWERBROADCAST => match wparam.0 as u32 {
                PBT_APMSUSPEND => self.suspended = true,
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND | PBT_APMRESUMECRITICAL => {
                    self.suspended = false
                }
                PBT_POWERSETTINGCHANGE if lparam.0 != 0 => {
                    // SAFETY: Windows supplies this live variable-length record
                    // for WM_POWERBROADCAST. Validate the GUID and DWORD length
                    // before reading its possibly unaligned payload.
                    unsafe {
                        let setting = &*(lparam.0 as *const POWERBROADCAST_SETTING);
                        if setting.PowerSetting == GUID_SESSION_DISPLAY_STATUS
                            && setting.DataLength == 4
                        {
                            let state =
                                std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>());
                            if state <= 2 {
                                self.display_on = state != 0;
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        // SAFETY: Registrations belong exclusively to this still-live HWND.
        unsafe {
            if let Some(power) = self.power.take() {
                let _ = UnregisterPowerSettingNotification(power);
            }
            if self.registered_session {
                let _ = WTSUnRegisterSessionNotification(self.window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_optional_registrations_keep_presentation_available_with_a_warning() {
        let activity = Activity::new(HWND::default());
        assert!(activity.available());
        assert!(activity.warning().is_some());
    }

    #[test]
    fn session_resume_does_not_override_an_off_display_or_suspended_system() {
        let mut activity = Activity {
            window: HWND::default(),
            power: None,
            registered_session: false,
            display_on: false,
            session_available: true,
            suspended: false,
            warning: None,
        };
        activity.message(
            WM_WTSSESSION_CHANGE,
            WPARAM(WTS_SESSION_LOCK as usize),
            LPARAM(0),
        );
        activity.message(
            WM_WTSSESSION_CHANGE,
            WPARAM(WTS_SESSION_UNLOCK as usize),
            LPARAM(0),
        );
        assert!(!activity.available());
        activity.display_on = true;
        activity.message(
            WM_POWERBROADCAST,
            WPARAM(PBT_APMSUSPEND as usize),
            LPARAM(0),
        );
        activity.message(
            WM_WTSSESSION_CHANGE,
            WPARAM(WTS_SESSION_UNLOCK as usize),
            LPARAM(0),
        );
        assert!(!activity.available());
        activity.message(
            WM_POWERBROADCAST,
            WPARAM(PBT_APMRESUMEAUTOMATIC as usize),
            LPARAM(0),
        );
        assert!(activity.available());
        activity.message(
            WM_WTSSESSION_CHANGE,
            WPARAM(WTS_REMOTE_DISCONNECT as usize),
            LPARAM(0),
        );
        assert!(!activity.available());
    }
}
