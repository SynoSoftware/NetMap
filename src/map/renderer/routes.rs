// Ported from TinyTorrent2 MapRenderer.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::{
    Budget,
    device::{Device, Stroke},
    strength,
};
use crate::map::{
    Appearance, Inspection,
    model::{DestinationKey, Model},
    viewport::Viewport,
};
use crate::{connections::Highlight, geography::Point};
use std::{
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::Graphics::Direct2D::{Common::*, *},
    core::{Interface, Result},
};
use windows_numerics::{Matrix3x2, Vector2, Vector3};

pub(super) struct Routes {
    appearance: Appearance,
    model: Arc<Model>,
    origin: Option<Point>,
    meridian: f32,
    routes: Vec<Route>,
    started: Instant,
    glow: Option<Glow>,
    halo_stops: [Option<ID2D1GradientStopCollection1>; 2],
    ink: ID2D1SolidColorBrush,
    dots: Option<Dots>,
    mask: Option<(f32, ID2D1RadialGradientBrush)>,
}

struct Route {
    key: DestinationKey,
    start: f32,
    strength: f32,
    paint: Option<Paint>,
}

struct Paint {
    guide: Rc<Guide>,
    line: Option<(f32, Stroke)>,
    halo_line: Option<(f32, Stroke)>,
    core: Option<(D2D1_COLOR_F, Vector2, ID2D1RadialGradientBrush)>,
    halo: Option<(D2D1_COLOR_F, ID2D1RadialGradientBrush)>,
}

struct Guide {
    path: ID2D1PathGeometry1,
    bounds: D2D_RECT_F,
    points: Vec<(f32, Vector2)>,
    end: Vector2,
    start: Vector2,
}

#[derive(Default)]
pub(super) struct Paths {
    origin: Option<Point>,
    meridian: f32,
    guides: HashMap<DestinationKey, Weak<Guide>>,
}

impl Paths {
    pub fn update(&mut self, origin: Option<Point>, meridian: f32) {
        if self.origin != origin || self.meridian != meridian {
            self.guides.clear();
            self.origin = origin;
            self.meridian = meridian;
        }
        self.guides.retain(|_, guide| guide.strong_count() > 0);
    }

    fn guide(
        &mut self,
        device: &Device,
        view: &Viewport,
        key: &DestinationKey,
        origin: Point,
        end: Point,
    ) -> Result<Rc<Guide>> {
        if let Some(guide) = self.guides.get(key).and_then(Weak::upgrade) {
            return Ok(guide);
        }
        let guide = Rc::new(Guide::new(device, view, origin, end)?);
        self.guides.insert(key.clone(), Rc::downgrade(&guide));
        Ok(guide)
    }
}

struct Glow {
    view: Viewport,
    strengths: Vec<f32>,
    image: ID2D1Image,
}

struct Dots {
    dpi: u32,
    idle: ID2D1Bitmap1,
    active: ID2D1Bitmap1,
    batch: ID2D1SpriteBatch,
    ring: ID2D1GeometryRealization,
    wide_ring: ID2D1GeometryRealization,
}

pub(super) struct Timing {
    pub next: Option<Duration>,
}

impl Routes {
    #[cfg(test)]
    pub(super) fn native_rendering(&self, value: &mut super::NativeRendering) {
        for route in &self.routes {
            if let Some(paint) = &route.paint {
                value
                    .guides
                    .push((route.key.clone(), Rc::as_ptr(&paint.guide) as usize));
                for (_, stroke) in paint.line.iter().chain(paint.halo_line.iter()) {
                    value.route_strokes.push(stroke.geometry.as_raw() as usize);
                }
            }
        }
        value.glow = self
            .glow
            .as_ref()
            .map_or(0, |glow| glow.image.as_raw() as usize);
        value.dots = self
            .dots
            .as_ref()
            .map_or(0, |dots| dots.batch.as_raw() as usize);
    }

    pub fn new(
        device: &Device,
        model: Arc<Model>,
        origin: Option<Point>,
        meridian: f32,
    ) -> Result<Self> {
        // SAFETY: Resources are created and retained on the render thread.
        let ink = unsafe {
            device
                .context
                .CreateSolidColorBrush(&Appearance::default().colors().ink, None)?
        };
        let mut value = Self {
            appearance: Appearance::default(),
            model: Arc::new(Model::default()),
            origin,
            meridian,
            routes: Vec::new(),
            started: Instant::now(),
            glow: None,
            halo_stops: [None, None],
            ink,
            dots: None,
            mask: None,
        };
        value.update(model, origin, meridian);
        Ok(value)
    }

