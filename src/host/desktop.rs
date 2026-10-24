use std::{marker::PhantomData, mem::size_of, rc::Rc};
use windows::{
    Win32::{
        Foundation::{
            E_FAIL, ERROR_SUCCESS, GetLastError, HWND, LPARAM, POINT, RECT, SetLastError, WPARAM,
        },
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW, MapWindowPoints,
        },
        UI::WindowsAndMessaging::*,
    },
    core::{BOOL, Error, HRESULT, PCWSTR, Result, w},
};

#[derive(Clone, Debug)]
pub(crate) struct Monitor {
    pub(crate) device: String,
    pub(crate) rect: RECT,
    pub(crate) primary: bool,
}

/// A discovery result can cross Work's reply boundary. Native handles are
/// borrowed identities, never ownership of Explorer's windows.
#[derive(Clone, Debug)]
pub(crate) struct Attachment {
    parent: Window,
    icons: Window,
    shell: Window,
}

#[derive(Clone, Debug)]
struct Window {
    handle: usize,
    process: u32,
    thread: u32,
    class: Vec<u16>,
    context: isize,
}

impl Window {
    fn read(window: HWND) -> Result<Self> {
        let mut process = 0;
        let mut class = [0; 256];
        // SAFETY: These queries borrow a window; neither accesses its process
        // memory nor sends a message to its procedure.
        unsafe {
            let thread = GetWindowThreadProcessId(window, Some(&mut process));
            let length = GetClassNameW(window, &mut class);
            if thread == 0 || length == 0 {
                return Err(unavailable("The desktop window is no longer available"));
            }
            Ok(Self {
                handle: window.0 as usize,
                process,
                thread,
                class: class[..length as usize].to_vec(),
                context: GetWindowLongPtrW(window, GWLP_USERDATA),
            })
        }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.handle as *mut _)
    }

    fn valid(&self) -> bool {
        Self::read(self.hwnd()).is_ok_and(|current| {
            self.process == current.process
                && self.thread == current.thread
                && self.class == current.class
                && self.context == current.context
        })
    }
}

impl Attachment {
    pub(crate) fn valid(&self) -> bool {
        // SAFETY: GetShellWindow is a non-messaging identity query.
        self.parent.valid()
            && self.icons.valid()
            && self.shell.valid()
            && unsafe { GetShellWindow() } == self.shell.hwnd()
    }
}

/// Call through Work, never from a window procedure: Explorer may be hung.
/// Progman's private message and WorkerW topology are not a supported Windows
/// wallpaper API; the UI revalidates this result immediately before attaching.
pub(crate) fn discover() -> Result<Attachment> {
    // SAFETY: Only the discovered shell receives the bounded, pointer-free
    // request. No broadcast and no Explorer window mutation/destruction occur.
    unsafe {
        let shell = GetShellWindow();
        let shell_identity =
            Window::read(shell).map_err(|error| native("Read Explorer shell identity", error))?;
        let progman = FindWindowW(w!("Progman"), PCWSTR::null())
            .map_err(|_| unavailable("Find Explorer Progman window: Progman was not found"))?;
        if Window::read(progman)
            .map_err(|error| native("Read Progman identity", error))?
            .process
            != shell_identity.process
        {
            return Err(unavailable("Explorer's desktop host could not be verified"));
        }
        if SendMessageTimeoutW(
            progman,
            0x052c,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT | SMTO_BLOCK,
            1_000,
            None,
        )
        .0 == 0
        {
            return Err(unavailable(
                "Explorer did not answer the desktop attachment request",
            ));
        }
        let mut search = Search {
            process: shell_identity.process,
            found: None,
        };
        EnumWindows(
            Some(find_host),
            LPARAM((&mut search as *mut Search) as isize),
        )
        .map_err(|error| native("Enumerate Explorer desktop windows", error))?;
        let (parent, icons) = search.found.ok_or_else(|| {
            unavailable("Explorer did not provide a background window behind the desktop icons")
        })?;
        let attachment = Attachment {
            parent: Window::read(parent)
                .map_err(|error| native("Read background WorkerW identity", error))?,
            icons: Window::read(icons)
                .map_err(|error| native("Read desktop icon window identity", error))?,
            shell: shell_identity,
        };
        if !attachment.valid() {
            return Err(unavailable(
                "Explorer recreated the desktop during attachment discovery",
            ));
        }
        Ok(attachment)
    }
}

