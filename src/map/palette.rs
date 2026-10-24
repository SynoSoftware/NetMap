// Palette roles ported from TinyTorrent2 PeerMap.xaml and App.xaml at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: D2D1_COLOR_F,
    pub ink: D2D1_COLOR_F,
    pub land: D2D1_COLOR_F,
    pub edge: D2D1_COLOR_F,
    pub download: D2D1_COLOR_F,
    pub upload: D2D1_COLOR_F,
}

impl Palette {
    // TinyTorrent2 PeerMap's WinUI palette roles, with its upload swatches.
    // Accent text colors come from native Windows color settings.
    pub fn new(dark: bool, accent: D2D1_COLOR_F) -> Self {
        if dark {
            Self {
                background: argb(0xff2c2c2c),
                ink: argb(0xffffffff),
                land: argb(0x19000000),
                edge: argb(0x8bffffff),
                download: accent,
                upload: argb(0xffc5a3e6),
            }
        } else {
            Self {
                background: argb(0xffffffff),
                ink: argb(0xe4000000),
                land: argb(0x06000000),
                edge: argb(0x72000000),
                download: accent,
                upload: argb(0xff7756a3),
            }
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new(false, argb(0xff005fb8))
    }
}

pub fn argb(value: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((value >> 16) & 255) as f32 / 255.0,
        g: ((value >> 8) & 255) as f32 / 255.0,
        b: (value & 255) as f32 / 255.0,
        a: (value >> 24) as f32 / 255.0,
    }
}