    pub fn update(&mut self, model: Arc<Model>, origin: Option<Point>, meridian: f32) {
        if Arc::ptr_eq(&self.model, &model) && self.origin == origin && self.meridian == meridian {
            return;
        }
        let progress = self.progress();
        let retain = self.origin == origin && self.meridian == meridian;
        let mut previous: HashMap<_, _> = self
            .routes
            .drain(..)
            .map(|route| (route.key.clone(), route))
            .collect();
        self.routes = model
            .destinations
            .iter()
            .map(|destination| {
                let strength =
                    strength(model.groups[destination.group].rates.map_or(0.0, |rates| {
                        (rates.received + rates.sent) * destination.share
                    }));
                match previous.remove(&destination.key) {
                    Some(mut route) => {
                        route.start += (route.strength - route.start) * progress;
                        route.strength = strength;
                        if !retain {
                            route.paint = None;
                        }
                        route
                    }
                    None => Route {
                        key: destination.key.clone(),
                        start: strength,
                        strength,
                        paint: None,
                    },
                }
            })
            .collect();
        self.model = model;
        self.origin = origin;
        self.meridian = meridian;
        self.started = Instant::now();
        self.glow = None;
    }

    fn progress(&self) -> f32 {
        if self.appearance.static_marks() {
            1.0
        } else {
            (self.started.elapsed().as_secs_f32() / 1.6).min(1.0)
        }
    }

    pub fn set_appearance(&mut self, appearance: Appearance) {
        if self.appearance == appearance {
            return;
        }
        self.appearance = appearance;
        self.glow = None;
        self.mask = None;
        self.halo_stops = [None, None];
        for route in &mut self.routes {
            if let Some(paint) = &mut route.paint {
                paint.halo = None;
            }
        }
        // SAFETY: This brush belongs to the current render thread/device.
        unsafe {
            self.ink.SetColor(&appearance.colors().ink);
        }
    }

    pub fn invalidate_dpi(&mut self) {
        self.dots = None;
        self.glow = None;
        for route in &mut self.routes {
            if let Some(paint) = &mut route.paint {
                paint.line = None;
                paint.halo_line = None;
            }
        }
    }

    /// Runs before BeginDraw, so retained command lists and dot targets have
    /// their own paired drawing sessions instead of nesting a native session.
    pub fn prepare(
        &mut self,
        device: &Device,
        view: &Viewport,
        dpi: u32,
        inspection: &Inspection,
        paths: &mut Paths,
        budget: &mut Budget,
    ) -> Result<()> {
        if view.view_scale() <= 0.0 {
            return Ok(());
        }
        if self.model.destinations.is_empty() {
            return Ok(());
        }
        if let Some(origin) = self.origin {
            // Selection feedback gets the shared allowance before ordinary
            // route and outline rebuilds, including its sharp stroke/brush.
            for selected in [true, false] {
                for (route, destination) in self.routes.iter_mut().zip(&self.model.destinations) {
                    let group = &self.model.groups[destination.group];
                    let marked = inspection.pointed.as_ref() == Some(&route.key)
                        || inspection.selected.as_ref() == Some(&group.place);
                    if marked != selected {
                        continue;
                    }
                    if route.paint.is_none() {
                        if !budget.allows() {
                            break;
                        }
                        route.paint = Some(Paint::new(paths.guide(
                            device,
                            view,
                            &route.key,
                            origin,
                            destination.point,
                        )?));
                        self.glow = None;
                    }
                    if selected {
                        let paint = route.paint.as_mut().unwrap();
                        if (-1..=1).any(|turn| paint.visible(view, turn)) {
                            Paint::realize(
                                device,
                                &paint.guide.path,
                                &mut paint.line,
                                2.0,
                                view.axis_scale(),
                                budget,
                            )?;
                            if self.appearance.contrast.is_none() {
                                paint.core(
                                    device,
                                    view.axis_scale(),
                                    self.appearance.colors().ink,
                                    budget,
                                )?;
                            }
                        }
                    }
                }
            }
        }
        if self.dots.as_ref().is_none_or(|dots| dots.dpi != dpi) {
            self.dots = Some(Dots::new(device, dpi)?);
        }
        if self.origin.is_none() {
            self.glow = None;
            return Ok(());
        }
        if self.appearance.contrast.is_some() {
            self.glow = None;
            return Ok(());
        }
        let progress = self.progress();
        let strengths: Vec<_> = self
            .routes
            .iter()
            .map(|route| route.start + (route.strength - route.start) * progress)
            .collect();
        for (route, &strength) in self.routes.iter_mut().zip(&strengths) {
            let Some(paint) = &mut route.paint else {
                continue;
            };
            if strength > 0.0
                && (-1..=1).any(|turn| paint.visible(view, turn))
                && Paint::realize(
                    device,
                    &paint.guide.path,
                    &mut paint.halo_line,
                    step(2.0 + 3.0 * strength, 2.0),
                    view.axis_scale(),
                    budget,
                )?
            {
                // Deferred outline/core work does not change this image. Keep
                // it until one of its own retained geometries is replaced.
                self.glow = None;
            }
        }
        if self
            .glow
            .as_ref()
            .is_some_and(|glow| glow.view == *view && glow.strengths == strengths)
        {
            return Ok(());
        }
        // SAFETY: The render thread owns the context and retained resources.
        // This session ends and restores its target before the main frame starts.
        unsafe {
            let target = device.context.GetTarget()?;
            let commands = device.context.CreateCommandList()?;
            device.context.SetTarget(&commands);
            device.context.BeginDraw();
            let recorded = (|| -> Result<()> {
                for turn in -1..=1 {
                    for ((route, destination), &strength) in self
                        .routes
                        .iter_mut()
                        .zip(&self.model.destinations)
                        .zip(&strengths)
                    {
                        let Some(paint) = &mut route.paint else {
                            continue;
                        };
                        if strength <= 0.0 || !paint.visible(view, turn) {
                            continue;
                        }
                        let group = &self.model.groups[destination.group];
                        let uploading =
                            group.rates.is_some_and(|rates| rates.sent > rates.received);
                        let color = if uploading {
                            self.appearance.colors().upload
                        } else {
                            self.appearance.colors().download
                        };
                        let width = step(2.0 + 3.0 * strength, 2.0);
                        if paint.halo.as_ref().is_none_or(|(old, _)| *old != color) {
                            let stops = &mut self.halo_stops[usize::from(uploading)];
                            if stops.is_none() {
                                *stops = Some(gradient_stops(
                                    device,
                                    &[(0.0, alpha(color, 0.3)), (1.0, color)],
                                )?);
                            }
                            paint.halo = Some((
                                color,
                                radial_with_stops(
                                    device,
                                    paint.guide.end,
                                    Vector2::one(),
                                    stops.as_ref().unwrap(),
                                )?,
                            ));
                        }
                        let brush = &paint.halo.as_ref().unwrap().1;
                        brush.SetRadiusX(30.0 / view.axis_scale().X);
                        brush.SetRadiusY(30.0 / view.axis_scale().Y);
                        brush.SetOpacity(0.5 * 0.35 * strength * strength);
                        paint.draw(device, view, turn, paint.halo_line.as_ref(), width, brush);
                    }
                }
                Ok(())
            })();
            let ended = device.context.EndDraw(None, None);
            device.context.SetTarget(&target);
            recorded.and(ended)?;
            commands.Close()?;
            let crop = device.context.CreateEffect(&CLSID_D2D1Crop)?;
            crop.SetInput(0, &commands, true);
            crop.SetValue(
                D2D1_CROP_PROP_RECT.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR4,
                &floats(&[-6.0, -6.0, view.size.X + 6.0, view.size.Y + 6.0]),
            )?;
            let blur = device.context.CreateEffect(&CLSID_D2D1GaussianBlur)?;
            blur.SetInput(0, &crop.GetOutput()?, true);
            blur.SetValue(
                D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &1.5_f32.to_ne_bytes(),
            )?;
            blur.SetValue(
                D2D1_PROPERTY_CACHED.0 as u32,
                D2D1_PROPERTY_TYPE_BOOL,
                &1_u32.to_ne_bytes(),
            )?;
            self.glow = Some(Glow {
                view: *view,
                strengths,
                image: blur.GetOutput()?,
            });
        }
        Ok(())
    }

