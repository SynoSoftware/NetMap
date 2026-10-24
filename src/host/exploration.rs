use super::*;
use crate::app::exploration::Event;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    Map,
    Table,
    Both,
}

#[derive(Clone, Copy)]
pub(super) struct SplitDrag {
    stacked: bool,
    fraction: f64,
    pointer: i32,
}

impl Host {
    pub(super) fn exploration_layout(
        &self,
        width: i32,
        height: i32,
        dpi: u32,
        fit: bool,
    ) -> (RECT, RECT) {
        self.split_bounds.set(RECT::default());
        let full = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        match self.view.get() {
            View::Map => (full, RECT::default()),
            View::Table => (RECT::default(), full),
            View::Both => {
                let gutter = (16 * dpi.max(96) / 96) as i32;
                let side =
                    ((width - gutter).max(0) as f64 * (1.0 - self.side_fraction.get())) as i32;
                let below =
                    ((height - gutter).max(0) as f64 * (1.0 - self.stacked_fraction.get())) as i32;
                let scale = |w: i32, h: i32| {
                    ((w as f64 * 96.0 / dpi.max(1) as f64 - 24.0) / 360.0)
                        .min((h as f64 * 96.0 / dpi.max(1) as f64 - 20.0) / 180.0)
                        .max(0.0)
                };
                let (side_fit, below_fit) = (scale(side, height), scale(width, below));
                if fit && self.split_drag.get().is_none() && side_fit != below_fit {
                    self.stacked.set(below_fit > side_fit);
                }
                if self.stacked.get() {
                    self.split_bounds.set(RECT {
                        left: 0,
                        top: below,
                        right: width,
                        bottom: (below + gutter).min(height),
                    });
                    (
                        RECT {
                            bottom: below,
                            ..full
                        },
                        RECT {
                            top: (below + gutter).min(height),
                            ..full
                        },
                    )
                } else {
                    self.split_bounds.set(RECT {
                        left: side,
                        top: 0,
                        right: (side + gutter).min(width),
                        bottom: height,
                    });
                    (
                        RECT {
                            right: side,
                            ..full
                        },
                        RECT {
                            left: (side + gutter).min(width),
                            ..full
                        },
                    )
                }
            }
        }
    }

