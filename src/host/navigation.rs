// Ported from TinyTorrent2 PeerMap/Navigation.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::map::{Capture, Command, Direction, Input, Update};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::ScreenToClient,
    System::SystemServices::{MK_LBUTTON, SS_CENTER, SS_CENTERIMAGE},
    UI::Controls::{
        NMHDR, NMTTDISPINFOW, TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_POP,
        TTM_SETMAXTIPWIDTH, TTN_GETDISPINFOW, TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
        WM_MOUSELEAVE,
    },
    UI::Input::KeyboardAndMouse::*,
};
use windows_numerics::Vector2;

pub(super) fn command(id: u16) -> Option<Command> {
    match id {
        ids::IDM_ZOOM_IN => Some(Command::ZoomIn),
        ids::IDM_ZOOM_OUT => Some(Command::ZoomOut),
        ids::IDM_FIT => Some(Command::Fit),
        _ => None,
    }
}

impl Host {
    pub(super) fn map_command(&self, id: u16) -> Result<bool> {
        if let Some(command) = command(id) {
            self.navigate(command);
            return Ok(true);
        }
        use crate::app::settings::{self, Detail, Page};
        let detail = match id {
            ids::IDM_COUNTRIES => Some(Detail::Countries),
            ids::IDM_CITIES => Some(Detail::Cities),
            _ => None,
        };
        if let Some(detail) = detail {
            self.app.choose_detail(detail);
            self.focus_map();
            return Ok(true);
        }
        if id == ids::IDM_ORIGIN && !self.window.get().is_invalid() {
            settings::show(&self.app, self.window.get(), Page::Location)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn create_tooltip(
        &self,
        instance: HINSTANCE,
        window: HWND,
        map: HWND,
    ) -> Result<()> {
        // SAFETY: One native tooltip is owned by our main window. Callback text
        // stays in Host until that owner destroys the tooltip during shutdown.
        unsafe {
            let tooltip = CreateWindowExW(
                WS_EX_TOPMOST,
                TOOLTIPS_CLASSW,
                None,
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
                0,
                0,
                0,
                0,
                Some(window),
                None,
                Some(instance),
                None,
            )?;
            self.tooltip.set(tooltip);
            let tool = TTTOOLINFOW {
                cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                uFlags: TTF_IDISHWND | TTF_SUBCLASS,
                hwnd: map,
                uId: map.0 as usize,
                lpszText: windows::core::PWSTR(usize::MAX as *mut u16),
                ..Default::default()
            };
            if SendMessageW(
                tooltip,
                TTM_ADDTOOLW,
                None,
                Some(LPARAM((&tool as *const TTTOOLINFOW) as isize)),
            )
            .0 == 0
            {
                return Err(Error::new(E_FAIL, "Could not attach map tooltip"));
            }
            SendMessageW(tooltip, TTM_SETMAXTIPWIDTH, None, Some(LPARAM(480)));
        }
        Ok(())
    }

    pub(super) fn refresh_tooltip(&self) {
        let text = HSTRING::from(if self.app.map.panning() {
            String::new()
        } else {
            self.app.inspection_description().unwrap_or_default()
        });
        if *self.tooltip_text.borrow() == text {
            return;
        }
        let tooltip = self.tooltip.get();
        if !tooltip.is_invalid() {
            // SAFETY: Hide any use of the previous callback text before replacing
            // its storage. TTF_DI_SETITEM is never set, so each show asks again.
            unsafe {
                SendMessageW(tooltip, TTM_POP, None, None);
            }
        }
        self.tooltip_text.replace(text);
    }

    pub(super) fn tooltip_notification(&self, lparam: LPARAM) -> bool {
        if lparam.0 == 0 {
            return false;
        }
        // SAFETY: WM_NOTIFY supplies a live NMHDR. Only our tooltip's matching
        // notification has the larger writable NMTTDISPINFOW payload.
        unsafe {
            let header = &*(lparam.0 as *const NMHDR);
            if header.hwndFrom != self.tooltip.get() || header.code != TTN_GETDISPINFOW {
                return false;
            }
            let notification = &mut *(lparam.0 as *mut NMTTDISPINFOW);
            notification.lpszText =
                windows::core::PWSTR(self.tooltip_text.borrow().as_ptr() as *mut u16);
        }
        true
    }

    pub(super) fn create_navigation(&self, instance: HINSTANCE, window: HWND) -> Result<()> {
        // SAFETY: These ordinary native controls belong to the main window;
        // their IDs, font and lifetime are coordinated by Host.
        unsafe {
            self.zoom.set(CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPSIBLINGS | WINDOW_STYLE(SS_CENTER.0 | SS_CENTERIMAGE.0),
                0,
                0,
                1,
                1,
                Some(window),
                Some(HMENU(ids::IDC_ZOOM as usize as *mut _)),
                Some(instance),
                None,
            )?);
            self.fit.set(CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("&Fit"),
                WS_CHILD | WS_CLIPSIBLINGS | WS_TABSTOP,
                0,
                0,
                1,
                1,
                Some(window),
                Some(HMENU(ids::IDC_FIT as usize as *mut _)),
                Some(instance),
                None,
            )?);
        }
        Ok(())
    }