    fn bounds(&self, device: &Device, view: &Viewport) -> Result<D2D_RECT_F> {
        let tolerance = device.tolerance(1.0);
        let pixel = 4.0 * tolerance;
        let mut bounds = D2D_RECT_F {
            left: f32::MAX,
            top: f32::MAX,
            right: f32::MIN,
            bottom: f32::MIN,
        };
        let mut include = |rect: D2D_RECT_F| {
            if rect.left < rect.right && rect.top < rect.bottom {
                bounds.left = bounds.left.min(rect.left);
                bounds.top = bounds.top.min(rect.top);
                bounds.right = bounds.right.max(rect.right);
                bounds.bottom = bounds.bottom.max(rect.bottom);
            }
        };
        for route in &self.routes {
            let Some(paint) = &route.paint else {
                continue;
            };
            // A pending rebuild can still draw an older, wider realization.
            // The new/fallback two-DIP core and moving pulse must also fit.
            let mut margin = Vector2::one() * 1.8_f32.max(1.0 + tolerance);
            if let Some((width, stroke)) = &paint.line {
                let retained = view.axis_scale() / stroke.scale * (width / 2.0 + tolerance);
                margin.X = margin.X.max(retained.X);
                margin.Y = margin.Y.max(retained.Y);
            }
            for turn in -1..=1 {
                if !paint.visible(view, turn) {
                    continue;
                }
                let guide = paint.guide.bounds;
                let shift = 360.0 * turn as f32;
                let low = view.to_screen(Vector2::new(guide.left + shift, guide.top)) - margin;
                let high = view.to_screen(Vector2::new(guide.right + shift, guide.bottom)) + margin;
                include(D2D_RECT_F {
                    left: low.X,
                    top: low.Y,
                    right: high.X,
                    bottom: high.Y,
                });
            }
        }
        if let Some(glow) = &self.glow {
            // SAFETY: The closed effect graph belongs to this context. Native
            // image bounds include its crop/blur support at the selected DPI.
            include(unsafe { device.context.GetImageLocalBounds(&glow.image)? });
        }
        // Cover rasterization at the edge, round outward to physical pixels,
        // then retain the source's single-longitude/viewport clip.
        bounds.left = ((bounds.left - pixel) / pixel).floor() * pixel;
        bounds.top = ((bounds.top - pixel) / pixel).floor() * pixel;
        bounds.right = ((bounds.right + pixel) / pixel).ceil() * pixel;
        bounds.bottom = ((bounds.bottom + pixel) / pixel).ceil() * pixel;
        bounds.left = bounds
            .left
            .max((view.offset.X - 180.0 * view.axis_scale().X).max(0.0));
        bounds.top = bounds.top.max(0.0);
        bounds.right = bounds
            .right
            .min((view.offset.X + 180.0 * view.axis_scale().X).min(view.size.X));
        bounds.bottom = bounds.bottom.min(view.size.Y);
        Ok(
            if bounds.left < bounds.right && bounds.top < bounds.bottom {
                bounds
            } else {
                D2D_RECT_F::default()
            },
        )
    }