struct Search {
    process: u32,
    found: Option<(HWND, HWND)>,
}

unsafe extern "system" fn find_host(window: HWND, value: LPARAM) -> BOOL {
    // SAFETY: discover owns this stack record through synchronous EnumWindows.
    let search = unsafe { &mut *(value.0 as *mut Search) };
    if search.found.is_some()
        || !Window::read(window).is_ok_and(|identity| identity.process == search.process)
    {
        return BOOL(1);
    }
    // SAFETY: FindWindowEx only enumerates native children/siblings; absent
    // classes are an ordinary unsupported topology, not a thread error.
    unsafe {
        if let Ok(icons) = FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), PCWSTR::null())
        {
            // Recent Explorer builds can put WorkerW beside DefView inside
            // Progman. The older arrangement has a top-level WorkerW below it.
            let candidate = FindWindowExW(Some(window), Some(icons), w!("WorkerW"), PCWSTR::null())
                .or_else(|_| FindWindowExW(None, Some(window), w!("WorkerW"), PCWSTR::null()));
            if let Ok(parent) = candidate
                && Window::read(parent).is_ok_and(|identity| identity.process == search.process)
                && FindWindowExW(Some(parent), None, w!("SHELLDLL_DefView"), PCWSTR::null())
                    .is_err()
            {
                search.found = Some((parent, icons));
            }
        }
    }
    BOOL(1)
}

pub(crate) fn monitors() -> Result<Vec<Monitor>> {
    let mut found: Result<Vec<Monitor>> = Ok(Vec::new());
    // SAFETY: The callback receives this live stack-owned result only during
    // enumeration; no native monitor handle escapes the call.
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(read_monitor),
            LPARAM((&mut found as *mut Result<Vec<Monitor>>) as isize),
        )
        .ok()
        .map_err(|error| native("Enumerate connected monitors", error))?;
    }
    let monitors = found?;
    if monitors.is_empty() {
        return Err(unavailable("Windows did not report an attached display"));
    }
    Ok(monitors)
}