    pub(super) fn layout_navigation(&self, width: i32, height: i32, dpi: u32) -> Result<()> {
        let scale = dpi as f64 / 96.0;
        let px = |value: i32| (f64::from(value) * scale).round() as i32;
        let left = (width - px(144)) / 2;
        let top = (height - px(48)).max(0);
        // SAFETY: Batch changed sibling positions under the same native parent.
        unsafe {
            let mut changed = Vec::with_capacity(2);
            for (control, x, w) in [
                (self.zoom.get(), left, px(72)),
                (self.fit.get(), left + px(80), px(64)),
            ] {
                if control.is_invalid() {
                    continue;
                }
                let mut previous = RECT::default();
                GetWindowRect(control, &mut previous)?;
                let mut point = POINT {
                    x: previous.left,
                    y: previous.top,
                };
                let _ = ScreenToClient(self.window.get(), &mut point);
                if point.x != x
                    || point.y != top
                    || previous.right - previous.left != w
                    || previous.bottom - previous.top != px(32)
                {
                    changed.push((control, x, w));
                }
            }
            if !changed.is_empty() {
                let mut batch = BeginDeferWindowPos(changed.len() as i32)?;
                for (control, x, w) in changed {
                    batch = DeferWindowPos(
                        batch,
                        control,
                        Some(HWND_TOP),
                        x,
                        top,
                        w,
                        px(32),
                        SWP_NOACTIVATE,
                    )?;
                }
                EndDeferWindowPos(batch)?;
            }
        }
        self.refresh_navigation();
        Ok(())
    }