    pub fn draw(
        &mut self,
        device: &Device,
        view: &Viewport,
        now: Duration,
        highlight: Highlight,
        inspection: &Inspection,
        budget: &mut Budget,
    ) -> Result<Timing> {
        if view.view_scale() <= 0.0 {
            return Ok(Timing { next: None });
        }
        let progress = self.progress();
        let mut next = None;
        let mut frame = false;
        let static_marks = self.appearance.static_marks();
        let contrast = self.appearance.contrast.is_some();
        let palette = self.appearance.colors();
        let ink_color = palette.ink;
        let mask_color = D2D1_COLOR_F {
            a: 1.0,
            ..ink_color
        };
        let mut positions = Vec::new();
        let mut colors = Vec::new();
        let mut active_positions = Vec::new();
        let mut active_colors = Vec::new();
        let mut rings = Vec::new();
        // SAFETY: Main BeginDraw is live. Every layer and transform is restored
        // before returning, including when native resource creation fails.
        unsafe {
            device.context.SetTransform(&Matrix3x2::identity());
            device.context.SetTags(2, 0);
            let arcs = self
                .routes
                .iter()
                .filter(|route| route.paint.is_some())
                .count();
            if let Some(origin) = self.origin.filter(|_| arcs > 0) {
                let target = (0.8 * (5.0 / arcs.max(1) as f32).sqrt()).clamp(0.1, 0.8);
                let amount = ((arcs as f32 - 8.0) / 12.0).clamp(0.0, 1.0);
                let opacity = 1.0 - (1.0 - target) * amount * amount * (3.0 - 2.0 * amount);
                if self.mask.as_ref().is_none_or(|(old, _)| *old != opacity) {
                    self.mask = Some((
                        opacity,
                        radial(
                            device,
                            Vector2::zero(),
                            Vector2::new(40.0, 40.0),
                            &[
                                (0.0, alpha(mask_color, opacity)),
                                (0.25, alpha(mask_color, opacity + (1.0 - opacity) * 0.15625)),
                                (0.5, alpha(mask_color, (1.0 + opacity) / 2.0)),
                                (0.75, alpha(mask_color, opacity + (1.0 - opacity) * 0.84375)),
                                (1.0, mask_color),
                            ],
                        )?,
                    ));
                }
                let mask = &self.mask.as_ref().unwrap().1;
                mask.SetCenter(view.screen(origin));
                let mut layer = D2D1_LAYER_PARAMETERS1 {
                    contentBounds: self.bounds(device, view)?,
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: Matrix3x2::identity(),
                    opacity: if contrast { 1.0 } else { 0.8 },
                    opacityBrush: std::mem::ManuallyDrop::new(if contrast {
                        None
                    } else {
                        Some(mask.cast()?)
                    }),
                    ..Default::default()
                };
                device.context.PushLayer(&layer, None::<&ID2D1Layer>);
                std::mem::ManuallyDrop::drop(&mut layer.opacityBrush);
                let guides = (|| -> Result<()> {
                    if let Some(glow) = &self.glow {
                        device.context.SetTags(2, 1);
                        device.context.DrawImage(
                            &glow.image,
                            None,
                            None,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            D2D1_COMPOSITE_MODE_SOURCE_OVER,
                        );
                    }
                    for turn in -1..=1 {
                        device.context.SetTags(2, 2);
                        for (route, destination) in
                            self.routes.iter_mut().zip(&self.model.destinations)
                        {
                            let group = &self.model.groups[destination.group];
                            let Some(paint) = &mut route.paint else {
                                continue;
                            };
                            if !paint.visible(view, turn) {
                                continue;
                            }
                            let strength = route.start + (route.strength - route.start) * progress;
                            let establishment =
                                highlight.opacity(group.introduced, now, static_marks);
                            let active = group
                                .rates
                                .is_some_and(|rates| rates.received + rates.sent > 0.0);
                            let visible = establishment.max(if active { 1.0 } else { 0.0 });
                            if visible == 0.0 {
                                continue;
                            }
                            if let Some(deadline) =
                                highlight.deadline(group.introduced, now, static_marks)
                            {
                                let delay = deadline.saturating_sub(now);
                                next = Some(next.map_or(delay, |old: Duration| old.min(delay)));
                            }
                            frame |= !static_marks
                                && now >= group.introduced.saturating_add(highlight.bright)
                                && now < group.introduced.saturating_add(highlight.duration());
                            frame |= progress < 1.0 && route.start != route.strength;
                            let marked = inspection.pointed.as_ref() == Some(&route.key)
                                || inspection.selected.as_ref() == Some(&group.place);
                            let width = if marked {
                                2.0
                            } else {
                                step(1.0 + strength, 4.0)
                            };
                            Paint::realize(
                                device,
                                &paint.guide.path,
                                &mut paint.line,
                                width,
                                view.axis_scale(),
                                budget,
                            )?;
                            let color = if marked {
                                ink_color
                            } else if group.rates.is_some_and(|rates| rates.sent > rates.received) {
                                palette.upload
                            } else {
                                palette.download
                            };
                            if contrast {
                                paint.draw(
                                    device,
                                    view,
                                    turn,
                                    paint.line.as_ref(),
                                    width,
                                    &self.ink,
                                );
                            } else {
                                paint.core(device, view.axis_scale(), color, budget)?;
                                let brush = &paint.core.as_ref().unwrap().2;
                                brush.SetOpacity((0.18 + 0.82 * strength) * visible);
                                paint.draw(device, view, turn, paint.line.as_ref(), width, brush);
                            }
                            if let Some(rates) = group.rates.filter(|_| !static_marks) {
                                for (present, sending, color) in [
                                    (rates.sent > 0.0, true, palette.upload),
                                    (rates.received > 0.0, false, palette.download),
                                ] {
                                    if !present {
                                        continue;
                                    }
                                    frame = true;
                                    let along = (now.as_secs_f64() * 0.5).fract() as f32;
                                    let mut point =
                                        paint.sample(if sending { along } else { 1.0 - along });
                                    point.X += 360.0 * turn as f32;
                                    point = view.to_screen(point);
                                    device.context.SetTransform(&Matrix3x2::identity());
                                    self.ink.SetColor(&color);
                                    device.context.FillEllipse(
                                        &D2D1_ELLIPSE {
                                            point,
                                            radiusX: 1.8,
                                            radiusY: 1.8,
                                        },
                                        &self.ink,
                                    );
                                    #[cfg(test)]
                                    if let Some(observations) = &device.observations {
                                        observations.pulses.borrow_mut().push(
                                            crate::map::PresentedPulse {
                                                key: route.key.clone(),
                                                sending,
                                                position: point,
                                            },
                                        );
                                    }
                                    self.ink.SetColor(&ink_color);
                                }
                            }
                        }
                    }
                    Ok(())
                })();
                device.context.SetTransform(&Matrix3x2::identity());
                device.context.PopLayer();
                guides?;
            }
            if let Some(origin) = self.origin {
                device.context.SetTags(3, 0);
                device.context.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: view.screen(origin),
                        radiusX: 3.0,
                        radiusY: 3.0,
                    },
                    &self.ink,
                );
            }
            for (route, destination) in self.routes.iter().zip(&self.model.destinations) {
                let group = &self.model.groups[destination.group];
                let point = view.screen(destination.point);
                if point.X < -16.0
                    || point.Y < -16.0
                    || point.X > view.size.X + 16.0
                    || point.Y > view.size.Y + 16.0
                {
                    continue;
                }
                let opacity = highlight.opacity(group.introduced, now, static_marks);
                let active = group
                    .rates
                    .is_some_and(|rates| rates.received + rates.sent > 0.0);
                let opacity = opacity.max(if active { 1.0 } else { 0.0 });
                if opacity == 0.0 {
                    continue;
                }
                if let Some(deadline) = highlight.deadline(group.introduced, now, static_marks) {
                    let after = deadline.saturating_sub(now);
                    next = Some(next.map_or(after, |old: Duration| old.min(after)));
                }
                frame |= !static_marks
                    && now >= group.introduced.saturating_add(highlight.bright)
                    && now < group.introduced.saturating_add(highlight.duration());
                frame |= progress < 1.0 && route.start != route.strength;
                let radius = if active { 2.4 } else { 2.0 };
                let kinds = [
                    (group.receiving, palette.download),
                    (group.sending, palette.upload),
                    (group.unknown, ink_color),
                ];
                let count = kinds.iter().filter(|(count, _)| *count != 0).count();
                let mut offset = -2.5 * count.saturating_sub(1) as f32;
                for (count, color) in kinds {
                    if count == 0 {
                        continue;
                    }
                    let rect = D2D_RECT_F {
                        left: point.X + offset - radius - 1.0,
                        top: point.Y - radius - 1.0,
                        right: point.X + offset + radius + 1.0,
                        bottom: point.Y + radius + 1.0,
                    };
                    let tint = alpha(if contrast { ink_color } else { color }, opacity);
                    if active {
                        active_positions.push(rect);
                        active_colors.push(tint);
                    } else {
                        positions.push(rect);
                        colors.push(tint);
                    }
                    offset += 5.0;
                }
                if inspection.pointed.as_ref() == Some(&route.key)
                    || inspection.selected.as_ref() == Some(&group.place)
                {
                    rings.push((point, count > 1));
                }
            }
            if let Some(dots) = &self.dots {
                device.context.SetTags(4, 0);
                dots.draw(device, &dots.idle, &positions, &colors)?;
                dots.draw(device, &dots.active, &active_positions, &active_colors)?;
            }
            for (point, wide) in rings {
                if let Some(dots) = &self.dots {
                    device
                        .context
                        .SetTransform(&Matrix3x2::translation(point.X, point.Y));
                    device.context.DrawGeometryRealization(
                        if wide { &dots.wide_ring } else { &dots.ring },
                        &self.ink,
                    );
                }
            }
            device.context.SetTransform(&Matrix3x2::identity());
        }
        if frame {
            next = Some(Duration::from_millis(33));
        }
        Ok(Timing { next })
    }

    pub fn hit(
        &self,
        view: &Viewport,
        point: Vector2,
        now: Duration,
        highlight: Highlight,
        static_marks: bool,
    ) -> Option<DestinationKey> {
        if view.view_scale() <= 0.0 {
            return None;
        }
        let visible = |index: usize| {
            let group = &self.model.groups[self.model.destinations[index].group];
            highlight.opacity(group.introduced, now, static_marks) > 0.0
                || group
                    .rates
                    .is_some_and(|rates| rates.received + rates.sent > 0.0)
        };
        for (index, destination) in self.model.destinations.iter().enumerate() {
            if visible(index) && (view.screen(destination.point) - point).length_squared() <= 144.0
            {
                return Some(destination.key.clone());
            }
        }
        let longitude = view.to_map(point).X;
        if longitude < view.center.X - 180.0 || longitude > view.center.X + 180.0 {
            return None;
        }
        for (index, route) in self.routes.iter().enumerate() {
            if !visible(index) {
                continue;
            }
            let Some(paint) = &route.paint else {
                continue;
            };
            for turn in -1..=1 {
                for segment in paint.guide.points.windows(2) {
                    if (segment[1].1.X - segment[0].1.X).abs() > 180.0 {
                        continue;
                    }
                    let a = view.to_screen(segment[0].1 + Vector2::new(turn as f32 * 360.0, 0.0));
                    let b = view.to_screen(segment[1].1 + Vector2::new(turn as f32 * 360.0, 0.0));
                    let span = b - a;
                    let amount = if span.length_squared() == 0.0 {
                        0.0
                    } else {
                        ((point - a).dot(&span) / span.length_squared()).clamp(0.0, 1.0)
                    };
                    if (point - (a + span * amount)).length_squared() <= 20.25 {
                        return Some(route.key.clone());
                    }
                }
            }
        }
        None
    }
}

