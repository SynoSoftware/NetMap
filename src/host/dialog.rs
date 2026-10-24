use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT, SIZE, WPARAM},
        Graphics::Gdi::{
            CreateFontIndirectW, DT_CALCRECT, DT_NOPREFIX, DT_WORDBREAK, DeleteObject, DrawTextW,
            GetDC, GetMonitorInfoW, GetTextExtentPoint32W, HFONT, LOGFONTW,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect, ReleaseDC, SelectObject,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::{Error, PCWSTR, Result},
};

pub fn modal(owner: HWND, template: u16, procedure: DLGPROC, context: LPARAM) -> Result<()> {
    // SAFETY: The feature owns the pointed-to state across this native modal
    // loop, on the same thread as its owner. Workers use the application endpoint.
    let result = unsafe {
        DialogBoxParamW(
            Some(GetModuleHandleW(None)?.into()),
            PCWSTR(template as usize as *const u16),
            Some(owner),
            procedure,
            context,
        )
    };
    if result == -1 {
        Err(Error::from_thread())
    } else {
        Ok(())
    }
}

fn work_area(rect: &RECT) -> Option<RECT> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: The nearest monitor is selected from physical window coordinates;
    // the initialized record is filled synchronously on the owning UI thread.
    unsafe { GetMonitorInfoW(MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST), &mut info) }
        .as_bool()
        .then_some(info.rcWork)
}

