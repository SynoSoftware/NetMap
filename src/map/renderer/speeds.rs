// Speed presentation ported from TinyTorrent2 InspectorPane.xaml and Peers.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::device::Device;
use crate::{
    connections::Rates,
    map::{Prepared, SpeedCorner},
};
use windows::{
    Win32::Graphics::{
        Direct2D::{D2D1_DRAW_TEXT_OPTIONS_CLIP, ID2D1SolidColorBrush},
        DirectWrite::*,
    },
    core::{Result, w},
};
use windows_numerics::{Matrix3x2, Vector2};

pub(super) struct Speeds {
    factory: IDWriteFactory,
    format: IDWriteTextFormat,
    caption: IDWriteTextLayout,
    ink: ID2D1SolidColorBrush,
    down: ID2D1SolidColorBrush,
    up: ID2D1SolidColorBrush,
    value: Option<Value>,
}

struct Value {
    rates: Option<Rates>,
    available: f32,
    layout: IDWriteTextLayout,
    width: f32,
    height: f32,
}

impl Speeds {
    pub fn new(device: &Device) -> Result<Self> {
        // SAFETY: Text resources and brushes remain on their creating UI thread.
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let create = |size| {
                factory.CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!(""),
                )
            };
            let format = create(20.0)?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let caption = factory.CreateTextLayout(
                &"Speed".encode_utf16().collect::<Vec<_>>(),
                &create(12.0)?,
                256.0,
                20.0,
            )?;
            let color = crate::map::Palette::default().ink;
            Ok(Self {
                factory,
                format,
                caption,
                ink: device.context.CreateSolidColorBrush(&color, None)?,
                down: device.context.CreateSolidColorBrush(&color, None)?,
                up: device.context.CreateSolidColorBrush(&color, None)?,
                value: None,
            })
        }
    }

    pub fn prepare(&mut self, prepared: &Prepared) -> Result<()> {
        let colors = prepared.appearance.colors();
        let available = (prepared.viewport.size.X - 32.0).max(1.0);
        // SAFETY: Brushes/layouts belong to this render thread; effects retain
        // their brush references. Rates are formatted only when facts change.
        unsafe {
            self.ink.SetColor(&colors.ink);
            self.down.SetColor(&colors.download);
            self.up.SetColor(&colors.upload);
            if self.value.as_ref().is_some_and(|value| {
                value.rates == prepared.speeds.rates && value.available == available
            }) {
                return Ok(());
            }
            let (text, received, sent) = match prepared.speeds.rates {
                Some(rates) => {
                    let down = format!("↓ {}", amount(rates.received));
                    let up = format!("↑ {}", amount(rates.sent));
                    let received = down.encode_utf16().count() as u32;
                    let sent = up.encode_utf16().count() as u32;
                    (format!("{down} · {up}"), received, sent)
                }
                None => ("Traffic unavailable".to_string(), 0, 0),
            };
            let layout = self.factory.CreateTextLayout(
                &text.encode_utf16().collect::<Vec<_>>(),
                &self.format,
                available,
                32.0,
            )?;
            if received > 0 {
                layout.SetDrawingEffect(
                    &self.down,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: received,
                    },
                )?;
                layout.SetDrawingEffect(
                    &self.up,
                    DWRITE_TEXT_RANGE {
                        startPosition: received + 3,
                        length: sent,
                    },
                )?;
            }
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            self.value = Some(Value {
                rates: prepared.speeds.rates,
                available,
                layout,
                width: metrics.width.min(available),
                height: metrics.height + 20.0,
            });
        }
        Ok(())
    }

    pub fn draw(&self, device: &Device, prepared: &Prepared) {
        let Some(value) = &self.value else {
            return;
        };
        let size = prepared.viewport.size;
        if size.X < 64.0 || size.Y < value.height + 32.0 {
            return;
        }
        let x = match prepared.speeds.corner {
            SpeedCorner::BottomRight => (size.X - 16.0 - value.width).max(16.0),
            _ => 16.0,
        };
        let mut y = match prepared.speeds.corner {
            SpeedCorner::TopLeft => 16.0,
            _ => (size.Y - 16.0 - value.height).max(16.0),
        };
        if let Some(bounds) = prepared.navigation_bounds
            && x < bounds.right
            && x + value.width > bounds.left
            && y < bounds.bottom
            && y + value.height > bounds.top
        {
            y = bounds.top - 16.0 - value.height;
            if y < 16.0 {
                return;
            }
        }
        // SAFETY: Main BeginDraw is live and both text layouts are retained.
        unsafe {
            device.context.SetTransform(&Matrix3x2::identity());
            device.context.DrawTextLayout(
                Vector2::new(x, y),
                &self.caption,
                &self.ink,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
            device.context.DrawTextLayout(
                Vector2::new(x, y + 20.0),
                &value.layout,
                &self.ink,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }
}

fn amount(rate: f64) -> String {
    if rate >= 1024.0 * 1024.0 {
        format!("{:.1} MiB/s", rate / (1024.0 * 1024.0))
    } else if rate >= 1024.0 {
        format!("{:.1} KiB/s", rate / 1024.0)
    } else {
        format!("{rate:.0} B/s")
    }
}