impl Guide {
    fn new(device: &Device, view: &Viewport, origin: Point, end: Point) -> Result<Self> {
        let from = sphere(origin);
        let to = sphere(end);
        let angle = f64::from(from.dot(&to).clamp(-1.0, 1.0)).acos();
        let mut tangent = to - from * angle.cos() as f32;
        if tangent.length_squared() < 0.00000001 {
            let other = if from.Z.abs() < 0.9 {
                Vector3::unit_z()
            } else {
                Vector3::unit_y()
            };
            tangent = Vector3::new(
                from.Y * other.Z - from.Z * other.Y,
                from.Z * other.X - from.X * other.Z,
                from.X * other.Y - from.Y * other.X,
            );
        }
        tangent = tangent / tangent.length_squared().sqrt();
        let count = 8_usize.max((angle * 48.0).ceil() as usize);
        let start = view.coordinates(origin);
        let mut previous = start;
        let mut figures = vec![vec![start]];
        let mut points = vec![(0.0, start)];
        for index in 1..=count {
            let travelled = angle * index as f64 / count as f64;
            let direction = from * travelled.cos() as f32 + tangent * travelled.sin() as f32;
            let point = view.coordinates(Point {
                longitude: f64::from(direction.Y)
                    .atan2(f64::from(direction.X))
                    .to_degrees(),
                latitude: f64::from(direction.Z)
                    .atan2(f64::from(direction.X * direction.X + direction.Y * direction.Y).sqrt())
                    .to_degrees(),
            });
            if (point.X - previous.X).abs() > 180.0 {
                let unwrapped = point.X + if point.X > previous.X { -360.0 } else { 360.0 };
                let edge = if previous.X > 0.0 { 180.0 } else { -180.0 };
                let amount = if unwrapped == previous.X {
                    0.0
                } else {
                    (edge - previous.X) / (unwrapped - previous.X)
                };
                let crossing = Vector2::new(edge, previous.Y + (point.Y - previous.Y) * amount);
                figures.last_mut().unwrap().push(crossing);
                let progress = (index as f32 - 1.0 + amount) / count as f32;
                points.push((progress, crossing));
                let opposite = Vector2::new(-crossing.X, crossing.Y);
                figures.push(vec![opposite]);
                points.push((progress, opposite));
            }
            figures.last_mut().unwrap().push(point);
            points.push((index as f32 / count as f32, point));
            previous = point;
        }
        let path = device.path(figures.iter().map(Vec::as_slice), false)?;
        // SAFETY: Completed immutable geometry belongs to this render thread.
        let bounds = unsafe { path.GetBounds(None)? };
        Ok(Self {
            path,
            bounds,
            points,
            start,
            end: view.coordinates(end),
        })
    }
}

