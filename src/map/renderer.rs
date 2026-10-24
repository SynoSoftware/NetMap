// Ported from TinyTorrent2 MapRenderer.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::{Frame, Prepared, model::DestinationKey, viewport::Viewport};
use crate::connections::Highlight;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::D2DERR_RECREATE_TARGET,
    Graphics::{
        Dxgi::{DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET},
        Gdi::InvalidateRect,
    },
};
use windows::{Win32::Foundation::HWND, core::Result};
use windows_numerics::Vector2;

mod device;
mod outlines;
mod routes;
mod speeds;

#[derive(Default)]
pub struct Renderer {
    #[cfg(test)]
    window: HWND,
    device: Option<device::Device>,
    targets: HashMap<isize, Target>,
    outlines: Option<outlines::Paths>,
    guides: routes::Paths,
}

#[derive(Default)]
struct Target {
    outlines: Option<outlines::Outlines>,
    routes: Option<routes::Routes>,
    speeds: Option<speeds::Speeds>,
}

// Identities are observed only while these resources remain owned by a live
// renderer. They establish reuse, not lifetime-wide allocation counts.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRendering {
    pub targets: usize,
    pub shapes: Vec<usize>,
    pub fills: Vec<usize>,
    pub strokes: Vec<usize>,
    pub fill_window: [f32; 4],
    pub detail: f32,
    pub guides: Vec<(DestinationKey, usize)>,
    pub route_strokes: Vec<usize>,
    pub glow: usize,
    pub dots: usize,
}

impl Renderer {
    #[cfg(test)]
    pub(super) fn native_rendering(&self) -> Option<NativeRendering> {
        let target = self.targets.values().next()?;
        let mut value = NativeRendering {
            targets: self.targets.len(),
            shapes: Vec::new(),
            fills: Vec::new(),
            strokes: Vec::new(),
            fill_window: [0.0; 4],
            detail: 0.0,
            guides: Vec::new(),
            route_strokes: Vec::new(),
            glow: 0,
            dots: 0,
        };
        if let Some(outlines) = &target.outlines {
            outlines.native_rendering(&mut value);
        }
        if let Some(routes) = &target.routes {
            routes.native_rendering(&mut value);
        }
        Some(value)
    }

