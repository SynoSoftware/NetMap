use std::{marker::PhantomData, rc::Rc};
use windows::{
    Win32::{
        Foundation::HWND,
        System::{Com::*, Variant::VARIANT},
        UI::{
            Accessibility::*,
            WindowsAndMessaging::{EVENT_OBJECT_HELPCHANGE, OBJID_CLIENT},
        },
    },
    core::{HSTRING, PCWSTR, Result, w},
};

pub(super) struct Apartment(PhantomData<Rc<()>>);

impl Apartment {
    pub fn new(model: COINIT) -> Result<Self> {
        // SAFETY: The caller owns this thread's apartment and drops all COM
        // interfaces before the balancing guard. S_FALSE still needs balancing.
        unsafe {
            CoInitializeEx(None, model).ok()?;
        }
        Ok(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: Only a successful initialization constructs this guard.
        unsafe {
            CoUninitialize();
        }
    }
}

pub(super) struct Accessibility {
    properties: IAccPropServices,
}

impl Accessibility {
    pub fn new() -> Result<Self> {
        Ok(Self {
            // SAFETY: Host owns this in-process service on its initialized UI thread.
            properties: unsafe { CoCreateInstance(&CAccPropServices, None, CLSCTX_INPROC_SERVER)? },
        })
    }

    pub fn name(&self, window: HWND, name: PCWSTR) -> Result<()> {
        // SAFETY: Host supplies its live child and a valid string. The service
        // copies the name; Host clears the annotation before destroying the HWND.
        unsafe {
            for property in [PROPID_ACC_NAME, Name_Property_GUID] {
                self.properties
                    .SetHwndPropStr(window, OBJID_CLIENT.0 as u32, 0, property, name)?;
            }
        }
        Ok(())
    }

    pub fn attach(&self, window: HWND) -> Result<()> {
        self.name(window, w!("Network map"))?;
        // SAFETY: This is the live map's client, not another application's HWND.
        unsafe {
            self.properties.SetHwndPropStr(
                window,
                OBJID_CLIENT.0 as u32,
                0,
                AutomationId_Property_GUID,
                w!("NetMap.Map"),
            )?;
            self.properties.SetHwndProp(
                window,
                OBJID_CLIENT.0 as u32,
                0,
                PROPID_ACC_ROLE,
                &VARIANT::from(ROLE_SYSTEM_PANE as i32),
            )?;
            self.properties.SetHwndProp(
                window,
                OBJID_CLIENT.0 as u32,
                0,
                IsKeyboardFocusable_Property_GUID,
                &VARIANT::from(true),
            )?;
        }
        Ok(())
    }

    pub fn describe(&self, window: HWND, help: &HSTRING) -> Result<()> {
        // SAFETY: Annotation copies owned text. No model borrow spans these
        // calls or the notification, which can cause accessibility queries.
        unsafe {
            for property in [PROPID_ACC_HELP, HelpText_Property_GUID] {
                self.properties
                    .SetHwndPropStr(window, OBJID_CLIENT.0 as u32, 0, property, help)?;
            }
            NotifyWinEvent(EVENT_OBJECT_HELPCHANGE, window, OBJID_CLIENT.0, 0);
        }
        Ok(())
    }

    pub fn detach(&self, window: HWND) {
        // SAFETY: Clear before the map HWND is retired or could be reused.
        unsafe {
            let _ = self.properties.ClearHwndProps(
                window,
                OBJID_CLIENT.0 as u32,
                0,
                &[
                    PROPID_ACC_NAME,
                    Name_Property_GUID,
                    PROPID_ACC_ROLE,
                    PROPID_ACC_HELP,
                    AutomationId_Property_GUID,
                    IsKeyboardFocusable_Property_GUID,
                    HelpText_Property_GUID,
                ],
            );
        }
    }
}

#[cfg(test)]
mod tests;