impl Paint {
    fn new(guide: Rc<Guide>) -> Self {
        Self {
            guide,
            line: None,
            halo_line: None,
            core: None,
            halo: None,
        }
    }

    fn sample(&self, along: f32) -> Vector2 {
        let index = self
            .guide
            .points
            .partition_point(|(at, _)| *at <= along)
            .max(1)
            .min(self.guide.points.len() - 1);
        let (from, a) = self.guide.points[index - 1];
        let (to, b) = self.guide.points[index];
        if to <= from {
            b
        } else {
            a + (b - a) * ((along - from) / (to - from))
        }
    }

    fn visible(&self, view: &Viewport, turn: i32) -> bool {
        let low = view.to_map(Vector2::zero());
        let high = view.to_map(view.size);
        let margin = Vector2::new(30.0, 30.0) / view.axis_scale();
        let shift = 360.0 * turn as f32;
        self.guide.bounds.right + shift + margin.X > low.X.max(view.center.X - 180.0)
            && self.guide.bounds.left + shift - margin.X < high.X.min(view.center.X + 180.0)
            && self.guide.bounds.bottom + margin.Y > low.Y
            && self.guide.bounds.top - margin.Y < high.Y
    }

    fn realize(
        device: &Device,
        path: &ID2D1PathGeometry1,
        slot: &mut Option<(f32, Stroke)>,
        width: f32,
        scale: Vector2,
        budget: &mut Budget,
    ) -> Result<bool> {
        if slot
            .as_ref()
            .is_some_and(|(made, stroke)| *made == width && stroke.current(scale, 0.05))
            || !budget.allows()
        {
            return Ok(false);
        }
        *slot = Some((width, device.stroke(path, scale, width)?));
        Ok(true)
    }

