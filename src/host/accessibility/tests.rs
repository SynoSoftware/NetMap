use super::*;
use windows::{
    Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId},
    core::Error,
};

#[test]
#[ignore = "Queries the owned live NetMap HWND supplied by the native journey through UI Automation"]
fn native_map_has_a_stable_name_current_help_and_keyboard_access() -> Result<()> {
    let address: usize = std::env::var("NETMAP_UIA_WINDOW")
        .map_err(|_| {
            Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Missing owned map HWND",
            )
        })?
        .parse()
        .map_err(|_| {
            Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Invalid owned map HWND",
            )
        })?;
    let pid: u32 = std::env::var("NETMAP_UIA_PROCESS")
        .unwrap_or_default()
        .parse()
        .map_err(|_| {
            Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "Missing owned process ID",
            )
        })?;
    let window = HWND(address as *mut _);
    let _apartment = Apartment::new(COINIT_MULTITHREADED)?;
    // SAFETY: This separate test process queries only the verified journey HWND.
    // No UI thread waits for these cross-process COM calls; the journey bounds
    // this client process independently if a native call stops responding.
    unsafe {
        let mut owner = 0;
        GetWindowThreadProcessId(window, Some(&mut owner));
        assert_eq!(owner, pid, "Target is no longer the owned map window");
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
        let element = automation.ElementFromHandle(window)?;
        assert_eq!(element.CurrentProcessId()? as u32, pid);
        assert_eq!(element.CurrentClassName()?.to_string(), "NetMap.Map");
        assert_eq!(element.CurrentAutomationId()?.to_string(), "NetMap.Map");
        let name = element.CurrentName()?.to_string();
        let help = element.CurrentHelpText()?.to_string();
        assert!(
            !name.is_empty() && name.len() < 80,
            "Map needs a stable, concise accessible name: {name}"
        );
        assert!(
            help.contains("Arrow keys") && help.contains("Ctrl+0"),
            "Map help must expose its keyboard route: {help}"
        );
        assert!(element.CurrentIsKeyboardFocusable()?.as_bool());
        assert!(element.CurrentIsEnabled()?.as_bool());
        assert_eq!(element.CurrentControlType()?, UIA_PaneControlTypeId);
        let rect = element.CurrentBoundingRectangle()?;
        assert!(rect.right > rect.left && rect.bottom > rect.top);
        let legacy: IUIAutomationLegacyIAccessiblePattern =
            element.GetCurrentPatternAs(UIA_LegacyIAccessiblePatternId)?;
        assert_eq!(legacy.CurrentRole()?, ROLE_SYSTEM_PANE);
        assert!(!legacy.CurrentName()?.is_empty());
        let mut caption = [0_u16; 4096];
        let length = GetWindowTextW(window, &mut caption).max(0) as usize;
        assert!(String::from_utf16_lossy(&caption[..length]).contains("Arrow keys"));
        eprintln!(
            "UIA: name={name}; id=NetMap.Map; pane; keyboard-focusable; bounds={}x{}; help={help}",
            rect.right - rect.left,
            rect.bottom - rect.top
        );
    }
    Ok(())
}
