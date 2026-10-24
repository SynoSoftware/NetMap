use crate::app::{Application, REFRESH_MESSAGE};
use std::{
    cell::{Cell, OnceCell, RefCell},
    mem::size_of,
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, DeleteObject, EndPaint, HFONT, InvalidateRect, PAINTSTRUCT},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, SB_SETTEXTW},
            HiDpi::{GetDpiForWindow, SystemParametersInfoForDpi},
            WindowsAndMessaging::*,
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

mod accessibility;
mod activity;
mod appearance;
pub(crate) mod controls;
pub(crate) mod desktop;
pub(crate) mod dialog;
mod exploration;
mod hosting;
pub(crate) mod ids;
#[cfg(test)]
pub(crate) mod native_broker;
#[cfg(test)]
mod native_dataset;
#[cfg(test)]
mod native_desktop;
#[cfg(test)]
mod native_dynamics;
#[cfg(test)]
mod native_exploration;
#[cfg(test)]
mod native_foreground;
#[cfg(test)]
mod native_lifetime;
#[cfg(test)]
mod native_settings;
#[cfg(test)]
mod native_shutdown;
mod navigation;
pub(crate) mod screensaver;
mod tray;

const TRAY_MESSAGE: u32 = WM_APP + 12;

pub(crate) fn configuration_apartment() -> Result<impl Drop> {
    initialize_controls()?;
    accessibility::Apartment::new(windows::Win32::System::Com::COINIT_APARTMENTTHREADED)
}

fn initialize_controls() -> Result<()> {
    // SAFETY: Process-local common control registration; no window is created.
    unsafe {
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES
                | windows::Win32::UI::Controls::ICC_LISTVIEW_CLASSES
                | windows::Win32::UI::Controls::ICC_UPDOWN_CLASS,
        })
        .ok()
    }
}

struct Host {
    accessibility: accessibility::Accessibility,
    app: Rc<Application>,
    window: Cell<HWND>,
    map_window: Cell<HWND>,
    status: Cell<HWND>,
    fit: Cell<HWND>,
    zoom: Cell<HWND>,
    tooltip: Cell<HWND>,
    tooltip_text: RefCell<HSTRING>,
    restore_maximized: Cell<bool>,
    activity: RefCell<Option<activity::Activity>>,
    foreground: RefCell<Option<activity::Foreground>>,
    saver_covered: Cell<bool>,
    font: Cell<Option<HFONT>>,
    failure: RefCell<Option<Error>>,
    exploration: OnceCell<crate::app::exploration::Exploration>,
    view: Cell<exploration::View>,
    stacked: Cell<bool>,
    side_fraction: Cell<f64>,
    stacked_fraction: Cell<f64>,
    split_drag: Cell<Option<exploration::SplitDrag>>,
    split_bounds: Cell<RECT>,
    hosting_preferences: RefCell<Option<(bool, String)>>,
    exit_requested: Cell<bool>,
    background_requested: Cell<bool>,
    desktop_attachment: RefCell<Option<desktop::Attached>>,
    desktop_monitor: RefCell<Option<desktop::Monitor>>,
    tray: OnceCell<tray::Tray>,
    notice: RefCell<Option<String>>,
}