    fn core(
        &mut self,
        device: &Device,
        scale: Vector2,
        color: D2D1_COLOR_F,
        budget: &mut Budget,
    ) -> Result<()> {
        if self.core.as_ref().is_some_and(|(old, made, _)| {
            let change = scale / *made - Vector2::one();
            *old == color && change.X.abs() <= 0.05 && change.Y.abs() <= 0.05
        }) {
            return Ok(());
        }
        if self.core.is_some() && !budget.allows() {
            return Ok(());
        }
        let length = ((self.guide.end - self.guide.start) * scale)
            .length_squared()
            .sqrt()
            .max(1.0);
        let landed = 30.0 / length;
        let mut positions = vec![
            0.0,
            landed / 4.0,
            landed / 2.0,
            landed * 0.75,
            landed,
            0.2,
            0.65,
            1.0,
        ];
        positions.retain(|position| *position <= 1.0);
        positions.sort_by(f32::total_cmp);
        let stops: Vec<_> = positions
            .into_iter()
            .map(|position| {
                let along = 1.0 - position;
                let route = if along < 0.35 {
                    0.55 + 0.45 * along / 0.35
                } else if along < 0.8 {
                    1.0 - 0.1 * (along - 0.35) / 0.45
                } else {
                    0.9 - 0.25 * (along - 0.8) / 0.2
                };
                (
                    position,
                    alpha(
                        color,
                        route * (0.3 + 0.7 * (position * length / 30.0).min(1.0)),
                    ),
                )
            })
            .collect();
        self.core = Some((
            color,
            scale,
            radial(
                device,
                self.guide.end,
                Vector2::new(length / scale.X, length / scale.Y),
                &stops,
            )?,
        ));
        Ok(())
    }

    fn draw(
        &self,
        device: &Device,
        view: &Viewport,
        turn: i32,
        stroke: Option<&(f32, Stroke)>,
        width: f32,
        brush: &ID2D1Brush,
    ) {
        let transform = Matrix3x2::translation(turn as f32 * 360.0, 0.0) * view.transform();
        // SAFETY: All COM inputs are retained on this render thread. Brush and
        // context transforms pair the cached screen-space stroke with map space.
        unsafe {
            if let Some((_, stroke)) = stroke {
                brush.SetTransform(&Matrix3x2::scale(stroke.scale.X, stroke.scale.Y));
                device.context.SetTransform(
                    &(Matrix3x2::scale(1.0 / stroke.scale.X, 1.0 / stroke.scale.Y) * transform),
                );
                device
                    .context
                    .DrawGeometryRealization(&stroke.geometry, brush);
            } else {
                brush.SetTransform(&Matrix3x2::identity());
                device.context.SetTransform(&transform);
                device
                    .context
                    .DrawGeometry(&self.guide.path, brush, width, &device.fixed_stroke);
            }
        }
    }
}

fn radial(
    device: &Device,
    center: Vector2,
    radius: Vector2,
    stops: &[(f32, D2D1_COLOR_F)],
) -> Result<ID2D1RadialGradientBrush> {
    radial_with_stops(device, center, radius, &gradient_stops(device, stops)?)
}

fn gradient_stops(
    device: &Device,
    stops: &[(f32, D2D1_COLOR_F)],
) -> Result<ID2D1GradientStopCollection1> {
    let stops: Vec<_> = stops
        .iter()
        .map(|&(position, color)| D2D1_GRADIENT_STOP { position, color })
        .collect();
    // SAFETY: Native creation copies the bounded stop slice. The immutable
    // collection stays on its owning render thread/device.
    unsafe {
        device.context.CreateGradientStopCollection(
            &stops,
            D2D1_COLOR_SPACE_SRGB,
            D2D1_COLOR_SPACE_SRGB,
            D2D1_BUFFER_PRECISION_8BPC_UNORM,
            D2D1_EXTEND_MODE_CLAMP,
            D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
        )
    }
}

