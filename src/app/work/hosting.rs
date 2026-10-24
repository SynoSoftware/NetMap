use super::Reply;
use windows::{
    Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW},
    },
    core::w,
};

pub(super) fn info() -> Reply {
    let monitors = crate::host::desktop::monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| (monitor.device, monitor.primary))
        .collect();
    let saver = match std::env::current_exe().map(|path| path.with_file_name("netmap.scr")) {
        Err(error) => format!("Screen saver path unavailable: {error}"),
        Ok(path) if !path.is_file() => "Screen saver file is not present beside NetMap. Install the packaged distribution to use it.".into(),
        Ok(path) => match selected_saver() {
            Ok(None) => "Screen saver file is available. No screen saver is selected for this user.".into(),
            Ok(Some(selected)) => {
                let selected = std::path::Path::new(selected.trim_matches('"'));
                let same = selected.to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy());
                if same { "NetMap is selected in this user's Windows screen saver configuration.".into() }
                else { "Screen saver file is available. Another screen saver is selected for this user.".into() }
            }
            Err(error) => format!("Screen saver file is available; Windows selection could not be read: {error}"),
        },
    };
    Reply::HostingInfo { monitors, saver }
}

fn selected_saver() -> std::io::Result<Option<String>> {
    for key in [
        w!("Software\\Policies\\Microsoft\\Windows\\Control Panel\\Desktop"),
        w!("Control Panel\\Desktop"),
    ] {
        let mut bytes = 0u32;
        // SAFETY: Worker-only read of a bounded current-user string, no registry
        // mutation. The policy value, when present, takes precedence.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                w!("SCRNSAVE.EXE"),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            continue;
        }
        status
            .ok()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if bytes > 65536 || !bytes.is_multiple_of(2) {
            return Err(std::io::Error::other("Invalid screen saver selection"));
        }
        let mut value = vec![0u16; (bytes as usize / 2).max(1)];
        // SAFETY: The requested byte capacity is backed by this initialized Vec.
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                w!("SCRNSAVE.EXE"),
                RRF_RT_REG_SZ,
                None,
                Some(value.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        }
        .ok()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
        value.truncate(bytes as usize / 2);
        while value.last() == Some(&0) {
            value.pop();
        }
        let text =
            String::from_utf16(&value).map_err(|error| std::io::Error::other(error.to_string()))?;
        return Ok((!text.is_empty()).then_some(text));
    }
    Ok(None)
}
