use super::*;
use crate::map::Input;

impl Host {
    pub(super) fn background_enabled(&self) -> bool {
        self.app.preferences().committed.desktop
    }
    pub(super) fn apply_hosting_preferences(&self) {
        let preferences = self.app.preferences().committed;
        let choice = (preferences.desktop, preferences.monitor);
        if self.hosting_preferences.borrow().as_ref() == Some(&choice) {
            return;
        }
        let previous = self.hosting_preferences.replace(Some(choice.clone()));
        if choice.0 {
            if previous.is_none_or(|old| !old.0) || self.on_desktop() {
                self.leave_desktop();
                self.use_desktop();
            }
        } else if self.on_desktop() || self.background_requested.get() {
            self.open_map();
        }
    }

    pub(super) fn on_desktop(&self) -> bool {
        self.desktop_attachment.borrow().is_some()
    }

    pub(super) fn hosting_command(&self, id: u16) -> bool {
        match id {
            ids::IDM_OPEN_MAP => self.open_map(),
            ids::IDM_DESKTOP => {
                let mut desired = self.app.preferences().committed;
                desired.desktop = !desired.desktop;
                self.app.save_preferences(desired);
            }
            _ => return false,
        }
        true
    }

    pub(super) fn use_desktop(&self) {
        self.cancel_split();
        if self.on_desktop() {
            return;
        }
        let window = self.window.get();
        if self.tray.get().is_none() {
            // SAFETY: The stock icon is borrowed for the Host lifetime.
            let result = unsafe { LoadIconW(None, IDI_APPLICATION) }
                .and_then(|icon| tray::Tray::new(window, TRAY_MESSAGE, icon));
            match result {
                Ok(tray) => {
                    let _ = self.tray.set(tray);
                }
                Err(error) => {
                    self.hosting_error(error.to_string());
                    return;
                }
            }
        }
        self.save_window_placement();
        self.background_requested.set(true);
        self.app.hover_map(None);
        self.apply_map_update(self.app.map.input(Input::Cancel));
        self.app.discover_desktop();
    }