    #[cfg(test)]
    pub(super) fn native_image(&self, path: &std::path::Path) -> Result<()> {
        let observations = self
            .device
            .as_ref()
            .and_then(|device| device.observations.as_ref())
            .ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "Native image requires an observed live map target",
                )
            })?;
        let mut capture = observations.capture.borrow_mut();
        if capture.is_some() {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Native image request is already armed",
            ));
        }
        observations.captured.set(None);
        *capture = Some(path.to_path_buf());
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn observe_presentations(
        &mut self,
        observations: std::rc::Rc<super::Presentations>,
    ) -> Result<()> {
        let device = self.device.as_mut().ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Native check requires an attached map device",
            )
        })?;
        device.observations = Some(observations);
        Ok(())
    }

    #[cfg(test)]
    pub fn hit(
        &self,
        view: &Viewport,
        position: Vector2,
        now: Duration,
        highlight: Highlight,
        static_marks: bool,
    ) -> Option<DestinationKey> {
        self.hit_target(self.window, view, position, now, highlight, static_marks)
    }

    pub fn hit_target(
        &self,
        window: HWND,
        view: &Viewport,
        position: Vector2,
        now: Duration,
        highlight: Highlight,
        static_marks: bool,
    ) -> Option<DestinationKey> {
        self.targets
            .get(&(window.0 as isize))?
            .routes
            .as_ref()?
            .hit(view, position, now, highlight, static_marks)
    }
    pub fn graphics_mode(&self) -> &'static str {
        self.device
            .as_ref()
            .map_or("Graphics recovering", |device| device.mode)
    }

    #[cfg(test)]
    pub fn new(window: HWND) -> Result<Self> {
        let mut renderer = Self {
            window,
            ..Self::default()
        };
        renderer.attach(window)?;
        Ok(renderer)
    }

    pub fn attach(&mut self, window: HWND) -> Result<()> {
        if self.device.is_none() {
            self.device = Some(device::Device::new()?);
        }
        self.device.as_mut().unwrap().attach(window)?;
        self.targets.entry(window.0 as isize).or_default();
        Ok(())
    }

    pub fn retain_targets(&mut self, mut keep: impl FnMut(HWND) -> bool) -> bool {
        let count = self.targets.len();
        self.targets.retain(|&key, _| keep(HWND(key as _)));
        let removed = count != self.targets.len();
        if removed && let Some(device) = &mut self.device {
            device.retain_targets(|window| self.targets.contains_key(&(window.0 as isize)));
        }
        if self.targets.is_empty() {
            self.outlines = None;
            self.guides = routes::Paths::default();
            self.device = None;
        }
        removed
    }

    #[cfg(test)]
    pub fn paint(
        &mut self,
        prepared: &Prepared,
        width: u32,
        height: u32,
        dpi: u32,
    ) -> Result<Frame> {
        self.paint_target(self.window, prepared, width, height, dpi)
    }

    pub fn paint_target(
        &mut self,
        window: HWND,
        prepared: &Prepared,
        width: u32,
        height: u32,
        dpi: u32,
    ) -> Result<Frame> {
        if width == 0 || height == 0 {
            return Ok(Frame::Idle);
        }
        let established = self.device.is_some();
        let result = self.draw(window, prepared, width, height, dpi);
        if let Err(error) = &result
            && established
            && [
                D2DERR_RECREATE_TARGET,
                DXGI_ERROR_DEVICE_REMOVED,
                DXGI_ERROR_DEVICE_RESET,
                DXGI_ERROR_DEVICE_HUNG,
            ]
            .contains(&error.code())
        {
            let windows: Vec<_> = self.targets.keys().copied().collect();
            // Every target and retained realization depends on this device.
            // Release all old chains before recreating any on these HWNDs.
            self.outlines = None;
            self.guides = routes::Paths::default();
            self.targets.clear();
            self.device = None;
            for window in windows {
                // SAFETY: Invalidating is asynchronous; retired HWNDs are harmless.
                // Idle targets also need a frame on the replacement shared device.
                unsafe {
                    let _ = InvalidateRect(Some(HWND(window as _)), None, false);
                }
            }
            return Ok(Frame::Again);
        }
        result
    }

    fn draw(
        &mut self,
        window: HWND,
        prepared: &Prepared,
        width: u32,
        height: u32,
        dpi: u32,
    ) -> Result<Frame> {
        self.attach(window)?;
        let target = self.targets.get_mut(&(window.0 as isize)).unwrap();
        let device = self.device.as_mut().unwrap();
        #[cfg(test)]
        let capture = device
            .observations
            .as_ref()
            .is_some_and(|observations| observations.capture.borrow().is_some());
        #[cfg(test)]
        if let Some(observations) = &device.observations {
            observations.pulses.borrow_mut().clear();
        }
        device.apply_appearance(prepared.appearance);
        if device.select(window, width, height, dpi)? {
            if let Some(outlines) = &mut target.outlines {
                outlines.invalidate_strokes();
            }
            if let Some(routes) = &mut target.routes {
                routes.invalidate_dpi();
            }
        }
        if let Some(world) = &prepared.world
            && self
                .outlines
                .as_ref()
                .is_none_or(|outlines| !outlines.matches(world, prepared.viewport.meridian))
        {
            self.outlines = Some(outlines::Paths::new(
                world.clone(),
                prepared.viewport.meridian,
            ));
            for target in self.targets.values_mut() {
                target.outlines = None;
            }
        }
        let target = self.targets.get_mut(&(window.0 as isize)).unwrap();
        let mut budget = Budget {
            until: Instant::now() + Duration::from_millis(6),
            pending: false,
        };
        self.guides
            .update(prepared.origin, prepared.viewport.meridian);
        if target.routes.is_none()
            && (!prepared.model.destinations.is_empty() || prepared.origin.is_some())
        {
            target.routes = Some(routes::Routes::new(
                device,
                prepared.model.clone(),
                prepared.origin,
                prepared.viewport.meridian,
            )?);
        }
        if let Some(routes) = &mut target.routes {
            routes.set_appearance(prepared.appearance);
            routes.update(
                prepared.model.clone(),
                prepared.origin,
                prepared.viewport.meridian,
            );
            routes.prepare(
                device,
                &prepared.viewport,
                dpi,
                &prepared.inspection,
                &mut self.guides,
                &mut budget,
            )?;
        }
        if target.speeds.is_none() {
            target.speeds = Some(speeds::Speeds::new(device)?);
        }
        target.speeds.as_mut().unwrap().prepare(prepared)?;
        device.begin(prepared.appearance);
        // SAFETY: Tags identify deferred native failures at EndDraw.
        unsafe {
            device.context.SetTags(1, 0);
        }
        let mut next = None;
        let drawing = (|| -> Result<()> {
            if let Some(paths) = &mut self.outlines {
                let outlines = target.outlines.get_or_insert_with(outlines::Outlines::new);
                next = outlines.draw(device, paths, prepared, &mut budget)?;
            }
            if let (Some(routes), Some(highlight)) = (&mut target.routes, prepared.highlight) {
                let routes_next = routes
                    .draw(
                        device,
                        &prepared.viewport,
                        prepared.now,
                        highlight,
                        &prepared.inspection,
                        &mut budget,
                    )?
                    .next;
                if let Some(delay) = routes_next {
                    next = Some(next.map_or(delay, |old| old.min(delay)));
                }
            }
            target.speeds.as_ref().unwrap().draw(device, prepared);
            Ok(())
        })();
        let ended = device.end();
        #[cfg(test)]
        if drawing.is_ok()
            && ended.is_ok()
            && let Some(observations) = &device.observations
        {
            let frame = super::PresentedFrame {
                target: window,
                revision: prepared.revision,
                at: prepared.now,
                model: std::sync::Arc::as_ptr(&prepared.model) as usize,
            };
            observations.latest.set(Some(frame));
            if capture {
                observations.captured.set(Some(frame));
            }
        }
        drawing.and(ended)?;
        Ok(if budget.pending {
            Frame::Again
        } else {
            next.map_or(Frame::Idle, Frame::After)
        })
    }
}

struct Budget {
    until: Instant,
    pending: bool,
}

fn strength(rate: f64) -> f32 {
    ((1.0 + rate / 1024.0).ln() / (1.0_f64 + 5.0 * 1024.0).ln()).clamp(0.0, 1.0) as f32
}

impl Budget {
    fn allows(&mut self) -> bool {
        if Instant::now() < self.until {
            true
        } else {
            self.pending = true;
            false
        }
    }
}

#[cfg(test)]
mod resource_checks;
#[cfg(test)]
mod tests;
