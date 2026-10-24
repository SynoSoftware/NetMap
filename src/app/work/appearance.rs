use crate::map::{Palette, palette::argb};
use windows::{
    UI::ViewManagement::{UIColorType, UISettings},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::Result,
};

pub(super) fn read() -> Result<Palette> {
    // SAFETY: This registered worker initializes and balances its own WinRT
    // apartment; all UISettings interfaces drop before uninitialization.
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    let result = (|| {
        let settings = UISettings::new()?;
        let background = settings.GetColorValue(UIColorType::Background)?;
        let dark =
            u32::from(background.R) + u32::from(background.G) + u32::from(background.B) < 384;
        let accent = settings.GetColorValue(if dark {
            UIColorType::AccentLight3
        } else {
            UIColorType::AccentDark2
        })?;
        Ok(Palette::new(
            dark,
            argb(u32::from_be_bytes([accent.A, accent.R, accent.G, accent.B])),
        ))
    })();
    // SAFETY: All apartment-affine objects were scoped inside the closure.
    unsafe {
        RoUninitialize();
    }
    result
}