    pub(super) fn accept_desktop(&self) {
        let Some(result) = self.app.take_desktop_result() else {
            return;
        };
        let requested = self.background_requested.replace(false);
        if !requested || !self.background_enabled() {
            return;
        }
        let result = result.and_then(|(attachment, monitors)| {
            let choice = self.app.preferences().committed.monitor;
            let monitor = monitors
                .iter()
                .find(|monitor| monitor.device == choice)
                .or_else(|| monitors.iter().find(|monitor| monitor.primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| "No connected display is available".to_string())?;
            self.ensure_map().map_err(|error| error.to_string())?;
            let attached = desktop::attach(self.map_window.get(), &attachment, monitor)
                .map_err(|error| error.to_string())?;
            self.desktop_monitor.replace(Some(monitor.clone()));
            self.desktop_attachment.replace(Some(attached));
            self.app.map.set_navigation_bounds(None);
            self.layout_desktop().map_err(|error| error.to_string())?;
            // SAFETY: Keep normal placement and controls hidden while the same
            // map HWND is displayed under Explorer. No focus change is made.
            unsafe {
                let _ = ShowWindow(self.window.get(), SW_HIDE);
            }
            self.notice.take();
            self.refresh_activity();
            Ok(())
        });
        if let Err(error) = result {
            self.leave_desktop();
            self.open_map();
            self.hosting_error(error);
        }
    }

    pub(super) fn layout_desktop(&self) -> Result<()> {
        let Some(monitor) = self.desktop_monitor.borrow().clone() else {
            return Ok(());
        };
        let map = self.map_window.get();
        // SAFETY: The existing map child is owned by this thread. Desktop
        // attachment already established its rectangle in Explorer coordinates.
        let dpi = unsafe { GetDpiForWindow(map) }.max(96);
        self.apply_map_update(self.app.map.resize(
            (monitor.rect.right - monitor.rect.left).max(0) as u32,
            (monitor.rect.bottom - monitor.rect.top).max(0) as u32,
            dpi,
        ));
        Ok(())
    }

    pub(super) fn leave_desktop(&self) {
        self.background_requested.set(false);
        let attached = self.desktop_attachment.take();
        if let Some(attached) = attached
            && let Err(error) = attached.detach()
        {
            self.hosting_error(error.to_string());
        }
        self.desktop_monitor.take();
    }

    pub(super) fn open_map(&self) {
        self.leave_desktop();
        if let Err(error) = self.ensure_map().and_then(|_| self.layout(true)) {
            self.fail(error);
            return;
        }
        // SAFETY: Open map is an explicit user command to activate our window.
        unsafe {
            let _ = ShowWindow(
                self.window.get(),
                if self.restore_maximized.get() {
                    SW_SHOWMAXIMIZED
                } else {
                    SW_SHOWNORMAL
                },
            );
            let _ = SetForegroundWindow(self.window.get());
        }
        self.refresh_activity();
    }

    pub(super) fn recover_desktop(&self) {
        if let Some(tray) = self.tray.get() {
            let _ = tray.readd();
        }
        // Recovery notifications can arrive again after detachment but before
        // the same outstanding discovery completes. Preserve that intent;
        // Application already coalesces discovery while its worker is pending.
        let requested = self.on_desktop() || self.background_requested.get();
        self.leave_desktop();
        if self.background_enabled() && requested {
            self.background_requested.set(true);
            self.app.discover_desktop();
        }
    }

    pub(super) fn tray_message(&self, lparam: LPARAM) {
        let event = (lparam.0 & 0xffff) as u32;
        if event == 0x400 || event == 0x401 || event == WM_LBUTTONDBLCLK {
            self.open_map();
        } else if event == WM_CONTEXTMENU {
            let command = self.tray.get().and_then(|tray| {
                tray.popup(
                    tray::Commands {
                        open: ids::IDM_OPEN_MAP,
                        desktop: ids::IDM_DESKTOP,
                        exit: ids::IDM_EXIT,
                    },
                    self.background_enabled(),
                )
                .ok()
                .flatten()
            });
            if let Some(command) = command {
                // SAFETY: Queue the selected command after native menu tracking
                // ends; normal dispatch rechecks the current hosting state.
                unsafe {
                    let _ = PostMessageW(
                        Some(self.window.get()),
                        WM_COMMAND,
                        WPARAM(command as usize),
                        LPARAM(0),
                    );
                }
            }
        }
    }

    pub(super) fn update_hosting_menu(&self, menu: HMENU) {
        // SAFETY: Only our existing native command state changes.
        unsafe {
            let _ = CheckMenuItem(
                menu,
                ids::IDM_DESKTOP as u32,
                (MF_BYCOMMAND
                    | if self.background_enabled() {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    })
                .0,
            );
        }
    }

    fn hosting_error(&self, message: String) {
        if let Some(tray) = self.tray.get() {
            let _ = tray.error(&message);
        }
        self.notice.replace(Some(message));
        self.refresh_status();
    }

    fn ensure_map(&self) -> Result<()> {
        if !self.map_window.get().is_invalid() {
            return Ok(());
        }
        // SAFETY: Recreate only our own child if Explorer destroyed its former
        // parent. The stable Host allocation and Application state survive.
        unsafe {
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
                Some(self.window.get()),
                Some(HMENU(ids::IDC_MAP as usize as *mut _)),
                Some(instance),
                Some((self as *const Host).cast()),
            )?;
            self.map_window.set(map);
            self.accessibility.attach(map)?;
            let tooltip = self.tooltip.replace(HWND::default());
            if !tooltip.is_invalid() {
                let _ = DestroyWindow(tooltip);
            }
            self.create_tooltip(instance, self.window.get(), map)?;
            self.app.map.attach(map)?;
            self.update_font()?;
        }
        Ok(())
    }
}