pub fn run(app: Rc<Application>) -> Result<()> {
    let _apartment =
        accessibility::Apartment::new(windows::Win32::System::Com::COINIT_APARTMENTTHREADED)?;
    let host = Box::new(Host {
        accessibility: accessibility::Accessibility::new()?,
        app,
        window: Cell::new(HWND::default()),
        map_window: Cell::new(HWND::default()),
        status: Cell::new(HWND::default()),
        fit: Cell::new(HWND::default()),
        zoom: Cell::new(HWND::default()),
        tooltip: Cell::new(HWND::default()),
        tooltip_text: RefCell::new(HSTRING::new()),
        restore_maximized: Cell::new(false),
        activity: RefCell::new(None),
        foreground: RefCell::new(None),
        saver_covered: Cell::new(false),
        font: Cell::new(None),
        failure: RefCell::new(None),
        exploration: OnceCell::new(),
        view: Cell::new(exploration::View::Map),
        stacked: Cell::new(false),
        side_fraction: Cell::new(0.55),
        stacked_fraction: Cell::new(0.5),
        split_drag: Cell::new(None),
        split_bounds: Cell::new(RECT::default()),
        hosting_preferences: RefCell::new(None),
        exit_requested: Cell::new(false),
        background_requested: Cell::new(false),
        desktop_attachment: RefCell::new(None),
        desktop_monitor: RefCell::new(None),
        tray: OnceCell::new(),
        notice: RefCell::new(None),
    });
    // SAFETY: Both classes use the same stable Box address. The Box outlives
    // every HWND and callback; NCDESTROY removes each native pointer first.
    unsafe {
        initialize_controls()?;
        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let main_class = WNDCLASSW {
            hInstance: instance,
            lpfnWndProc: Some(main_proc),
            hCursor: cursor,
            hbrBackground: windows::Win32::Graphics::Gdi::GetSysColorBrush(
                windows::Win32::Graphics::Gdi::COLOR_BTNFACE,
            ),
            lpszClassName: w!("NetMap.Window"),
            lpszMenuName: PCWSTR(ids::IDR_MAIN_MENU as usize as *const u16),
            ..Default::default()
        };
        let map_class = WNDCLASSW {
            style: CS_DBLCLKS,
            hInstance: instance,
            lpfnWndProc: Some(map_proc),
            hCursor: cursor,
            lpszClassName: w!("NetMap.Map"),
            ..Default::default()
        };
        if RegisterClassW(&main_class) == 0 || RegisterClassW(&map_class) == 0 {
            return Err(Error::from_thread());
        }
        let pointer = (&*host as *const Host).cast();
        let window = CreateWindowExW(
            Default::default(),
            main_class.lpszClassName,
            w!("NetMap"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            800,
            None,
            None,
            Some(instance),
            Some(pointer),
        )?;
        let initialization = (|| {
            host.activity.replace(Some(activity::Activity::new(window)));
            host.foreground.replace(activity::Foreground::new(window));
            host.saver_covered.set(activity::saver_foreground());
            if host.foreground.borrow().is_none() {
                host.notice.replace(Some(
                    "Screen saver foreground monitoring unavailable.".into(),
                ));
            }
            let map = CreateWindowExW(
                Default::default(),
                map_class.lpszClassName,
                w!("Network map"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(window),
                Some(HMENU(ids::IDC_MAP as usize as *mut _)),
                Some(instance),
                Some(pointer),
            )?;
            host.map_window.set(map);
            host.accessibility.attach(map)?;
            let status = CreateWindowExW(
                Default::default(),
                w!("msctls_statusbar32"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(windows::Win32::UI::Controls::SBARS_TOOLTIPS),
                0,
                0,
                1,
                1,
                Some(window),
                Some(HMENU(ids::IDC_STATUS as usize as *mut _)),
                Some(instance),
                None,
            )?;
            host.status.set(status);
            let exploration =
                crate::app::exploration::Exploration::create(window, ids::IDC_CONNECTIONS)?;
            let table = exploration.window();
            let _ = host.exploration.set(exploration);
            host.accessibility.name(table, w!("Connections"))?;
            host.create_navigation(instance, window)?;
            host.create_tooltip(instance, window, map)?;
            host.update_font()?;
            host.layout(true)?;
            host.app.map.attach(map)?;
            host.refresh_appearance();
            if host.window.get().is_invalid() {
                return Ok(());
            }
            #[cfg(test)]
            if native_broker::requested() {
                return native_broker::exercise(&host);
            }
            #[cfg(test)]
            if native_dataset::requested() {
                return native_dataset::exercise(&host);
            }
            #[cfg(test)]
            if native_dynamics::requested() {
                return native_dynamics::exercise(&host);
            }
            #[cfg(test)]
            if native_desktop::requested() {
                return native_desktop::exercise(&host);
            }
            #[cfg(test)]
            if native_exploration::requested() {
                return native_exploration::exercise(&host);
            }
            #[cfg(test)]
            if native_foreground::requested() {
                return native_foreground::exercise(&host);
            }
            #[cfg(test)]
            if native_lifetime::requested() {
                return native_lifetime::exercise(&host);
            }
            #[cfg(test)]
            if native_settings::requested() {
                return native_settings::exercise(&host);
            }
            #[cfg(test)]
            if native_shutdown::requested() {
                return native_shutdown::exercise(&host);
            }
            host.app
                .load_world()
                .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
            host.refresh_status();
            Ok(())
        })();
        if let Err(error) = initialization {
            host.app.cancel_work();
            let _ = DestroyWindow(window);
            return Err(error);
        }
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result == -1 {
                host.failure.replace(Some(Error::from_thread()));
                break;
            }
            if result == 0 {
                break;
            }
            if host.navigation_key(&message) {
                continue;
            }
            if IsDialogMessageW(window, &message).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        host.app.cancel_work();
        if !host.window.get().is_invalid() {
            let _ = DestroyWindow(host.window.get());
        }
    }
    host.failure.take().map_or(Ok(()), Err)
}

impl Host {
    fn restore_window(&self) {
        let window = self.window.get();
        if window.is_invalid() {
            return;
        }
        let Some(placement) = self.app.take_window_placement() else {
            return;
        };
        // SAFETY: Only this UI thread's top-level HWND is affected. Native
        // placement preserves workspace units and relocates off-screen bounds.
        unsafe {
            let restored = placement.is_some_and(|saved| {
                SetWindowPlacement(
                    window,
                    &WINDOWPLACEMENT {
                        length: size_of::<WINDOWPLACEMENT>() as u32,
                        showCmd: if saved.maximized {
                            SW_SHOWMAXIMIZED.0
                        } else {
                            SW_SHOWNORMAL.0
                        } as u32,
                        ptMinPosition: windows::Win32::Foundation::POINT { x: -1, y: -1 },
                        ptMaxPosition: windows::Win32::Foundation::POINT { x: -1, y: -1 },
                        rcNormalPosition: RECT {
                            left: saved.left,
                            top: saved.top,
                            right: saved.right,
                            bottom: saved.bottom,
                        },
                        ..Default::default()
                    },
                )
                .is_ok()
            });
            if !restored {
                let _ = ShowWindow(window, SW_SHOWDEFAULT);
            }
        }
    }

    fn save_window_placement(&self) {
        let mut placement = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        // SAFETY: Read our live top-level HWND into initialized stack storage.
        if unsafe { GetWindowPlacement(self.window.get(), &mut placement) }.is_ok() {
            self.app
                .save_window_placement(crate::app::placement::Placement {
                    left: placement.rcNormalPosition.left,
                    top: placement.rcNormalPosition.top,
                    right: placement.rcNormalPosition.right,
                    bottom: placement.rcNormalPosition.bottom,
                    maximized: self.restore_maximized.get(),
                });
        }
    }

    // Reconsider the source's fitted arrangement after a size/view transition;
    // proportion changes retain the orientation the person is manipulating.
    fn layout(&self, fit: bool) -> Result<()> {
        if self.on_desktop() {
            return self.layout_desktop();
        }
        let window = self.window.get();
        if window.is_invalid()
            || self.map_window.get().is_invalid()
            || self.status.get().is_invalid()
        {
            return Ok(());
        }
        // SAFETY: Native children and rectangles belong to this UI thread. The
        // status bar owns its height; Host coordinates only changed map bounds.
        unsafe {
            if IsIconic(window).as_bool() {
                return Ok(());
            }
            SendMessageW(self.status.get(), WM_SIZE, None, None);
            let mut client = RECT::default();
            let mut status = RECT::default();
            let mut previous = RECT::default();
            GetClientRect(window, &mut client)?;
            GetWindowRect(self.status.get(), &mut status)?;
            GetClientRect(self.map_window.get(), &mut previous)?;
            let width = client.right.max(0);
            let height = (client.bottom - (status.bottom - status.top)).max(0);
            let (map_rect, table_rect) =
                self.exploration_layout(width, height, GetDpiForWindow(window), fit);
            self.place_exploration(table_rect)?;
            if self.view.get() == exploration::View::Table {
                self.refresh_navigation();
                return Ok(());
            }
            let width = map_rect.right;
            let height = map_rect.bottom;
            let resized = previous.right != width || previous.bottom != height;
            if resized {
                SetWindowPos(
                    self.map_window.get(),
                    None,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
            }
            let mut update =
                self.app
                    .map
                    .resize(width as u32, height as u32, GetDpiForWindow(window));
            update.changed |= resized;
            self.layout_navigation(width, height, GetDpiForWindow(window))?;
            self.apply_map_update(update);
        }
        Ok(())
    }

    fn update_font(&self) -> Result<()> {
        if self.status.get().is_invalid() {
            return Ok(());
        }
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        // SAFETY: DPI and metrics are queried for our live window. The new font
        // stays owned until all controls have switched, and is freed after HWNDs.
        unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
                Default::default(),
                GetDpiForWindow(self.window.get()),
            )?;
            let font = windows::Win32::Graphics::Gdi::CreateFontIndirectW(&metrics.lfMessageFont);
            if font.is_invalid() {
                return Err(Error::from_thread());
            }
            if let Some(exploration) = self.exploration.get() {
                exploration.font(font);
            }
            for control in [
                self.status.get(),
                self.fit.get(),
                self.zoom.get(),
                self.tooltip.get(),
            ] {
                if !control.is_invalid() {
                    SendMessageW(
                        control,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                }
            }
            if let Some(previous) = self.font.replace(Some(font)) {
                let _ = DeleteObject(previous.into());
            }
        }
        Ok(())
    }

    fn refresh_status(&self) {
        if self.window.get().is_invalid() || self.status.get().is_invalid() {
            return;
        }
        let mut activity_warning = self
            .activity
            .borrow()
            .as_ref()
            .and_then(|activity| activity.warning().map(str::to_owned))
            .unwrap_or_default();
        if let Some(notice) = self.notice.borrow().as_ref() {
            activity_warning.push(' ');
            activity_warning.push_str(notice);
        }
        let text = HSTRING::from(format!(
            "{} — {} {}",
            self.app
                .map
                .graphics_mode()
                .unwrap_or("Graphics unavailable"),
            self.app.status(),
            activity_warning
        ));
        let map = self.map_window.get();
        if !map.is_invalid() {
            let description = self
                .app
                .inspection_description()
                .unwrap_or_else(|| self.app.status());
            self.refresh_tooltip();
            let view = self.app.map.viewport();
            let center = view.geographic_center();
            let help = HSTRING::from(format!(
                "{description}. View: {:.3}×; latitude {:.3}; longitude {:.3}. Arrow keys inspect connections; plus and minus zoom; Ctrl+arrows pan; Ctrl+0 fits; Escape clears selection.",
                view.zoom, center.latitude, center.longitude,
            ));
            // SAFETY: Native text remains available to ordinary HWND clients.
            // End application borrows before sending a potentially reentrant set.
            unsafe {
                let mut current = vec![0; GetWindowTextLengthW(map).max(0) as usize + 1];
                let length = GetWindowTextW(map, &mut current).max(0) as usize;
                if current[..length] != help[..] {
                    let _ = SetWindowTextW(map, &help);
                    if let Err(error) = self.accessibility.describe(map, &help) {
                        self.fail(error);
                    }
                }
            }
        }
        // SAFETY: This is our live status control on the UI thread. Reading its
        // current text avoids resetting or repainting an unchanged native value.
        unsafe {
            let mut current = vec![0; GetWindowTextLengthW(self.status.get()).max(0) as usize + 1];
            let length = GetWindowTextW(self.status.get(), &mut current).max(0) as usize;
            if current[..length] == text[..] {
                return;
            }
        }
        // SAFETY: SendMessage consumes this text synchronously. No application
        // borrow remains live and the common control owns its displayed copy.
        unsafe {
            SendMessageW(
                self.status.get(),
                SB_SETTEXTW,
                None,
                Some(LPARAM(text.as_ptr() as isize)),
            );
            SendMessageW(
                self.status.get(),
                windows::Win32::UI::Controls::SB_SETTIPTEXTW,
                None,
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
    }

    fn paint(&self, window: HWND) {
        let graphics = self.app.map.graphics_mode();
        let mut paint = PAINTSTRUCT::default();
        // SAFETY: Begin/EndPaint are paired even when drawing fails or reenters.
        unsafe {
            let _ = BeginPaint(window, &mut paint);
        }
        let drawing = (|| {
            if !self.can_draw() {
                return Ok(crate::map::Frame::Idle);
            }
            let mut client = RECT::default();
            // SAFETY: The callback supplied this live map HWND.
            unsafe {
                GetClientRect(window, &mut client)?;
            }
            // SAFETY: The callback supplied this live map HWND.
            let dpi = unsafe { GetDpiForWindow(window) };
            self.app
                .paint_map(client.right.max(0) as u32, client.bottom.max(0) as u32, dpi)
        })();
        #[cfg(test)]
        native_dynamics::painted(&drawing);
        // SAFETY: paint is the same live stack record passed to BeginPaint.
        unsafe {
            let _ = EndPaint(window, &paint);
            let _ = KillTimer(Some(window), 1);
        }
        if graphics != self.app.map.graphics_mode() {
            self.refresh_status();
        }
        match drawing {
            Ok(crate::map::Frame::Again) if !self.window.get().is_invalid() => {
                // SAFETY: A new revision arrived during drawing; request its frame.
                unsafe {
                    let _ = InvalidateRect(Some(window), None, false);
                }
            }
            Ok(crate::map::Frame::After(delay)) if !self.window.get().is_invalid() => {
                // SAFETY: One timer belongs to this map HWND; it is retired on
                // firing or destruction and never keeps a static bright hold busy.
                unsafe {
                    SetTimer(
                        Some(window),
                        1,
                        delay.as_millis().max(1).min(u128::from(u32::MAX)) as u32,
                        None,
                    );
                }
            }
            Ok(_) => {}
            Err(error) => self.fail(error),
        }
    }

    fn can_draw(&self) -> bool {
        if self.saver_covered.get() {
            return false;
        }
        let available = self
            .activity
            .borrow()
            .as_ref()
            .is_some_and(activity::Activity::available);
        let window = self.window.get();
        // SAFETY: Query only this presentation's UI-thread-owned HWND.
        if self.on_desktop() {
            return available
                && !self.map_window.get().is_invalid()
                && self
                    .desktop_attachment
                    .borrow()
                    .as_ref()
                    .is_some_and(desktop::Attached::valid);
        }
        available
            && self.view.get() != exploration::View::Table
            && !window.is_invalid()
            // SAFETY: These non-messaging queries borrow our live UI-thread HWND.
            && unsafe { IsWindowVisible(window).as_bool() && !IsIconic(window).as_bool() }
    }

    fn refresh_activity(&self) {
        let map = self.map_window.get();
        if map.is_invalid() {
            return;
        }
        // SAFETY: Retire the one-shot frame wake while unavailable. Native
        // resume/visibility notifications request a fresh current-age frame.
        unsafe {
            let _ = KillTimer(Some(map), 1);
            if self.can_draw() {
                let _ = InvalidateRect(Some(map), None, false);
            }
        }
    }

    fn fail(&self, error: Error) {
        if self.failure.borrow().is_some() {
            return;
        }
        self.failure.replace(Some(error));
        self.app.cancel_work();
        if !self.window.get().is_invalid() {
            // SAFETY: Close is posted, after borrows end, only to our live window.
            unsafe {
                let _ = PostMessageW(Some(self.window.get()), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(rows) = self.exploration.get().and_then(|table| table.take_rows()) {
            self.app.retire_rows(rows);
        }
        self.desktop_attachment.take();
        if let Some(font) = self.font.take() {
            // SAFETY: run destroys all font-using HWNDs before dropping Host.
            unsafe {
                let _ = DeleteObject(font.into());
            }
        }
    }
}

unsafe fn context(window: HWND, message: u32, lparam: LPARAM) -> *const Host {
    if message == WM_NCCREATE {
        // SAFETY: WM_NCCREATE supplies the CREATESTRUCT passed to CreateWindowEx.
        let creation = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        // SAFETY: This stores the stable, run-owned Box pointer without taking ownership.
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
    }
    // SAFETY: Only our procedures access this slot, cleared at NCDESTROY. run's
    // Box remains alive through message-loop termination and startup failure.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *const Host }
}

unsafe extern "system" fn main_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: context follows the CREATESTRUCT/NCDESTROY lifetime described above.
    if let Some(host) = unsafe { context(window, message, lparam).as_ref() } {
        if let Some(result) = host.split_input(message, lparam) {
            return result;
        }
        match message {
            WM_NCCREATE => {
                host.window.set(window);
                host.app.set_presentation(Some(window));
                return LRESULT(1);
            }
            WM_SIZE => {
                // GetWindowPlacement can return zero flags while minimized.
                // Keep the last non-minimized state as the restoration target.
                match wparam.0 as u32 {
                    SIZE_MAXIMIZED => host.restore_maximized.set(true),
                    SIZE_RESTORED => host.restore_maximized.set(false),
                    _ => {}
                }
                if let Err(error) = host.layout(true) {
                    host.fail(error);
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                host.cancel_split();
                // SAFETY: Windows supplies a proposed RECT for this DPI transition.
                let rect = unsafe { &*(lparam.0 as *const RECT) };
                // SAFETY: The proposed coordinates affect our own top-level window.
                let resized = unsafe {
                    SetWindowPos(
                        window,
                        None,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                if let Err(error) = resized
                    .and_then(|_| host.update_font())
                    .and_then(|_| host.layout(true))
                {
                    host.fail(error);
                }
                return LRESULT(0);
            }
            WM_COMMAND if (wparam.0 & 0xffff) == usize::from(ids::IDM_EXIT) => {
                host.exit_requested.set(true);
                // SAFETY: Close is routed through the window's ordinary lifetime.
                unsafe {
                    let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                return LRESULT(0);
            }
            WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_SETTINGCHANGE => {
                host.refresh_appearance();
            }
            WM_POWERBROADCAST | WM_WTSSESSION_CHANGE => {
                if let Some(activity) = host.activity.borrow_mut().as_mut() {
                    activity.message(message, wparam, lparam);
                }
                host.refresh_activity();
                return LRESULT(1);
            }
            activity::FOREGROUND_CHANGED => {
                let covered = activity::saver_foreground();
                if host.saver_covered.replace(covered) != covered {
                    host.refresh_activity();
                }
                return LRESULT(0);
            }
            WM_COMMAND => {
                let id = wparam.0 as u16;
                if id == ids::IDM_CAPTURE {
                    host.app.start_capture();
                    return LRESULT(0);
                }
                if host.hosting_command(id) || host.exploration_command(id) {
                    return LRESULT(0);
                }
                if id == ids::IDM_SETTINGS {
                    if let Err(error) = crate::app::settings::show(
                        &host.app,
                        window,
                        crate::app::settings::Page::Map,
                    ) {
                        host.fail(error);
                    }
                    return LRESULT(0);
                }
                if id == ids::IDM_CITY_DATA {
                    host.app.download_city();
                    return LRESULT(0);
                }
                if id == ids::IDM_CANCEL_CITY {
                    host.app.cancel_city();
                    return LRESULT(0);
                }
                if id == ids::IDC_FIT {
                    host.navigate(crate::map::Command::Fit);
                    return LRESULT(0);
                }
                match host.map_command(id) {
                    Ok(true) => return LRESULT(0),
                    Err(error) => {
                        host.fail(error);
                        return LRESULT(0);
                    }
                    Ok(false) => {}
                }
            }
            WM_INITMENUPOPUP => {
                let menu = HMENU(wparam.0 as *mut _);
                host.update_hosting_menu(menu);
                host.update_exploration_menu(menu);
                host.update_navigation_menu(menu);
                let download = host.app.can_download_city();
                let cancel = host.app.can_cancel_city();
                let available = host.app.city_available();
                // SAFETY: Refresh this native menu from owned model values;
                // command handlers recheck those values before execution.
                unsafe {
                    let _ = ModifyMenuW(
                        menu,
                        u32::from(ids::IDM_CITY_DATA),
                        MF_BYCOMMAND | MF_STRING,
                        usize::from(ids::IDM_CITY_DATA),
                        if available {
                            w!("Update &city data")
                        } else {
                            w!("Download &city data")
                        },
                    );
                    let _ = EnableMenuItem(
                        menu,
                        u32::from(ids::IDM_CITY_DATA),
                        MF_BYCOMMAND | if download { MF_ENABLED } else { MF_GRAYED },
                    );
                    let _ = EnableMenuItem(
                        menu,
                        u32::from(ids::IDM_CANCEL_CITY),
                        MF_BYCOMMAND | if cancel { MF_ENABLED } else { MF_GRAYED },
                    );
                }
                return LRESULT(0);
            }
            WM_SETFOCUS => {
                host.focus_map();
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                let map = host.map_window.get();
                if !map.is_invalid()
                    && let Some(result) = host.map_input(map, message, wparam, lparam)
                {
                    return result;
                }
            }
            WM_GETMINMAXINFO => {
                // SAFETY: Windows provides a writable MINMAXINFO during this message.
                unsafe {
                    let limits = &mut *(lparam.0 as *mut MINMAXINFO);
                    let dpi = GetDpiForWindow(window).max(96) as i32;
                    limits.ptMinTrackSize.x = 320 * dpi / 96;
                    limits.ptMinTrackSize.y = 240 * dpi / 96;
                }
                return LRESULT(0);
            }
            WM_CLOSE => {
                if host.background_enabled()
                    && !host.exit_requested.get()
                    && host.failure.borrow().is_none()
                {
                    host.use_desktop();
                    return LRESULT(0);
                }
                host.leave_desktop();
                host.save_window_placement();
                host.app.set_presentation(None);
                host.app.cancel_work();
                // SAFETY: Only this application's main window is destroyed.
                unsafe {
                    let _ = DestroyWindow(window);
                }
                return LRESULT(0);
            }
            WM_DESTROY => {
                if let Some(table) = host.exploration.get() {
                    host.accessibility.detach(table.window());
                    if let Some(rows) = table.take_rows() {
                        host.app.retire_rows(rows);
                    }
                }
                host.foreground.take();
                host.activity.take();
                host.app.set_presentation(None);
                host.app.cancel_work();
                host.status.set(HWND::default());
                host.fit.set(HWND::default());
                host.zoom.set(HWND::default());
                host.tooltip.set(HWND::default());
                // SAFETY: Only the main presentation ends this UI message loop.
                unsafe {
                    PostQuitMessage(0);
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                host.window.set(HWND::default());
                // SAFETY: Retire native delivery before run releases the Box.
                unsafe {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
            }
            REFRESH_MESSAGE => {
                host.restore_window();
                host.apply_hosting_preferences();
                host.accept_desktop();
                host.refresh_exploration();
                host.refresh_status();
                let map = host.map_window.get();
                if wparam.0 != 0 && !host.window.get().is_invalid() && !map.is_invalid() {
                    host.apply_map_update(crate::map::Update {
                        changed: true,
                        capture: host.app.map.capture(),
                    });
                }
                return LRESULT(0);
            }
            WM_NOTIFY => {
                if let Some(result) = host.exploration_notification(lparam) {
                    return result;
                }
            }
            TRAY_MESSAGE => {
                host.tray_message(lparam);
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                host.cancel_split();
                host.app.refresh_appearance();
                if host.background_enabled() {
                    host.recover_desktop();
                }
            }
            _ if host
                .tray
                .get()
                .is_some_and(|tray| message == tray.taskbar_created()) =>
            {
                host.recover_desktop();
                return LRESULT(0);
            }
            _ => {}
        }
    }
    // SAFETY: Unhandled messages retain standard Win32 window behavior.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

unsafe extern "system" fn map_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Both procedures use the same stable, run-owned context.
    if let Some(host) = unsafe { context(window, message, lparam).as_ref() } {
        if host.on_desktop() {
            match message {
                WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                _ => {}
            }
        }
        if !host.on_desktop()
            && let Some(result) = host.map_input(window, message, wparam, lparam)
        {
            return result;
        }
        match message {
            WM_NCCREATE => return LRESULT(1),
            WM_NOTIFY if host.tooltip_notification(lparam) => return LRESULT(0),
            WM_PAINT => {
                host.paint(window);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_TIMER if wparam.0 == 1 => {
                // SAFETY: This is the map's one-shot frame wake. Hidden/minimized
                // hosts resume from current facts on their next visible paint.
                unsafe {
                    let _ = KillTimer(Some(window), 1);
                    if host.can_draw() {
                        let _ = InvalidateRect(Some(window), None, false);
                    }
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                host.app.map.detach();
                host.accessibility.detach(window);
                host.map_window.set(HWND::default());
                // SAFETY: The native pointer is retired before any Box destruction.
                unsafe {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
            }
            _ => {}
        }
    }
    // SAFETY: The native default handles remaining focus/input/window messages.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

pub fn show_startup_error(error: &Error) {
    // SAFETY: The strings remain alive for the modal call; no model is borrowed.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(error.to_string()),
            w!("NetMap"),
            MB_OK | MB_ICONERROR,
        );
    }
}