unsafe extern "system" fn read_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    value: LPARAM,
) -> BOOL {
    // SAFETY: monitors owns this result through synchronous enumeration.
    let result = unsafe { &mut *(value.0 as *mut Result<Vec<Monitor>>) };
    if let Ok(found) = result {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: cbSize identifies this writable SDK MONITORINFOEXW record.
        if !unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) }.as_bool() {
            *result = Err(native(
                "Read connected monitor information",
                Error::from_thread(),
            ));
        } else {
            let length = info
                .szDevice
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(info.szDevice.len());
            found.push(Monitor {
                device: String::from_utf16_lossy(&info.szDevice[..length]),
                rect: info.monitorInfo.rcMonitor,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
    }
    BOOL(1)
}

/// UI-thread-owned attachment. Drop restores only our map child, never an
/// Explorer window. Host must retire it before destroying the normal parent.
pub(crate) struct Attached {
    map: Window,
    original_parent: Window,
    attachment: Attachment,
    style: isize,
    extended: isize,
    bounds: RECT,
    restored: bool,
    ui_thread: PhantomData<Rc<()>>,
}

pub(crate) fn attach(map: HWND, attachment: &Attachment, monitor: &Monitor) -> Result<Attached> {
    if !attachment.valid() {
        return Err(unavailable(
            "Explorer's desktop host changed; try desktop mode again",
        ));
    }
    // SAFETY: Called on the map's owning UI thread, with no model borrow held.
    // The existing map is a child of the still-live normal NetMap window.
    unsafe {
        let parent = GetParent(map).map_err(|error| native("Read normal map parent", error))?;
        let mut bounds = RECT::default();
        GetWindowRect(map, &mut bounds).map_err(|error| native("Read normal map bounds", error))?;
        bounds = map_rect(bounds, None, Some(parent))
            .map_err(|error| native("Map normal bounds into original parent", error))?;
        let mut attached = Attached {
            map: Window::read(map).map_err(|error| native("Read owned map identity", error))?,
            original_parent: Window::read(parent)
                .map_err(|error| native("Read original parent identity", error))?,
            attachment: attachment.clone(),
            style: GetWindowLongPtrW(map, GWL_STYLE),
            extended: GetWindowLongPtrW(map, GWL_EXSTYLE),
            bounds,
            restored: false,
            ui_thread: PhantomData,
        };
        let result = (|| {
            set_style(
                map,
                GWL_STYLE,
                (attached.style & !((WS_POPUP | WS_TABSTOP).0 as isize))
                    | (WS_CHILD | WS_DISABLED).0 as isize,
            )
            .map_err(|error| native("Set passive desktop map style", error))?;
            set_style(
                map,
                GWL_EXSTYLE,
                attached.extended
                    | (WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW).0 as isize,
            )
            .map_err(|error| native("Set nonactivating desktop map extended style", error))?;
            SetParent(map, Some(attachment.parent.hwnd()))
                .map_err(|error| native("Attach map to Explorer WorkerW", error))?;
            let area = map_rect(monitor.rect, None, Some(attachment.parent.hwnd()))
                .map_err(|error| native("Map monitor bounds into Explorer WorkerW", error))?;
            SetWindowPos(
                map,
                Some(HWND_BOTTOM),
                area.left,
                area.top,
                area.right - area.left,
                area.bottom - area.top,
                SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
            )
            .map_err(|error| native("Position desktop map behind icons", error))?;
            if !attached.valid() {
                return Err(unavailable(
                    "Explorer changed or rejected the desktop map attachment",
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            // Rollback is also repeated by Drop if this attempt fails.
            let _ = attached.restore();
            return Err(error);
        }
        Ok(attached)
    }
}

impl Attached {
    pub(crate) fn valid(&self) -> bool {
        self.map.valid() && self.attachment.valid()
            // SAFETY: Identity checks above precede this non-messaging query.
            && unsafe { GetParent(self.map.hwnd()) }.is_ok_and(|parent| parent == self.attachment.parent.hwnd())
    }

    pub(crate) fn detach(mut self) -> Result<()> {
        self.restore()
    }

    fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        if !self.map.valid() {
            self.restored = true;
            return Ok(());
        }
        if !self.original_parent.valid() {
            return Err(unavailable(
                "The normal map window was destroyed before desktop detachment",
            ));
        }
        let map = self.map.hwnd();
        // SAFETY: Both windows are our UI-thread-owned original identities.
        // Never reparent into an Explorer or reused normal-window handle.
        unsafe {
            SetParent(map, Some(self.original_parent.hwnd()))
                .map_err(|error| native("Restore normal map parent", error))?;
            set_style(map, GWL_STYLE, self.style)
                .map_err(|error| native("Restore normal map style", error))?;
            set_style(map, GWL_EXSTYLE, self.extended)
                .map_err(|error| native("Restore normal map extended style", error))?;
            SetWindowPos(
                map,
                None,
                self.bounds.left,
                self.bounds.top,
                self.bounds.right - self.bounds.left,
                self.bounds.bottom - self.bounds.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
            .map_err(|error| native("Restore normal map bounds", error))?;
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for Attached {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn set_style(window: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> Result<()> {
    // SAFETY: Only the caller's own window styles are changed. Style messages
    // can reenter our procedure and change last-error even after a successful
    // zero-valued previous style. Verify the requested 32-bit native state;
    // preserve the call's error only when that state was not established.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        let previous = SetWindowLongPtrW(window, index, value);
        let error = GetLastError();
        if IsWindow(Some(window)).as_bool()
            && GetWindowLongPtrW(window, index) as u32 == value as u32
        {
            return Ok(());
        }
        if previous == 0 && error != ERROR_SUCCESS {
            return Err(Error::from_hresult(HRESULT::from_win32(error.0)));
        }
    }
    Err(unavailable(
        "Windows did not retain the requested map window style",
    ))
}

fn map_rect(rect: RECT, from: Option<HWND>, to: Option<HWND>) -> Result<RECT> {
    let mut points = [
        POINT {
            x: rect.left,
            y: rect.top,
        },
        POINT {
            x: rect.right,
            y: rect.bottom,
        },
    ];
    // SAFETY: Both points belong to this live stack record. A zero mapping can
    // mean no translation; clear/check last error rather than rejecting it.
    unsafe {
        SetLastError(ERROR_SUCCESS);
        if MapWindowPoints(from, to, &mut points) == 0 && GetLastError() != ERROR_SUCCESS {
            return Err(Error::from_thread());
        }
    }
    Ok(RECT {
        left: points[0].x,
        top: points[0].y,
        right: points[1].x,
        bottom: points[1].y,
    })
}

fn unavailable(message: &str) -> Error {
    Error::new(E_FAIL, message)
}

fn native(operation: &str, error: Error) -> Error {
    Error::new(error.code(), format!("{operation}: {error}"))
}
