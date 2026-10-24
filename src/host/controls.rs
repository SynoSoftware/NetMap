use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::Gdi::{InvalidateRect, MapWindowPoints},
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::*,
        },
    },
    core::{BOOL, HSTRING, PWSTR},
};

pub fn item(parent: HWND, id: u16) -> HWND {
    // SAFETY: Called for a live UI-thread dialog and its resource-defined ID.
    unsafe { GetDlgItem(Some(parent), i32::from(id)).unwrap_or_default() }
}

pub fn text(window: HWND) -> String {
    let mut buffer = [0u16; 4097];
    // SAFETY: Synchronous UI-thread read into a writable, bounded buffer.
    let length = unsafe { GetWindowTextW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length as usize])
}

pub fn set_text(window: HWND, value: &str) {
    if text(window) != value {
        // SAFETY: The native call copies this owned string before returning.
        unsafe {
            let _ = SetWindowTextW(window, &HSTRING::from(value));
        }
    }
}

pub fn enable(window: HWND, enabled: bool) {
    // SAFETY: Only the creating UI thread changes its controls' state.
    unsafe {
        if IsWindowEnabled(window).as_bool() != enabled {
            let _ = EnableWindow(window, enabled);
        }
    }
}

pub fn visible(window: HWND, visible: bool) {
    // SAFETY: Inspect the control's own style, independent of its hidden parent.
    unsafe {
        if (GetWindowLongW(window, GWL_STYLE) as u32 & WS_VISIBLE.0 != 0) != visible {
            let _ = ShowWindow(window, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
}

pub fn select_edit(window: HWND) {
    // SAFETY: This is a live edit on the UI thread. Selection is an explicit
    // user command/validation effect rather than an ordinary status refresh.
    unsafe {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(window));
        SendMessageW(window, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
    }
}

pub fn status(window: HWND, part: usize, value: &str) {
    let mut buffer = [0u16; 4097];
    // SAFETY: Native status strings here are bounded by our owned captions.
    unsafe {
        let length =
            SendMessageW(window, SB_GETTEXTLENGTHW, Some(WPARAM(part)), None).0 as u16 as usize;
        if length < buffer.len() {
            SendMessageW(
                window,
                SB_GETTEXTW,
                Some(WPARAM(part)),
                Some(LPARAM(buffer.as_mut_ptr() as isize)),
            );
            if String::from_utf16_lossy(&buffer[..length]) == value {
                return;
            }
        }
        let text = HSTRING::from(value);
        SendMessageW(
            window,
            SB_SETTEXTW,
            Some(WPARAM(part)),
            Some(LPARAM(text.as_ptr() as isize)),
        );
    }
}

pub fn list_pages(window: HWND, pages: &[&str], selected: Option<usize>) {
    // SAFETY: Three small owned rows; ListView copies item text synchronously.
    unsafe {
        SendMessageW(window, LVM_DELETEALLITEMS, None, None);
        for (index, caption) in pages.iter().enumerate() {
            let caption = HSTRING::from(*caption);
            let row = LVITEMW {
                mask: LVIF_TEXT,
                iItem: index as i32,
                pszText: PWSTR(caption.as_ptr() as *mut _),
                ..Default::default()
            };
            SendMessageW(
                window,
                LVM_INSERTITEMW,
                None,
                Some(LPARAM(&row as *const _ as isize)),
            );
        }
        if let Some(index) = selected {
            let row = LVITEMW {
                stateMask: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
                state: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
                ..Default::default()
            };
            SendMessageW(
                window,
                LVM_SETITEMSTATE,
                Some(WPARAM(index)),
                Some(LPARAM(&row as *const _ as isize)),
            );
        }
    }
}

pub fn place(parent: HWND, rectangles: &[(HWND, RECT)]) {
    // SAFETY: Batch only changed children of this parent on their UI thread.
    unsafe {
        let changed: Vec<_> = rectangles
            .iter()
            .filter(|(window, desired)| {
                let mut current = RECT::default();
                if GetWindowRect(*window, &mut current).is_err() {
                    return false;
                }
                let mut points = [
                    POINT {
                        x: current.left,
                        y: current.top,
                    },
                    POINT {
                        x: current.right,
                        y: current.bottom,
                    },
                ];
                MapWindowPoints(None, Some(parent), &mut points);
                (points[0].x, points[0].y, points[1].x, points[1].y)
                    != (desired.left, desired.top, desired.right, desired.bottom)
            })
            .collect();
        if changed.is_empty() {
            return;
        }
        let Ok(mut batch) = BeginDeferWindowPos(changed.len() as i32) else {
            return;
        };
        for (window, rect) in changed {
            let Ok(next) = DeferWindowPos(
                batch,
                *window,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            ) else {
                return;
            };
            batch = next;
        }
        let _ = EndDeferWindowPos(batch);
    }
}

pub fn scroll(window: HWND, position: i32) {
    // SAFETY: Native scroll state owns the position. Move its children by the
    // accepted delta so keyboard, scrollbar and validation share one path.
    unsafe {
        let mut info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_POS,
            ..Default::default()
        };
        if GetScrollInfo(window, SB_VERT, &mut info).is_err() {
            return;
        }
        let previous = info.nPos;
        info.nPos = position;
        let current = SetScrollInfo(window, SB_VERT, &info, true);
        move_content(window, previous - current);
    }
}

pub fn scroll_range(window: HWND, content: i32, page: u32) {
    // SAFETY: Resize can clamp the native position; apply exactly that delta to
    // every direct child, including those entirely outside the old viewport.
    unsafe {
        let mut before = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_POS,
            ..Default::default()
        };
        let _ = GetScrollInfo(window, SB_VERT, &mut before);
        let info = SCROLLINFO {
            fMask: SIF_RANGE | SIF_PAGE,
            nMin: 0,
            nMax: content - 1,
            nPage: page,
            ..before
        };
        let after = SetScrollInfo(window, SB_VERT, &info, true);
        move_content(window, before.nPos - after);
    }
}

fn move_content(window: HWND, delta: i32) {
    if delta == 0 {
        return;
    }
    let mut children = Vec::<HWND>::new();
    // SAFETY: Enumeration is synchronous and borrows this local vector.
    unsafe extern "system" fn collect(child: HWND, parameter: LPARAM) -> BOOL {
        // SAFETY: The caller supplies a live, exclusively borrowed Vec<HWND>.
        unsafe {
            (&mut *(parameter.0 as *mut Vec<HWND>)).push(child);
        }
        BOOL(1)
    }
    // SAFETY: Read and batch only our page's direct children. Moving HWNDs
    // rather than scrolling pixels also handles fully clipped controls.
    unsafe {
        let _ = EnumChildWindows(
            Some(window),
            Some(collect),
            LPARAM(&mut children as *mut _ as isize),
        );
        let rectangles: Vec<_> = children
            .into_iter()
            .filter(|child| GetParent(*child).ok() == Some(window))
            .filter_map(|child| {
                let mut rect = RECT::default();
                GetWindowRect(child, &mut rect).ok()?;
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
                MapWindowPoints(None, Some(window), &mut points);
                Some((
                    child,
                    RECT {
                        left: points[0].x,
                        right: points[1].x,
                        top: points[0].y + delta,
                        bottom: points[1].y + delta,
                    },
                ))
            })
            .collect();
        place(window, &rectangles);
        let _ = InvalidateRect(Some(window), None, true);
    }
}

pub fn reveal(window: HWND, child: HWND) {
    // SAFETY: The child belongs to this scrolling page. Read its geometry on
    // the UI thread, with no feature/model borrow across native calls.
    unsafe {
        let mut rect = RECT::default();
        let mut client = RECT::default();
        let mut info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_POS,
            ..Default::default()
        };
        if GetWindowRect(child, &mut rect).is_err()
            || GetClientRect(window, &mut client).is_err()
            || GetScrollInfo(window, SB_VERT, &mut info).is_err()
        {
            return;
        }
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
        MapWindowPoints(None, Some(window), &mut points);
        let delta = if points[0].y < 0 {
            points[0].y
        } else {
            (points[1].y - client.bottom).max(0)
        };
        if delta != 0 {
            scroll(window, info.nPos + delta);
        }
    }
}
