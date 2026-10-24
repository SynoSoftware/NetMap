use std::mem::size_of;
use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND, LPARAM, POINT, WPARAM},
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::{Error, Result, w},
};

#[derive(Clone, Copy)]
pub(crate) struct Commands {
    pub(crate) open: u16,
    pub(crate) desktop: u16,
    pub(crate) exit: u16,
}

/// UI-thread-owned icon. The caller retains the owner HWND and the borrowed
/// stock icon until this value is dropped; Explorer owns only its displayed copy.
pub(crate) struct Tray {
    data: NOTIFYICONDATAW,
    taskbar_created: u32,
}

impl Tray {
    pub(crate) fn new(owner: HWND, callback: u32, icon: HICON) -> Result<Self> {
        // SAFETY: This registers a process-wide message name, no callback runs.
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if taskbar_created == 0 {
            return Err(Error::from_thread());
        }
        let mut tray = Self {
            data: NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: owner,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
                uCallbackMessage: callback,
                hIcon: icon,
                ..Default::default()
            },
            taskbar_created,
        };
        text(&mut tray.data.szTip, "NetMap");
        tray.readd()?;
        Ok(tray)
    }

    pub(crate) fn taskbar_created(&self) -> u32 {
        self.taskbar_created
    }

    /// Call after TaskbarCreated: Explorer has discarded the previous icon.
    pub(crate) fn readd(&self) -> Result<()> {
        // SAFETY: The callback HWND and borrowed HICON outlive this record.
        unsafe {
            if !Shell_NotifyIconW(NIM_ADD, &self.data).as_bool() {
                return Err(failed(
                    "Windows could not add NetMap to the notification area",
                ));
            }
            let mut version = self.data;
            version.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            if !Shell_NotifyIconW(NIM_SETVERSION, &version).as_bool() {
                let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
                return Err(failed(
                    "Windows could not initialize NetMap's notification icon",
                ));
            }
        }
        Ok(())
    }

    /// The host decodes version-4 callbacks: LOWORD(lParam) is the event,
    /// HIWORD(lParam) the icon ID. NIN_SELECT/NIN_KEYSELECT open the map;
    /// WM_CONTEXTMENU opens this native menu, including keyboard invocation.
    pub(crate) fn popup(&self, commands: Commands, desktop_enabled: bool) -> Result<Option<u16>> {
        // SAFETY: A native menu owns its items and modal navigation. No model
        // borrow may span this call; command availability is rechecked by Host.
        unsafe {
            let menu = CreatePopupMenu()?;
            let result = (|| {
                AppendMenuW(menu, MF_STRING, usize::from(commands.open), w!("&Open map"))?;
                AppendMenuW(
                    menu,
                    MF_STRING
                        | if desktop_enabled {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                    usize::from(commands.desktop),
                    w!("Use as &desktop background"),
                )?;
                AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                AppendMenuW(menu, MF_STRING, usize::from(commands.exit), w!("E&xit"))?;
                SetMenuDefaultItem(menu, u32::from(commands.open), 0)?;
                let mut point = POINT::default();
                GetCursorPos(&mut point)?;
                let _ = SetForegroundWindow(self.data.hWnd);
                let command = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    None,
                    self.data.hWnd,
                    None,
                )
                .0 as u16;
                // A posted null message is required for reliable dismissal of
                // subsequent notification-icon menus after foreground changes.
                let _ = PostMessageW(Some(self.data.hWnd), WM_NULL, WPARAM(0), LPARAM(0));
                let _ = Shell_NotifyIconW(NIM_SETFOCUS, &self.data);
                Ok((command != 0).then_some(command))
            })();
            let _ = DestroyMenu(menu);
            result
        }
    }

    pub(crate) fn error(&self, message: &str) -> Result<()> {
        let mut data = self.data;
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = NIIF_ERROR | NIIF_RESPECT_QUIET_TIME;
        text(&mut data.szInfoTitle, "NetMap desktop background");
        text(&mut data.szInfo, message);
        // SAFETY: The shell copies both bounded, terminated stack strings.
        if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool() {
            return Err(failed(
                "Windows could not display NetMap's background error",
            ));
        }
        Ok(())
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: Delete only the icon identified by our owner HWND and ID.
        // Host drops Tray before destroying that HWND; HICON remains borrowed.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
        }
    }
}

fn text<const N: usize>(destination: &mut [u16; N], value: &str) {
    // Keep surrogate pairs intact if a native text field needs truncation.
    destination.fill(0);
    let mut length = 0;
    for character in value.chars().take_while(|character| *character != '\0') {
        let mut encoded = [0; 2];
        let units = character.encode_utf16(&mut encoded);
        if length + units.len() >= N {
            break;
        }
        destination[length..length + units.len()].copy_from_slice(units);
        length += units.len();
    }
}

fn failed(message: &str) -> Error {
    Error::new(E_FAIL, message)
}