    pub(super) fn refresh_navigation(&self) {
        let view = self.app.map.viewport();
        let visible = !self.on_desktop()
            && self.view.get() != super::exploration::View::Table
            && !view.fitted()
            && view.size.X >= 176.0
            && view.size.Y >= 64.0;
        let text = HSTRING::from(format!("{:.1}×", view.zoom));
        // SAFETY: Readback avoids replacing an unchanged caption. When hiding
        // a focused command, move focus before removing it from the tab order.
        unsafe {
            if self.window.get().is_invalid() {
                return;
            }
            if !visible && GetFocus() == self.fit.get() {
                self.focus_map();
            }
            if self.window.get().is_invalid() {
                return;
            }
            let zoom = self.zoom.get();
            let mut current = [0u16; 32];
            let length = GetWindowTextW(zoom, &mut current).max(0) as usize;
            if current[..length] != text[..] {
                let _ = SetWindowTextW(zoom, &text);
            }
            for control in [self.zoom.get(), self.fit.get()] {
                if control.is_invalid() {
                    continue;
                }
                let was_visible = GetWindowLongPtrW(control, GWL_STYLE) as u32 & WS_VISIBLE.0 != 0;
                if was_visible != visible {
                    let _ = ShowWindow(control, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                }
            }
            let bounds = if visible {
                let (mut zoom, mut fit) = (RECT::default(), RECT::default());
                if GetWindowRect(self.zoom.get(), &mut zoom).is_ok()
                    && GetWindowRect(self.fit.get(), &mut fit).is_ok()
                {
                    let mut point = POINT {
                        x: zoom.left.min(fit.left),
                        y: zoom.top.min(fit.top),
                    };
                    let scale = GetDpiForWindow(self.map_window.get()) as f32 / 96.0;
                    let width = zoom.right.max(fit.right) - point.x;
                    let height = zoom.bottom.max(fit.bottom) - point.y;
                    if ScreenToClient(self.map_window.get(), &mut point).as_bool() {
                        Some(windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                            left: point.x as f32 / scale,
                            top: point.y as f32 / scale,
                            right: (point.x + width) as f32 / scale,
                            bottom: (point.y + height) as f32 / scale,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            self.app.map.set_navigation_bounds(bounds);
        }
    }

    pub(super) fn navigation_key(&self, message: &MSG) -> bool {
        if message.message != WM_KEYDOWN {
            return false;
        }
        if message.wParam.0 == VK_ESCAPE.0 as usize && self.split_drag.get().is_some() {
            self.cancel_split();
            return true;
        }
        // SAFETY: Only map/Fit keys are interpreted; future edit controls retain theirs.
        let focus = unsafe { GetFocus() };
        if message.wParam.0 == VK_ESCAPE.0 as usize
            && self
                .exploration
                .get()
                .is_some_and(|table| table.window() == focus)
        {
            self.clear_exploration();
            return true;
        }
        (focus == self.map_window.get() || focus == self.fit.get())
            && self.key(message.wParam.0 as u16)
    }

    pub(super) fn focus_map(&self) {
        let map = if self.view.get() == super::exploration::View::Table {
            self.exploration
                .get()
                .map_or(self.map_window.get(), |table| table.window())
        } else {
            self.map_window.get()
        };
        if !map.is_invalid() {
            // SAFETY: Focus only our live child, with no model borrow held.
            unsafe {
                let _ = SetFocus(Some(map));
            }
        }
    }

    pub(super) fn navigate(&self, command: Command) {
        if self.map_window.get().is_invalid() || !self.app.map.available(command) {
            return;
        }
        let update = match command {
            Command::ZoomIn => self.app.zoom_inspection(1.0),
            Command::ZoomOut => self.app.zoom_inspection(-1.0),
            _ => self.app.map.input(Input::Command(command)),
        };
        self.apply_map_update(update);
        self.focus_map();
    }

    pub(super) fn apply_map_update(&self, update: Update) {
        self.app.prepare_scene();
        let map = self.map_window.get();
        if map.is_invalid() {
            return;
        }
        if update.changed {
            // SAFETY: Redraw only this live map; no state borrow spans the call.
            unsafe {
                let _ = InvalidateRect(Some(map), None, false);
            }
            self.refresh_navigation();
            self.refresh_status();
            if self.map_window.get() != map {
                return;
            }
        }
        // SAFETY: Capture belongs to the UI thread. Set/ReleaseCapture can
        // reenter; its result is accepted by the same pending transition only.
        unsafe {
            if self.app.map.capture() != update.capture {
                return;
            }
            match update.capture {
                Capture::Acquire(id) => {
                    #[cfg(test)]
                    super::native_dynamics::capture_requested(id);
                    SetCapture(map);
                    if self.map_window.get() != map {
                        return;
                    }
                    let update = self.app.map.input(Input::Captured {
                        id,
                        success: GetCapture() == map,
                    });
                    self.apply_map_update(update);
                }
                Capture::Release if GetCapture() == map => {
                    let _ = ReleaseCapture();
                }
                _ => {}
            }
            // Captured mouse input does not generate WM_SETCURSOR. Change the
            // cursor here as well, only while this map owns the pointer.
            let mut point = POINT::default();
            if (GetCapture() == map
                || (GetCapture().is_invalid()
                    && GetCursorPos(&mut point).is_ok()
                    && WindowFromPoint(point) == map))
                && let Ok(cursor) = LoadCursorW(
                    None,
                    if self.app.map.panning() {
                        IDC_SIZEALL
                    } else {
                        IDC_ARROW
                    },
                )
            {
                SetCursor(Some(cursor));
            }
        }
    }

    pub(super) fn update_navigation_menu(&self, menu: HMENU) {
        for id in [ids::IDM_ZOOM_IN, ids::IDM_ZOOM_OUT, ids::IDM_FIT] {
            let enabled = self.app.map.available(command(id).unwrap());
            // SAFETY: The menu is provided by our message or owned popup. Flags
            // update existing commands and do not transfer native ownership.
            unsafe {
                let _ = EnableMenuItem(
                    menu,
                    u32::from(id),
                    MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED },
                );
            }
        }
        use crate::app::settings::Detail;
        let committed = self.app.preferences().committed.detail;
        for (id, detail) in [
            (ids::IDM_COUNTRIES, Detail::Countries),
            (ids::IDM_CITIES, Detail::Cities),
        ] {
            // SAFETY: Refresh existing native commands from the shared owner;
            // execution rechecks the same availability after modal menu reentry.
            unsafe {
                let _ = EnableMenuItem(
                    menu,
                    u32::from(id),
                    MF_BYCOMMAND
                        | if self.app.can_choose_detail(detail) {
                            MF_ENABLED
                        } else {
                            MF_GRAYED
                        },
                );
                let _ = CheckMenuItem(
                    menu,
                    u32::from(id),
                    (MF_BYCOMMAND
                        | if committed == detail {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        })
                    .0,
                );
            }
        }
    }

    fn map_point(&self, window: HWND, lparam: LPARAM, screen: bool) -> Vector2 {
        let mut point = POINT {
            x: lparam.0 as i16 as i32,
            y: (lparam.0 >> 16) as i16 as i32,
        };
        // SAFETY: Coordinates refer to our live map. Mouse-wheel/context-menu
        // coordinates are screen pixels; other mouse messages are client pixels.
        let dpi = unsafe {
            if screen {
                let _ = ScreenToClient(window, &mut point);
            }
            GetDpiForWindow(window)
        };
        Vector2::new(point.x as f32, point.y as f32) * (96.0 / dpi as f32)
    }

    fn key(&self, key: u16) -> bool {
        // SAFETY: Query this UI thread's input state without changing it.
        let (alt, control) = unsafe {
            (
                GetKeyState(i32::from(VK_MENU.0)) < 0,
                GetKeyState(i32::from(VK_CONTROL.0)) < 0,
            )
        };
        if alt {
            return false;
        }
        let command = match VIRTUAL_KEY(key) {
            VK_ADD | VK_OEM_PLUS => Some(Command::ZoomIn),
            VK_SUBTRACT | VK_OEM_MINUS => Some(Command::ZoomOut),
            VK_0 | VK_NUMPAD0 if control => Some(Command::Fit),
            VK_LEFT if control => Some(Command::Pan(Direction::Left)),
            VK_RIGHT if control => Some(Command::Pan(Direction::Right)),
            VK_UP if control => Some(Command::Pan(Direction::Up)),
            VK_DOWN if control => Some(Command::Pan(Direction::Down)),
            VK_ESCAPE => {
                if !self.app.map.pressed() {
                    self.clear_exploration();
                }
                let update = self.app.map.input(Input::Cancel);
                self.apply_map_update(update);
                return true;
            }
            VK_LEFT | VK_UP | VK_RIGHT | VK_DOWN => {
                let update = self
                    .app
                    .inspect_next(matches!(VIRTUAL_KEY(key), VK_LEFT | VK_UP));
                self.apply_map_update(update);
                return true;
            }
            _ => None,
        };
        if let Some(command) = command {
            self.navigate(command);
            true
        } else {
            false
        }
    }

    fn map_menu(&self, window: HWND, lparam: LPARAM) -> Result<()> {
        // SAFETY: This popup is owned for the modal call and destroyed afterward;
        // the command runs only after rechecking the current model/window.
        unsafe {
            let menu = CreatePopupMenu()?;
            let result = (|| {
                AppendMenuW(
                    menu,
                    MF_STRING,
                    usize::from(ids::IDM_ZOOM_IN),
                    w!("Zoom &in\t+"),
                )?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    usize::from(ids::IDM_ZOOM_OUT),
                    w!("Zoom &out\t-"),
                )?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    usize::from(ids::IDM_FIT),
                    w!("&Fit\tCtrl+0"),
                )?;
                AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    usize::from(ids::IDM_COUNTRIES),
                    w!("&Countries"),
                )?;
                AppendMenuW(menu, MF_STRING, usize::from(ids::IDM_CITIES), w!("C&ities"))?;
                AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    usize::from(ids::IDM_ORIGIN),
                    w!("Map &origin..."),
                )?;
                self.update_navigation_menu(menu);
                let point = if lparam.0 == -1 {
                    let mut rect = RECT::default();
                    GetWindowRect(window, &mut rect)?;
                    POINT {
                        x: (rect.left + rect.right) / 2,
                        y: (rect.top + rect.bottom) / 2,
                    }
                } else {
                    POINT {
                        x: lparam.0 as i16 as i32,
                        y: (lparam.0 >> 16) as i16 as i32,
                    }
                };
                let selected = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    None,
                    window,
                    None,
                )
                .0 as u16;
                if self.map_window.get() == window {
                    self.map_command(selected)?;
                }
                Ok(())
            })();
            let _ = DestroyMenu(menu);
            result
        }
    }

    pub(super) fn map_input(
        &self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        let was_pressed = self.app.map.pressed();
        let was_panning = self.app.map.panning();
        let input = match message {
            WM_LBUTTONDOWN => {
                self.focus_map();
                if self.map_window.get() != window {
                    return Some(LRESULT(0));
                }
                Some(Input::Press(self.map_point(window, lparam, false)))
            }
            WM_MOUSEMOVE => Some(Input::Move {
                position: self.map_point(window, lparam, false),
                left_down: wparam.0 & MK_LBUTTON.0 as usize != 0,
            }),
            WM_LBUTTONUP => Some(Input::Release(self.map_point(window, lparam, false))),
            WM_MOUSELEAVE => {
                self.app.hover_map(None);
                return Some(LRESULT(0));
            }
            WM_LBUTTONDBLCLK => Some(Input::Zoom {
                steps: 1.0,
                anchor: self.map_point(window, lparam, false),
            }),
            WM_MOUSEWHEEL => {
                let anchor = self.map_point(window, lparam, true);
                let size = self.app.map.viewport().size;
                if anchor.X < 0.0 || anchor.Y < 0.0 || anchor.X >= size.X || anchor.Y >= size.Y {
                    return None;
                }
                Some(Input::Zoom {
                    steps: f64::from((wparam.0 >> 16) as i16) / 120.0,
                    anchor,
                })
            }
            WM_CANCELMODE | WM_KILLFOCUS => Some(Input::Cancel),
            WM_CAPTURECHANGED if lparam.0 != window.0 as isize => Some(Input::Cancel),
            WM_GETDLGCODE => return Some(LRESULT(DLGC_WANTARROWS as isize)),
            WM_SETCURSOR if lparam.0 as u16 == HTCLIENT as u16 => {
                // SAFETY: Use stock cursors; Windows owns their lifetime.
                unsafe {
                    if let Ok(cursor) = LoadCursorW(
                        None,
                        if self.app.map.panning() {
                            IDC_SIZEALL
                        } else {
                            IDC_ARROW
                        },
                    ) {
                        SetCursor(Some(cursor));
                    }
                }
                return Some(LRESULT(1));
            }
            WM_CONTEXTMENU => {
                if let Err(error) = self.map_menu(window, lparam) {
                    self.fail(error);
                }
                return Some(LRESULT(0));
            }
            _ => None,
        };
        input.map(|input| {
            let update = self.app.map.input(input);
            if message == WM_MOUSEMOVE && !self.app.map.pressed() {
                // SAFETY: Native leave tracking belongs to this live child.
                unsafe {
                    let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: window,
                        ..Default::default()
                    });
                }
                self.app
                    .hover_map(Some(self.map_point(window, lparam, false)));
            } else if message == WM_LBUTTONUP && was_pressed && !was_panning {
                self.app.choose_map(self.map_point(window, lparam, false));
            } else if self.app.map.panning() {
                self.app.hover_map(None);
            }
            self.apply_map_update(update);
            LRESULT(0)
        })
    }
}