    pub(super) fn place_exploration(&self, rect: RECT) -> Result<()> {
        let Some(table) = self.exploration.get() else {
            return Ok(());
        };
        // SAFETY: Both children belong to this UI thread; their native controls
        // paint themselves and retain selection, column widths and scroll.
        unsafe {
            let _ = ShowWindow(
                self.map_window.get(),
                if self.view.get() == View::Table {
                    SW_HIDE
                } else {
                    SW_SHOWNOACTIVATE
                },
            );
            let _ = ShowWindow(
                table.window(),
                if self.view.get() == View::Map {
                    SW_HIDE
                } else {
                    SW_SHOWNOACTIVATE
                },
            );
            if self.view.get() != View::Map {
                SetWindowPos(
                    table.window(),
                    None,
                    rect.left,
                    rect.top,
                    (rect.right - rect.left).max(0),
                    (rect.bottom - rect.top).max(0),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn refresh_exploration(&self) {
        if self.on_desktop() {
            return;
        }
        if self.app.take_source_reset()
            && let Some(table) = self.exploration.get()
        {
            table.clear_selection();
        }
        if self.app.take_exploration_request() {
            self.view.set(View::Both);
            if let Err(error) = self.layout(true) {
                self.fail(error);
                return;
            }
        }
        if let (Some(table), Some(rows)) = (self.exploration.get(), self.app.exploration_rows()) {
            let (selected, retired) =
                table.set_rows(rows.clone(), self.app.exploration_filter().as_ref());
            if let Some(retired) = retired {
                self.app.retire_rows(retired);
            }
            if let Some(id) = selected {
                self.app.reconcile_row_place(rows.place(id));
            }
        }
    }

    pub(super) fn exploration_command(&self, id: u16) -> bool {
        let view = match id {
            ids::IDM_MAP_ONLY => Some(View::Map),
            ids::IDM_TABLE_ONLY => Some(View::Table),
            ids::IDM_MAP_TABLE | ids::IDM_ALL_CONNECTIONS => Some(View::Both),
            _ => None,
        };
        if let Some(view) = view {
            self.cancel_split();
            self.view.set(view);
            if id == ids::IDM_ALL_CONNECTIONS || view == View::Table {
                self.clear_exploration();
            }
            if let Err(error) = self.layout(true) {
                self.fail(error);
            }
            self.refresh_exploration();
            // SAFETY: Focus follows the visible native control after layout.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(
                    if view == View::Table {
                        self.exploration
                            .get()
                            .map_or(self.map_window.get(), |table| table.window())
                    } else {
                        self.map_window.get()
                    },
                ));
            }
            return true;
        }
        if matches!(id, ids::IDM_MORE_MAP | ids::IDM_MORE_TABLE) {
            let fraction = if self.stacked.get() {
                &self.stacked_fraction
            } else {
                &self.side_fraction
            };
            fraction.set(
                (fraction.get()
                    + if id == ids::IDM_MORE_TABLE {
                        0.05
                    } else {
                        -0.05
                    })
                .clamp(0.15, 0.85),
            );
            if let Err(error) = self.layout(false) {
                self.fail(error);
            }
            return true;
        }
        false
    }

    pub(super) fn clear_exploration(&self) {
        if let Some(table) = self.exploration.get() {
            table.clear_selection();
            if let (_, Some(retired)) = table.filter(None) {
                self.app.retire_rows(retired);
            }
        }
        self.app.clear_inspection();
    }

    pub(super) fn exploration_notification(&self, lparam: LPARAM) -> Option<LRESULT> {
        if lparam.0 == 0 {
            return None;
        }
        let table = self.exploration.get()?;
        // SAFETY: WM_NOTIFY supplies a live header; the feature checks its HWND
        // and notification code before reading a larger native payload.
        let (result, event) = unsafe { table.notify(lparam.0 as *mut _) }?;
        match event {
            Some(Event::Selected(id)) => {
                let place = id.and_then(|id| self.app.exploration_rows()?.place(id));
                self.app.select_place(place);
            }
            Some(Event::Clear) => self.clear_exploration(),
            None => {}
        }
        Some(result)
    }

    pub(super) fn update_exploration_menu(&self, menu: HMENU) {
        // SAFETY: Refresh only our existing native command checks.
        unsafe {
            for (id, view) in [
                (ids::IDM_MAP_ONLY, View::Map),
                (ids::IDM_TABLE_ONLY, View::Table),
                (ids::IDM_MAP_TABLE, View::Both),
            ] {
                let _ = CheckMenuItem(
                    menu,
                    id as u32,
                    (MF_BYCOMMAND
                        | if self.view.get() == view {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        })
                    .0,
                );
            }
        }
    }

    /// The native host's unused client gutter supplies split interaction. The
    /// standard window background and child controls keep their own painting.
    pub(super) fn split_input(&self, message: u32, lparam: LPARAM) -> Option<LRESULT> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
        if self.on_desktop() || self.view.get() != View::Both {
            return None;
        }
        let x = lparam.0 as i16 as i32;
        let y = (lparam.0 >> 16) as i16 as i32;
        let bounds = self.split_bounds.get();
        let inside = x >= bounds.left && x < bounds.right && y >= bounds.top && y < bounds.bottom;
        match message {
            WM_LBUTTONDOWN if inside => {
                let stacked = self.stacked.get();
                let fraction = if stacked {
                    self.stacked_fraction.get()
                } else {
                    self.side_fraction.get()
                };
                // SAFETY: Capture only our own UI-thread host. Store the drag
                // after native acceptance; nested capture notifications see none.
                unsafe {
                    let _ = SetCapture(self.window.get());
                }
                // SAFETY: Non-messaging query of this thread's current capture.
                if unsafe { GetCapture() } == self.window.get() {
                    self.split_drag.set(Some(SplitDrag {
                        stacked,
                        fraction,
                        pointer: if stacked { y } else { x },
                    }));
                }
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE if self.split_drag.get().is_some() || inside => {
                let stacked = self
                    .split_drag
                    .get()
                    .map_or(self.stacked.get(), |drag| drag.stacked);
                // SAFETY: Set only the cursor for this host's native client area.
                unsafe {
                    if let Ok(cursor) =
                        LoadCursorW(None, if stacked { IDC_SIZENS } else { IDC_SIZEWE })
                    {
                        SetCursor(Some(cursor));
                    }
                }
                if let Some(drag) = self.split_drag.get() {
                    let table = self.exploration.get()?;
                    let mut rect = RECT::default();
                    // SAFETY: Native child bounds are read on their owning thread.
                    unsafe {
                        if GetWindowRect(table.window(), &mut rect).is_err() {
                            return Some(LRESULT(0));
                        }
                    }
                    let extent = if stacked {
                        bounds.top + rect.bottom - rect.top
                    } else {
                        bounds.left + rect.right - rect.left
                    };
                    if extent > 0 {
                        let fraction = (drag.fraction
                            - f64::from((if stacked { y } else { x }) - drag.pointer)
                                / f64::from(extent))
                        .clamp(0.15, 0.85);
                        if stacked {
                            self.stacked_fraction.set(fraction);
                        } else {
                            self.side_fraction.set(fraction);
                        }
                        if let Err(error) = self.layout(false) {
                            self.fail(error);
                        }
                    }
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP if self.split_drag.take().is_some() => {
                // SAFETY: Retire drag state before ReleaseCapture can reenter.
                unsafe {
                    if GetCapture() == self.window.get() {
                        let _ = ReleaseCapture();
                    }
                }
                Some(LRESULT(0))
            }
            WM_CANCELMODE | WM_CAPTURECHANGED if self.split_drag.get().is_some() => {
                self.cancel_split();
                Some(LRESULT(0))
            }
            _ => None,
        }
    }

    pub(super) fn cancel_split(&self) {
        let Some(drag) = self.split_drag.take() else {
            return;
        };
        if drag.stacked {
            self.stacked_fraction.set(drag.fraction);
        } else {
            self.side_fraction.set(drag.fraction);
        }
        // SAFETY: Release only our own host's capture, after ending the drag.
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture};
            if GetCapture() == self.window.get() {
                let _ = ReleaseCapture();
            }
        }
        if let Err(error) = self.layout(false) {
            self.fail(error);
        }
    }
}