fn constrained(rect: RECT, work: RECT) -> RECT {
    let width = (rect.right - rect.left).min(work.right - work.left);
    let height = (rect.bottom - rect.top).min(work.bottom - work.top);
    let left = rect.left.clamp(work.left, work.right - width);
    let top = rect.top.clamp(work.top, work.bottom - height);
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

pub fn fit_work_area(window: HWND, suggested: Option<RECT>) {
    let mut current = RECT::default();
    // SAFETY: Reads and moves this feature's live top-level HWND only. Native
    // resize messages may reenter; no feature borrow is held by this helper.
    unsafe {
        if GetWindowRect(window, &mut current).is_err() {
            return;
        }
        let requested = suggested.unwrap_or(current);
        let Some(work) = work_area(&requested) else {
            return;
        };
        let rect = constrained(requested, work);
        if rect != current {
            let _ = SetWindowPos(
                window,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    }
}

pub fn minimum(window: HWND, width: i32, height: i32) -> SIZE {
    let size = units(window, 0, 0, width, height);
    let mut outer = RECT::default();
    let mut client = RECT::default();
    // SAFETY: Both rectangles belong to this live dialog, in physical pixels.
    unsafe {
        let _ = GetWindowRect(window, &mut outer);
        let _ = GetClientRect(window, &mut client);
    }
    let mut result = SIZE {
        cx: size.right + outer.right - outer.left - client.right,
        cy: size.bottom + outer.bottom - outer.top - client.bottom,
    };
    if let Some(work) = work_area(&outer) {
        result.cx = result.cx.min(work.right - work.left);
        result.cy = result.cy.min(work.bottom - work.top);
    }
    result
}

pub fn wrapped_height(window: HWND, width: i32) -> i32 {
    let mut text: Vec<u16> = super::controls::text(window).encode_utf16().collect();
    if text.is_empty() {
        // DrawTextW can dereference an empty slice's dangling pointer even
        // with a zero character count. Empty labels need no native measurement.
        return 0;
    }
    let mut rect = RECT {
        right: width.max(1),
        ..Default::default()
    };
    // SAFETY: Measure the control's actual font/text without changing its state.
    // Native drawing receives only this local UTF-16 buffer and temporary DC.
    unsafe {
        let dc = GetDC(Some(window));
        if dc.is_invalid() {
            return 0;
        }
        let font = SendMessageW(window, WM_GETFONT, None, None);
        let old = SelectObject(dc, HFONT(font.0 as *mut _).into());
        DrawTextW(
            dc,
            &mut text,
            &mut rect,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
        );
        SelectObject(dc, old);
        ReleaseDC(Some(window), dc);
    }
    rect.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_native_labels_skip_draw_text_and_remain_measurable_after_text_changes() {
        // SAFETY: This hidden predefined control belongs to this test thread;
        // all text, measurement and destruction calls stay on that same thread.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!(""),
                WS_POPUP,
                0,
                0,
                200,
                40,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        let empty = wrapped_height(window, 200);
        super::super::controls::set_text(window, "Location status");
        let populated = wrapped_height(window, 200);
        super::super::controls::set_text(window, "");
        let cleared = wrapped_height(window, 200);
        // SAFETY: The test owns this live hidden HWND on its creating thread.
        unsafe { DestroyWindow(window) }.unwrap();
        assert_eq!(empty, 0);
        assert!(populated > 0);
        assert_eq!(cleared, 0);
    }

    #[test]
    fn oversized_dialog_fits_a_negative_origin_work_area_without_losing_its_footer() {
        let work = RECT {
            left: -1280,
            top: 30,
            right: 0,
            bottom: 750,
        };
        let rect = constrained(
            RECT {
                left: -1500,
                top: -200,
                right: 100,
                bottom: 900,
            },
            work,
        );
        assert_eq!(rect, work);
        let small = constrained(
            RECT {
                left: -200,
                top: 600,
                right: 100,
                bottom: 900,
            },
            work,
        );
        assert_eq!(
            small,
            RECT {
                left: -300,
                top: 450,
                right: 0,
                bottom: 750
            }
        );
    }
}

pub fn page(parent: HWND, template: u16, procedure: DLGPROC, context: LPARAM) -> Result<HWND> {
    // SAFETY: Child pages retain the same stable feature pointer as the shell,
    // and are destroyed by their parent before that feature state is dropped.
    unsafe {
        CreateDialogParamW(
            Some(GetModuleHandleW(None)?.into()),
            PCWSTR(template as usize as *const u16),
            Some(parent),
            procedure,
            context,
        )
    }
}

pub fn units(window: HWND, x: i32, y: i32, width: i32, height: i32) -> RECT {
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    // SAFETY: A live native dialog supplies its font-derived dialog unit scale.
    unsafe {
        let _ = MapDialogRect(window, &mut rect);
    }
    rect
}

#[derive(Clone, Copy)]
pub struct Fonts {
    pub normal: HFONT,
    pub bold: HFONT,
}

impl Fonts {
    pub fn new(window: HWND) -> Result<Self> {
        // SAFETY: Independent GDI fonts remain owned until every using HWND is
        // destroyed or has received their replacement on this UI thread.
        unsafe {
            let mut font = LOGFONTW {
                lfHeight: -(9 * GetDpiForWindow(window) as i32 / 72),
                lfWeight: 400,
                ..Default::default()
            };
            for (slot, value) in font.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
                *slot = value;
            }
            let normal = CreateFontIndirectW(&font);
            if normal.is_invalid() {
                return Err(Error::from_thread());
            }
            font.lfWeight = 700;
            let bold = CreateFontIndirectW(&font);
            if bold.is_invalid() {
                let _ = DeleteObject(normal.into());
                return Err(Error::from_thread());
            }
            Ok(Self { normal, bold })
        }
    }

    pub fn release(self) {
        // SAFETY: The owner has destroyed or replaced all font-using HWNDs.
        unsafe {
            let _ = DeleteObject(self.normal.into());
            let _ = DeleteObject(self.bold.into());
        }
    }
}

pub fn font(window: HWND, font: HFONT) {
    // SAFETY: Font stays owned by the feature through this HWND's use.
    unsafe {
        SendMessageW(
            window,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

pub fn text_width(window: HWND, font: HFONT, captions: &[&str]) -> i32 {
    // SAFETY: A temporary UI-thread DC measures the complete native catalogue.
    // Restore its selected object before release; the feature owns the font.
    unsafe {
        let dc = GetDC(Some(window));
        if dc.is_invalid() {
            return 0;
        }
        let previous = SelectObject(dc, font.into());
        let width = captions
            .iter()
            .map(|caption| {
                let mut size = SIZE::default();
                let _ = GetTextExtentPoint32W(
                    dc,
                    &caption.encode_utf16().collect::<Vec<_>>(),
                    &mut size,
                );
                size.cx
            })
            .max()
            .unwrap_or(0);
        SelectObject(dc, previous);
        ReleaseDC(Some(window), dc);
        width
    }
}
