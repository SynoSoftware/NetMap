use super::activity::Activity;
use crate::{
    app::{Application, REFRESH_MESSAGE},
    map::Frame,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, EndPaint, EnumDisplayMonitors, HDC, HMONITOR, InvalidateRect, PAINTSTRUCT,
            SC_SCREENSAVE,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
    },
    core::{BOOL, Error, Result, w},
};

const FRAME_TIMER: usize = 1;
const PREVIEW_TIMER: usize = 2;
const DISPLAYS_CHANGED: u32 = WM_APP + 31;

#[derive(Clone, Copy)]
pub(crate) enum Mode {
    FullScreen,
    Preview(HWND),
}

struct Saver {
    app: Rc<Application>,
    mode: Mode,
    instance: HINSTANCE,
    windows: RefCell<Vec<HWND>>,
    first: Cell<HWND>,
    activity: RefCell<Option<Activity>>,
    mouse_origin: Cell<POINT>,
    cursor: HCURSOR,
    display_change_pending: Cell<bool>,
    running: Cell<bool>,
    closing: Cell<bool>,
    failure: RefCell<Option<Error>>,
}

pub(crate) fn run(app: Rc<Application>, mode: Mode) -> Result<()> {
    let _apartment = super::accessibility::Apartment::new(
        windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
    )?;
    // SAFETY: Only the current executable's module handle is requested.
    let instance = unsafe { GetModuleHandleW(None)? }.into();
    // SAFETY: This class has no shared mutable state. Each HWND borrows the
    // stable run-owned Saver until every owned window has been destroyed.
    unsafe {
        let class = WNDCLASSW {
            hInstance: instance,
            lpfnWndProc: Some(window_proc),
            lpszClassName: w!("NetMap.ScreenSaver"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(Error::from_thread());
        }
    }
    let saver = Box::new(Saver {
        app,
        mode,
        instance,
        windows: RefCell::new(Vec::new()),
        first: Cell::new(HWND::default()),
        activity: RefCell::new(None),
        mouse_origin: Cell::new(POINT::default()),
        // SAFETY: Borrow the current cursor; no ownership is transferred.
        cursor: unsafe { GetCursor() },
        display_change_pending: Cell::new(false),
        running: Cell::new(false),
        closing: Cell::new(false),
        failure: RefCell::new(None),
    });
    match mode {
        Mode::FullScreen => saver.arrange_displays()?,
        Mode::Preview(parent) => {
            // SAFETY: The parent is borrowed from the native /p command. It is
            // validated before creation and on each bounded preview tick.
            if !unsafe { IsWindow(Some(parent)).as_bool() } {
                return Err(Error::new(
                    E_FAIL,
                    "Screen saver preview parent is unavailable",
                ));
            }
            saver.create(Some(parent), RECT::default())?;
            saver.resize_preview(parent)?;
        }
    }
    let first = saver.first.get();
    saver.app.set_presentation(Some(first));
    saver.activity.replace(Some(Activity::new(first)));
    super::appearance::refresh(&saver.app);
    saver
        .app
        .load_world()
        .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
    // SAFETY: Show only our initialized surfaces. Preview never activates,
    // focuses, hides the cursor or captures any input from its parent.
    unsafe {
        let windows = saver.windows.borrow().clone();
        for window in windows {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        if matches!(mode, Mode::FullScreen) {
            let mut position = POINT::default();
            GetCursorPos(&mut position)?;
            saver.mouse_origin.set(position);
            let _ = SetForegroundWindow(first);
            let _ = SetFocus(Some(first));
        } else if SetTimer(Some(first), PREVIEW_TIMER, 500, None) == 0 {
            return Err(Error::from_thread());
        }
    }
    saver.running.set(true);
    saver.invalidate();
    let mut message = MSG::default();
    loop {
        // SAFETY: One UI thread dispatches all saver surfaces and app replies.
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if result < 0 {
            return Err(Error::from_thread());
        }
        if result == 0 {
            break;
        }
        // SAFETY: GetMessage supplied this native message.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    saver.failure.take().map_or(Ok(()), Err)
}

impl Saver {
    fn create(&self, parent: Option<HWND>, bounds: RECT) -> Result<HWND> {
        let style = if parent.is_some() { WS_CHILD } else { WS_POPUP };
        let extended = if parent.is_some() {
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
        } else {
            WS_EX_TOOLWINDOW
        };
        // SAFETY: The stable run-owned Saver pointer is retired at NCDESTROY.
        let window = unsafe {
            CreateWindowExW(
                extended,
                w!("NetMap.ScreenSaver"),
                w!("NetMap"),
                style,
                bounds.left,
                bounds.top,
                (bounds.right - bounds.left).max(1),
                (bounds.bottom - bounds.top).max(1),
                parent,
                None,
                Some(self.instance),
                Some((self as *const Self).cast()),
            )?
        };
        self.app.map.attach_surface(window)?;
        Ok(window)
    }

    fn arrange_displays(&self) -> Result<()> {
        let mut displays = Vec::<RECT>::new();
        // SAFETY: The synchronous enumeration writes only to this local Vec;
        // Windows supplies each connected display rectangle during the callback.
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(display_rect),
                LPARAM((&mut displays as *mut Vec<RECT>) as isize),
            )
            .ok()?;
        }
        if displays.is_empty() {
            return Err(Error::new(
                E_FAIL,
                "No display is available for the screen saver",
            ));
        }
        let mut windows = self.windows.borrow().clone();
        for (index, bounds) in displays.iter().enumerate() {
            let window = if let Some(window) = windows.get(index) {
                *window
            } else {
                let window = self.create(None, *bounds)?;
                windows.push(window);
                window
            };
            // SAFETY: Only our top-level saver surface is sized to its display.
            // No display mode, power policy or execution-state override is used.
            unsafe {
                SetWindowPos(
                    window,
                    Some(HWND_TOPMOST),
                    bounds.left,
                    bounds.top,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    SWP_NOACTIVATE,
                )?;
                if self.running.get() {
                    let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
                }
            }
        }
        for window in windows.into_iter().skip(displays.len()) {
            // SAFETY: These are only our surfaces for disconnected displays.
            unsafe { DestroyWindow(window)? };
        }
        self.invalidate();
        Ok(())
    }

    fn resize_preview(&self, parent: HWND) -> Result<()> {
        let window = self.first.get();
        let mut parent_client = RECT::default();
        let mut client = RECT::default();
        // SAFETY: This child never reads or changes parent state beyond its
        // current client bounds. Parent destruction ends the child naturally.
        unsafe {
            if !IsWindow(Some(parent)).as_bool() {
                self.close();
                return Ok(());
            }
            GetClientRect(parent, &mut parent_client)?;
            GetClientRect(window, &mut client)?;
            if parent_client.right != client.right || parent_client.bottom != client.bottom {
                SetWindowPos(
                    window,
                    None,
                    0,
                    0,
                    parent_client.right.max(0),
                    parent_client.bottom.max(0),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
            }
        }
        Ok(())
    }

    fn available(&self, window: HWND) -> bool {
        self.activity.borrow().as_ref().is_none_or(Activity::available)
            // SAFETY: The caller is processing this owned HWND's message.
            && unsafe { IsWindowVisible(window).as_bool() && !IsIconic(window).as_bool() }
            && match self.mode {
                Mode::FullScreen => true,
                // SAFETY: The preview borrows this parent; these queries tolerate
                // its concurrent destruction without sending foreign messages.
                Mode::Preview(parent) => unsafe {
                    IsWindowVisible(parent).as_bool() && !IsIconic(parent).as_bool()
                },
            }
    }

    fn resize(&self, window: HWND) -> Result<()> {
        if window == self.first.get() {
            let mut client = RECT::default();
            // SAFETY: This first surface defines the common fitted grouping
            // scale. Other targets derive their own viewport from those facts.
            unsafe {
                GetClientRect(window, &mut client)?;
                self.app.map.resize(
                    client.right.max(0) as u32,
                    client.bottom.max(0) as u32,
                    GetDpiForWindow(window).max(96),
                );
            }
            self.app.prepare_scene();
        }
        self.invalidate();
        Ok(())
    }

    fn invalidate(&self) {
        let windows = self.windows.borrow().clone();
        for window in windows {
            // SAFETY: Frame wakes belong only to these owned surfaces. A static
            // map gets no repeating render timer; resume requests current ages.
            unsafe {
                let _ = KillTimer(Some(window), FRAME_TIMER);
                if self.available(window) {
                    let _ = InvalidateRect(Some(window), None, false);
                }
            }
        }
    }

    fn paint(&self, window: HWND) {
        let mut paint = PAINTSTRUCT::default();
        // SAFETY: Each WM_PAINT has a paired native paint lifetime, including
        // unavailable surfaces and renderer failure.
        unsafe {
            let _ = BeginPaint(window, &mut paint);
        }
        let result = (|| {
            if !self.available(window) {
                return Ok(Frame::Idle);
            }
            let mut client = RECT::default();
            // SAFETY: Query only this callback's live surface on its UI thread.
            unsafe {
                GetClientRect(window, &mut client)?;
            }
            // SAFETY: DPI comes from the actual presentation target.
            let dpi = unsafe { GetDpiForWindow(window) }.max(96);
            self.app.map.paint_surface(
                window,
                client.right.max(0) as u32,
                client.bottom.max(0) as u32,
                dpi,
            )
        })();
        // SAFETY: Retire the prior wake after ending the matching paint.
        unsafe {
            let _ = EndPaint(window, &paint);
            let _ = KillTimer(Some(window), FRAME_TIMER);
        }
        if self.closing.get() || !self.windows.borrow().contains(&window) {
            return;
        }
        match result {
            Ok(Frame::Again) => {
                // SAFETY: Request the newest revision through normal painting.
                unsafe {
                    let _ = InvalidateRect(Some(window), None, false);
                }
            }
            Ok(Frame::After(delay)) => {
                // SAFETY: One bounded one-shot frame timer per live surface.
                unsafe {
                    SetTimer(
                        Some(window),
                        FRAME_TIMER,
                        delay.as_millis().clamp(1, u128::from(u32::MAX)) as u32,
                        None,
                    );
                }
            }
            Ok(Frame::Idle) => {}
            Err(error) => self.fail(error),
        }
    }

    fn close(&self) {
        if self.closing.replace(true) {
            return;
        }
        // SAFETY: Posting avoids destroying HWNDs in a borrowed message handler.
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn fail(&self, error: Error) {
        if self.failure.borrow().is_none() {
            self.failure.replace(Some(error));
        }
        self.close();
    }

    fn moved(&self, window: HWND) -> bool {
        let mut position = POINT::default();
        // SAFETY: Reading the screen-space cursor does not move or capture it.
        if unsafe { GetCursorPos(&mut position) }.is_err() {
            return false;
        }
        let origin = self.mouse_origin.get();
        // Initial/synthetic mouse messages at the establishment point do not
        // dismiss the saver; meaningful displacement does, across any display.
        // SAFETY: Query only the window currently receiving the message.
        let threshold = i64::from(unsafe { GetDpiForWindow(window) }.max(96)) * 4 / 96;
        (i64::from(position.x) - i64::from(origin.x)).abs() > threshold
            || (i64::from(position.y) - i64::from(origin.y)).abs() > threshold
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        self.running.set(false);
        self.closing.set(true);
        self.app.set_presentation(None);
        self.app.cancel_work();
        self.activity.take();
        let windows = self.windows.borrow().clone();
        // SAFETY: All callbacks still borrow this live owner during destruction.
        // Each NCDESTROY retires its pointer and renderer target before return.
        unsafe {
            for window in windows {
                let _ = DestroyWindow(window);
            }
            if matches!(self.mode, Mode::FullScreen) {
                SetCursor((!self.cursor.is_invalid()).then_some(self.cursor));
            }
            let _ = UnregisterClassW(w!("NetMap.ScreenSaver"), Some(self.instance));
        }
    }
}

unsafe extern "system" fn display_rect(
    _monitor: HMONITOR,
    _dc: HDC,
    bounds: *mut RECT,
    context: LPARAM,
) -> BOOL {
    // SAFETY: EnumDisplayMonitors supplies a valid RECT and the run-owned Vec
    // pointer only for the duration of its synchronous enumeration.
    unsafe {
        (&mut *(context.0 as *mut Vec<RECT>)).push(*bounds);
    }
    BOOL(1)
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: CreateWindowEx passes our stable Saver pointer in CREATESTRUCT.
        let creation = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        // SAFETY: Only this procedure owns the slot, cleared at NCDESTROY.
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
    }
    if let Some(saver) =
        // SAFETY: NCCREATE stores run's stable Box pointer; NCDESTROY clears it
        // before that Box can drop, including during initialization failure.
        unsafe { (GetWindowLongPtrW(window, GWLP_USERDATA) as *const Saver).as_ref() }
    {
        if matches!(saver.mode, Mode::FullScreen) && saver.running.get() {
            match message {
                WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
                | WM_XBUTTONDOWN | WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_POINTERDOWN => saver.close(),
                WM_MOUSEMOVE if saver.moved(window) => saver.close(),
                WM_ACTIVATEAPP if wparam.0 == 0 => saver.close(),
                _ => {}
            }
        }
        match message {
            WM_NCCREATE => {
                saver.windows.borrow_mut().push(window);
                if saver.first.get().is_invalid() {
                    saver.first.set(window);
                }
                return LRESULT(1);
            }
            WM_PAINT => {
                saver.paint(window);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_SIZE => {
                if let Err(error) = saver.resize(window) {
                    saver.fail(error);
                }
            }
            WM_SHOWWINDOW => saver.invalidate(),
            REFRESH_MESSAGE if wparam.0 != 0 => saver.invalidate(),
            WM_TIMER if wparam.0 == FRAME_TIMER => {
                // SAFETY: Consume this surface's one-shot wake once.
                unsafe {
                    let _ = KillTimer(Some(window), FRAME_TIMER);
                    if saver.available(window) {
                        let _ = InvalidateRect(Some(window), None, false);
                    }
                }
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == PREVIEW_TIMER => {
                if let Mode::Preview(parent) = saver.mode
                    && let Err(error) = saver.resize_preview(parent)
                {
                    saver.fail(error);
                }
                return LRESULT(0);
            }
            WM_DPICHANGED | WM_DISPLAYCHANGE if matches!(saver.mode, Mode::FullScreen) => {
                // SAFETY: Coalesce monitor changes outside the current resize.
                if !saver.display_change_pending.replace(true) {
                    // SAFETY: The first surface remains the coordinating HWND.
                    unsafe {
                        let _ = PostMessageW(
                            Some(saver.first.get()),
                            DISPLAYS_CHANGED,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
                return LRESULT(0);
            }
            DISPLAYS_CHANGED => {
                saver.display_change_pending.set(false);
                if let Err(error) = saver.arrange_displays() {
                    saver.fail(error);
                }
                return LRESULT(0);
            }
            WM_DPICHANGED | WM_DPICHANGED_AFTERPARENT => {
                if let Err(error) = saver.resize(window) {
                    saver.fail(error);
                }
            }
            WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_SETTINGCHANGE => {
                super::appearance::refresh(&saver.app);
                saver.invalidate();
            }
            WM_WTSSESSION_CHANGE | WM_POWERBROADCAST => {
                if window == saver.first.get() {
                    if let Some(activity) = saver.activity.borrow_mut().as_mut() {
                        activity.message(message, wparam, lparam);
                    }
                    saver.invalidate();
                }
                return LRESULT(1);
            }
            WM_SETCURSOR if matches!(saver.mode, Mode::FullScreen) => {
                // SAFETY: Hide only this saver surface's client cursor; no global
                // ShowCursor count is changed. Drop restores the borrowed cursor.
                unsafe {
                    SetCursor(None);
                }
                return LRESULT(1);
            }
            WM_MOUSEACTIVATE if matches!(saver.mode, Mode::Preview(_)) => {
                return LRESULT(MA_NOACTIVATE as isize);
            }
            WM_SYSCOMMAND if wparam.0 & 0xfff0 == SC_SCREENSAVE as usize => return LRESULT(0),
            WM_CLOSE => {
                saver.close();
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                saver.app.map.detach_surface(window);
                saver.windows.borrow_mut().retain(|owned| *owned != window);
                if window == saver.first.get() {
                    saver.activity.take();
                    saver.app.set_presentation(None);
                    saver.first.set(HWND::default());
                    saver.close();
                }
                // SAFETY: Retire the borrowed owner before returning to Windows.
                unsafe {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
            }
            _ => {}
        }
    }
    // SAFETY: Preserve default native behavior, including display power/sleep.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
