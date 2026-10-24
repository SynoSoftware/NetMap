use super::*;
use crate::map::Appearance;
use windows::Win32::{
    Graphics::{
        Direct2D::Common::D2D1_COLOR_F,
        Gdi::{COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor},
    },
    UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
};
use windows::core::BOOL;

impl Host {
    pub(super) fn refresh_appearance(&self) {
        if refresh(&self.app) {
            self.apply_map_update(crate::map::Update {
                changed: true,
                capture: self.app.map.capture(),
            });
        }
    }
}

pub(super) fn refresh(app: &Application) -> bool {
    app.refresh_appearance();
    let mut contrast = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let mut animations = BOOL(1);
    // SAFETY: Initialized outputs remain exclusively borrowed during these
    // system-setting queries. No settings are changed.
    let appearance = unsafe {
        let available = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
            Default::default(),
        )
        .is_ok();
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut animations as *mut BOOL).cast()),
            Default::default(),
        );
        Appearance {
            palette: app.map.appearance().palette,
            contrast: (available && (contrast.dwFlags & HCF_HIGHCONTRASTON) == HCF_HIGHCONTRASTON)
                .then(|| {
                    (
                        color(GetSysColor(COLOR_WINDOW)),
                        color(GetSysColor(COLOR_WINDOWTEXT)),
                    )
                }),
            animations: animations.as_bool(),
        }
    };
    app.map.set_appearance(appearance)
}

fn color(value: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (value & 255) as f32 / 255.0,
        g: ((value >> 8) & 255) as f32 / 255.0,
        b: ((value >> 16) & 255) as f32 / 255.0,
        a: 1.0,
    }
}