fn radial_with_stops(
    device: &Device,
    center: Vector2,
    radius: Vector2,
    stops: &ID2D1GradientStopCollection1,
) -> Result<ID2D1RadialGradientBrush> {
    // SAFETY: The immutable stop collection and each mutable brush stay on the
    // same render thread/device; the brush retains the collection it uses.
    unsafe {
        device.context.CreateRadialGradientBrush(
            &D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                center,
                gradientOriginOffset: Vector2::zero(),
                radiusX: radius.X,
                radiusY: radius.Y,
            },
            None,
            stops,
        )
    }
}

impl Dots {
    fn new(device: &Device, dpi: u32) -> Result<Self> {
        // SAFETY: Dot drawing ends and restores the window target before the
        // frame starts; bitmaps remain immutable sprite sources afterward.
        unsafe {
            let target = device.context.GetTarget()?;
            let make = |radius: f32| -> Result<ID2D1Bitmap1> {
                let size = ((radius * 2.0 + 2.0) * dpi as f32 / 96.0).ceil() as u32;
                let bitmap = device.context.CreateBitmap(
                    D2D_SIZE_U {
                        width: size,
                        height: size,
                    },
                    None,
                    0,
                    &D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format:
                                windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                        },
                        dpiX: dpi as f32,
                        dpiY: dpi as f32,
                        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                        ..Default::default()
                    },
                )?;
                let brush = device.context.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    None,
                )?;
                device.context.SetTarget(&bitmap);
                device.context.SetTransform(&Matrix3x2::identity());
                device.context.BeginDraw();
                device.context.Clear(Some(&D2D1_COLOR_F::default()));
                device.context.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(radius + 1.0, radius + 1.0),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                );
                device.context.EndDraw(None, None)?;
                Ok(bitmap)
            };
            let result = (|| {
                Ok(Self {
                    dpi,
                    idle: make(2.0)?,
                    active: make(2.4)?,
                    batch: device.context.CreateSpriteBatch()?,
                    ring: device.context.CreateStrokedGeometryRealization(
                        &device.factory.CreateEllipseGeometry(&D2D1_ELLIPSE {
                            point: Vector2::zero(),
                            radiusX: 5.0,
                            radiusY: 5.0,
                        })?,
                        0.25 * 96.0 / dpi as f32,
                        1.0,
                        None::<&ID2D1StrokeStyle>,
                    )?,
                    wide_ring: device.context.CreateStrokedGeometryRealization(
                        &device.factory.CreateEllipseGeometry(&D2D1_ELLIPSE {
                            point: Vector2::zero(),
                            radiusX: 8.0,
                            radiusY: 8.0,
                        })?,
                        0.25 * 96.0 / dpi as f32,
                        1.0,
                        None::<&ID2D1StrokeStyle>,
                    )?,
                })
            })();
            device.context.SetTarget(&target);
            result
        }
    }

    fn draw(
        &self,
        device: &Device,
        bitmap: &ID2D1Bitmap1,
        rectangles: &[D2D_RECT_F],
        colors: &[D2D1_COLOR_F],
    ) -> Result<()> {
        if rectangles.is_empty() {
            return Ok(());
        }
        // SAFETY: Matching slices stay live through AddSprites' native copy;
        // the retained batch is reused and drawing references a live dot bitmap.
        unsafe {
            self.batch.Clear();
            self.batch.AddSprites(
                rectangles.len() as u32,
                rectangles.as_ptr(),
                None,
                Some(colors.as_ptr()),
                None,
                size_of::<D2D_RECT_F>() as u32,
                0,
                size_of::<D2D1_COLOR_F>() as u32,
                0,
            )?;
            device.context.SetTransform(&Matrix3x2::identity());
            // The bitmap already contains antialiased edges. Native sprite
            // drawing requires aliased primitive mode; restore it for geometry.
            let antialias = device.context.GetAntialiasMode();
            device.context.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            device.context.DrawSpriteBatch(
                &self.batch,
                0,
                rectangles.len() as u32,
                bitmap,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                D2D1_SPRITE_OPTIONS_NONE,
            );
            device.context.SetAntialiasMode(antialias);
        }
        Ok(())
    }
}

fn step(value: f32, steps: f32) -> f32 {
    (value * steps).round_ties_even() / steps
}
fn alpha(mut color: D2D1_COLOR_F, opacity: f32) -> D2D1_COLOR_F {
    color.a *= opacity;
    color
}
fn floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}
fn sphere(point: Point) -> Vector3 {
    let longitude = point.longitude.to_radians();
    let latitude = point.latitude.to_radians();
    Vector3::new(
        (latitude.cos() * longitude.cos()) as f32,
        (latitude.cos() * longitude.sin()) as f32,
        latitude.sin() as f32,
    )
}
