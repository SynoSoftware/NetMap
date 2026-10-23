use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DialogBoxParamW, EndDialog, GetClientRect, GetDlgItem, IDCANCEL, MB_ICONERROR, MB_OK,
            MapDialogRect, MessageBoxW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_CLOSE,
            WM_COMMAND, WM_INITDIALOG, WM_SIZE,
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

mod ids;

pub fn run() -> Result<()> {
    // SAFETY: The executable owns the embedded template. The callback stores no
    // borrowed state and Windows runs it on this thread until the dialog ends.
    let result = unsafe {
        let instance = GetModuleHandleW(None)?.into();
        DialogBoxParamW(
            Some(instance),
            PCWSTR(ids::IDD_BOOTSTRAP as usize as *const u16),
            None,
            Some(window_proc),
            LPARAM(0),
        )
    };
    if result == -1 {
        return Err(Error::from_thread());
    }
    Ok(())
}

pub fn show_startup_error(error: &Error) {
    // SAFETY: The strings remain alive for the native modal call; no model is borrowed.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(error.to_string()),
            w!("NetMap"),
            MB_OK | MB_ICONERROR,
        );
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> isize {
    match message {
        WM_INITDIALOG | WM_SIZE => {
            let _ = layout(window);
            isize::from(message == WM_INITDIALOG)
        }
        WM_CLOSE => {
            // SAFETY: Windows supplies this live dialog HWND to its own callback.
            let _ = unsafe { EndDialog(window, 0) };
            1
        }
        WM_COMMAND if (wparam.0 & 0xffff) == IDCANCEL.0 as usize => {
            // SAFETY: IDCANCEL, including Escape, ends this live modal dialog.
            let _ = unsafe { EndDialog(window, 0) };
            1
        }
        _ => 0,
    }
}

fn layout(window: HWND) -> Result<()> {
    let mut client = RECT::default();
    let mut margin = RECT {
        right: 12,
        bottom: 12,
        ..RECT::default()
    };
    // SAFETY: All handles belong to this UI thread. Both RECT pointers are valid
    // for the calls, and the child remains owned by the dialog throughout layout.
    unsafe {
        GetClientRect(window, &mut client)?;
        MapDialogRect(window, &mut margin)?;
        SetWindowPos(
            GetDlgItem(Some(window), i32::from(ids::IDC_BOOTSTRAP_NOTICE))?,
            None,
            margin.right,
            margin.bottom,
            (client.right - 2 * margin.right).max(0),
            (client.bottom - 2 * margin.bottom).max(0),
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
    }
    Ok(())
}
