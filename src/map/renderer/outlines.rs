// Ported from TinyTorrent2 MapRenderer.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::Graphics::Direct2D::{
        Common::{D2D_RECT_F, D2D1_COLOR_F},
        D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, ID2D1GeometryRealization, ID2D1PathGeometry1,
    },
    core::Result,
};
use windows_numerics::{Matrix3x2, Vector2};

use super::{
    Budget,
    device::{Device, Stroke},
    strength,
};
use crate::{
    geography::{Country, World},
    map::{Prepared, model::Model, viewport::Viewport},
};

fn blend(from: D2D1_COLOR_F, to: D2D1_COLOR_F, amount: f32) -> D2D1_COLOR_F {
    let mix = |from: f32, to: f32| ((from + (to - from) * amount) * 255.0).floor() / 255.0;
    D2D1_COLOR_F {
        r: mix(from.r, to.r),
        g: mix(from.g, to.g),
        b: mix(from.b, to.b),
        a: mix(from.a, to.a),
    }
}

pub struct Paths {
    world: Arc<World>,
    meridian: f32,
    // Target caches own shapes; the index cannot retain unused detail levels.
    land: Vec<HashMap<u32, Weak<Shape>>>,
}

impl Paths {
    pub fn new(world: Arc<World>, meridian: f32) -> Self {
        Self {
            land: (0..world.countries.len()).map(|_| HashMap::new()).collect(),
            world,
            meridian,
        }
    }

    pub fn matches(&self, world: &Arc<World>, meridian: f32) -> bool {
        Arc::ptr_eq(&self.world, world) && self.meridian == meridian
    }

    fn trace(&mut self, country: usize, detail: f32) -> Rc<Shape> {
        let paths = &mut self.land[country];
        if let Some(shape) = paths.get(&detail.to_bits()).and_then(Weak::upgrade) {
            return shape;
        }
        paths.retain(|_, shape| shape.strong_count() > 0);
        let shape = Rc::new(Shape::trace(
            &self.world,
            &self.world.countries[country],
            detail,
            self.meridian,
        ));
        paths.insert(detail.to_bits(), Rc::downgrade(&shape));
        shape
    }
}

pub struct Outlines {
    land: Vec<Option<Outline>>,
    window: Extent,
    window_detail: f32,
    model: Arc<Model>,
    starts: HashMap<Box<str>, f32>,
    started: Instant,
}

impl Outlines {
    #[cfg(test)]
    pub(super) fn native_rendering(&self, value: &mut super::NativeRendering) {
        use windows::core::Interface;
        value.fill_window = [
            self.window.low.X,
            self.window.low.Y,
            self.window.high.X,
            self.window.high.Y,
        ];
        value.detail = self.window_detail;
        for outline in self.land.iter().flatten() {
            value.shapes.push(Rc::as_ptr(&outline.shape) as usize);
            if let Some(geometry) = outline
                .fill
                .as_ref()
                .and_then(|fill| fill.geometry.as_ref())
            {
                value.fills.push(geometry.as_raw() as usize);
            }
            for stroke in outline
                .pieces
                .iter()
                .filter_map(|piece| piece.stroke.as_ref())
            {
                value.strokes.push(stroke.geometry.as_raw() as usize);
            }
        }
    }

    pub fn new() -> Self {
        Self {
            land: Vec::new(),
            window: Extent::empty(),
            window_detail: 0.0,
            model: Arc::new(Model::default()),
            starts: HashMap::new(),
            started: Instant::now(),
        }
    }

    pub fn invalidate_strokes(&mut self) {
        // Round joins were flattened for the previous physical pixel size.
        // Geographic paths and line-only fills remain valid across DPI changes.
        for outline in self.land.iter_mut().flatten() {
            for piece in &mut outline.pieces {
                piece.stroke = None;
            }
        }
    }

    pub fn draw(
        &mut self,
        device: &Device,
        paths: &mut Paths,
        prepared: &Prepared,
        budget: &mut Budget,
    ) -> Result<Option<Duration>> {
        let view = &prepared.viewport;
        if view.view_scale() <= 0.0 {
            return Ok(None);
        }
        self.land.resize_with(paths.world.countries.len(), || None);
        if !Arc::ptr_eq(&self.model, &prepared.model) {
            let progress = self.progress(prepared.appearance.static_marks());
            self.starts = prepared
                .model
                .countries
                .keys()
                .map(|code| {
                    let current = &prepared.model.groups[prepared.model.countries[code]];
                    let fallback = strength(
                        current
                            .rates
                            .map_or(0.0, |rates| rates.received + rates.sent),
                    );
                    let target = self.model.countries.get(code).map_or(fallback, |&index| {
                        let group = &self.model.groups[index];
                        strength(group.rates.map_or(0.0, |rates| rates.received + rates.sent))
                    });
                    let start = self.starts.get(code).copied().unwrap_or(target);
                    (code.clone(), start + (target - start) * progress)
                })
                .collect();
            self.model = Arc::clone(&prepared.model);
            self.started = Instant::now();
        }
        let detail = 2.0f32.powf((0.5 / view.view_scale()).log2().round_ties_even());
        let scale = view.axis_scale();
        let low = view.to_map(Vector2::zero());
        let high = view.to_map(view.size);
        let west = low.X.max(view.center.X - 180.0);
        let east = high.X.min(view.center.X + 180.0);
        let visible = Extent {
            low: Vector2::new(west, low.Y),
            high: Vector2::new(east, high.Y),
        }
        .grown(Vector2::new(2.0, 2.0) / scale);
        self.move_window(visible, detail);

        for index in 0..paths.world.countries.len() {
            if paths.world.countries[index].rings.is_empty() || self.land[index].is_some() {
                continue;
            }
            if !budget.allows() {
                break;
            }
            self.land[index] = Some(Outline::new(paths.trace(index, detail)));
        }

        // One longitude turn is visible. Copies continue countries across the cut.
        // SAFETY: All drawing resources belong to this thread and BeginDraw is live.
        unsafe {
            device.context.SetTransform(&Matrix3x2::identity());
            device.context.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: view.offset.X + (west - view.center.X) * scale.X,
                    top: 0.0,
                    right: view.offset.X + (east - view.center.X) * scale.X,
                    bottom: view.size.Y,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
        }
        let drawn = self.draw_land(device, paths, prepared, visible, detail, budget);
        // SAFETY: Restore drawing state even if a resource rebuild failed.
        unsafe {
            device.context.SetTransform(&Matrix3x2::identity());
            device.context.PopAxisAlignedClip();
        }
        drawn
    }

    fn draw_land(
        &mut self,
        device: &Device,
        paths: &mut Paths,
        prepared: &Prepared,
        visible: Extent,
        detail: f32,
        budget: &mut Budget,
    ) -> Result<Option<Duration>> {
        let view = &prepared.viewport;
        let scale = view.axis_scale();
        let colors = prepared.appearance.colors();
        let static_marks = prepared.appearance.static_marks();
        let progress = self.progress(static_marks);
        let mut next = None;
        for turn in -1..=1 {
            let shift = 360.0 * turn as f32;
            let transform = Matrix3x2::translation(shift, 0.0) * view.transform();
            // SAFETY: The view transform is a finite map-to-DIP transform.
            unsafe {
                device.context.SetTransform(&transform);
            }
            for (index, slot) in self.land.iter_mut().enumerate() {
                let Some(outline) = slot.as_mut() else {
                    continue;
                };
                if !outline.shape.extent.moved(shift).overlaps(visible) {
                    continue;
                }
                if outline.shape.detail != detail && budget.allows() {
                    let mut traced = Outline::new(paths.trace(index, detail));
                    traced.fill = outline.fill.take();
                    // Warm only visible bounded pieces during the tracing allowance.
                    for piece in &mut traced.pieces {
                        if piece.shape.extent.overlaps(visible.moved(-shift)) {
                            piece.line(device)?;
                        }
                    }
                    *outline = traced;
                }
                if let Some(fill) =
                    outline.filled(device, view, self.window, visible, turn, budget)?
                {
                    let mut color = colors.land;
                    if !device.contrast
                        && let Some(group) = self
                            .model
                            .countries
                            .get(&paths.world.countries[index].code)
                            .map(|&index| &self.model.groups[index])
                        && group.estimated
                        && let Some(highlight) = prepared.highlight
                    {
                        let opacity = highlight
                            .opacity(group.introduced, prepared.now, static_marks)
                            .max(if group.active > 0 { 1.0 } else { 0.0 });
                        let target =
                            strength(group.rates.map_or(0.0, |rates| rates.received + rates.sent));
                        let start = self
                            .starts
                            .get(&group.place.code)
                            .copied()
                            .unwrap_or(target);
                        let displayed = start + (target - start) * progress;
                        let amount = (if group.active > 0 {
                            0.1 + 0.35 * displayed
                        } else {
                            0.08
                        }) * opacity;
                        color = blend(colors.land, colors.download, amount);
                        if opacity > 0.0 {
                            if let Some(deadline) =
                                highlight.deadline(group.introduced, prepared.now, static_marks)
                            {
                                let delay = deadline.saturating_sub(prepared.now);
                                next = Some(next.map_or(delay, |old: Duration| old.min(delay)));
                            }
                            if !static_marks
                                && ((progress < 1.0 && start != target)
                                    || (prepared.now
                                        >= group.introduced.saturating_add(highlight.bright)
                                        && prepared.now
                                            < group
                                                .introduced
                                                .saturating_add(highlight.duration())))
                            {
                                next = Some(Duration::from_millis(33));
                            }
                        }
                    }
                    // SAFETY: The cached realization and brush belong to this device.
                    unsafe {
                        device.land.SetColor(&color);
                        device.context.DrawGeometryRealization(fill, &device.land);
                    }
                }
            }
        }

        let width = device.border_width();
        let mut stale = Vec::new();
        for turn in -1..=1 {
            let shift = 360.0 * turn as f32;
            let transform = Matrix3x2::translation(shift, 0.0) * view.transform();
            let mut applied = None;
            for (country, slot) in self.land.iter_mut().enumerate() {
                let Some(outline) = slot.as_mut() else {
                    continue;
                };
                if !outline.shape.extent.moved(shift).overlaps(visible) {
                    continue;
                }
                for (index, piece) in outline.pieces.iter_mut().enumerate() {
                    if !piece.shape.extent.moved(shift).overlaps(visible) {
                        continue;
                    }
                    if piece.stroke.is_none() && budget.allows() {
                        piece.restroke(device, scale, width)?;
                    }
                    if let Some(stroke) = &piece.stroke {
                        if !stroke.current(scale, 0.1) {
                            stale.push((country, index));
                        }
                        let widened = scale / stroke.scale;
                        let compensated =
                            Matrix3x2::scale(1.0 / stroke.scale.X, 1.0 / stroke.scale.Y)
                                * transform;
                        // A cached stroke at an older scale changes width; inverse
                        // opacity keeps its ink stable until the budget replaces it.
                        // SAFETY: The realization contains geometry widened at stroke.scale.
                        unsafe {
                            if applied != Some(stroke.scale) {
                                device.context.SetTransform(&compensated);
                                applied = Some(stroke.scale);
                            }
                            device.edge.SetOpacity(if device.contrast {
                                1.0
                            } else {
                                (0.5 / width) * 2.0 / (widened.X + widened.Y)
                            });
                            device
                                .context
                                .DrawGeometryRealization(&stroke.geometry, &device.edge);
                        }
                    } else {
                        let line = piece.line(device)?;
                        // SAFETY: Fixed transform behavior keeps the fallback pen in DIPs.
                        unsafe {
                            if applied != Some(Vector2::new(1.0, 1.0)) {
                                device.context.SetTransform(&transform);
                                applied = Some(Vector2::new(1.0, 1.0));
                            }
                            device
                                .edge
                                .SetOpacity(if device.contrast { 1.0 } else { 0.5 / width });
                            device.context.DrawGeometry(
                                &line,
                                &device.edge,
                                width,
                                &device.fixed_stroke,
                            );
                        }
                    }
                }
            }
        }
        // Missing strokes consume the allowance before replacements for stale ones.
        for (country, index) in stale {
            let piece = &mut self.land[country].as_mut().unwrap().pieces[index];
            if piece
                .stroke
                .as_ref()
                .is_some_and(|stroke| stroke.current(scale, 0.1))
            {
                continue;
            }
            if !budget.allows() {
                break;
            }
            piece.restroke(device, scale, width)?;
            budget.pending = true;
        }
        Ok(next)
    }

    fn progress(&self, static_marks: bool) -> f32 {
        if static_marks {
            1.0
        } else {
            (self.started.elapsed().as_secs_f32() / 1.6).min(1.0)
        }
    }

    fn move_window(&mut self, visible: Extent, detail: f32) {
        let span = visible.high - visible.low;
        let needed = visible.grown(span / 2.0);
        if self.window_detail == detail {
            for turn in -1..=1 {
                let moved = self.window.moved(360.0 * turn as f32);
                if moved.contains(needed) {
                    self.window = moved;
                    return;
                }
            }
        }
        self.window = visible.grown(span);
        self.window_detail = detail;
    }
}

struct Outline {
    shape: Rc<Shape>,
    pieces: Vec<Piece>,
    fill: Option<Fill>,
}

struct Shape {
    rings: Vec<Vec<Vector2>>,
    pieces: Vec<Rc<Line>>,
    extent: Extent,
    detail: f32,
}

impl Shape {
    fn trace(world: &World, country: &Country, detail: f32, meridian: f32) -> Self {
        let mut outline = Self {
            rings: Vec::new(),
            pieces: Vec::new(),
            extent: Extent::empty(),
            detail,
        };
        let mut lines = Vec::new();
        let mut gathered = 0;
        for parts in &country.rings {
            let mut ring = Vec::new();
            for part in parts {
                collect(world, *part, detail, false, &mut ring);
            }
            if ring.is_empty() {
                continue;
            }
            let bounds = Extent::of(&ring);
            let shift = 360.0
                * (-((bounds.low.X + bounds.high.X) / 2.0 - meridian) / 360.0).round_ties_even()
                - meridian;
            let placed = place(&ring, shift);
            outline.extent = outline.extent.with(Extent::of(&placed));
            if placed.len() < 3 {
                continue;
            }
            outline.rings.push(placed);
            for part in parts.iter().filter(|part| part.draws) {
                let mut line = Vec::new();
                collect(world, *part, detail, true, &mut line);
                for start in (0..line.len().saturating_sub(1)).step_by(Piece::LENGTH - 1) {
                    let count = Piece::LENGTH.min(line.len() - start);
                    if gathered + count > Piece::LENGTH {
                        outline
                            .pieces
                            .push(Rc::new(Line::new(std::mem::take(&mut lines))));
                        gathered = 0;
                    }
                    lines.push(place(&line[start..start + count], shift));
                    gathered += count;
                }
            }
        }
        if !lines.is_empty() {
            outline.pieces.push(Rc::new(Line::new(lines)));
        }
        outline
    }
}

impl Outline {
    fn new(shape: Rc<Shape>) -> Self {
        Self {
            pieces: shape
                .pieces
                .iter()
                .map(|piece| Piece {
                    shape: Rc::clone(piece),
                    stroke: None,
                })
                .collect(),
            shape,
            fill: None,
        }
    }

    fn filled(
        &mut self,
        device: &Device,
        view: &Viewport,
        window: Extent,
        visible: Extent,
        turn: i32,
        budget: &mut Budget,
    ) -> Result<Option<&ID2D1GeometryRealization>> {
        let shift = 360.0 * turn as f32;
        let own_window = window.moved(-shift);
        let turns = (-1..=1)
            .filter(|other| {
                self.shape
                    .extent
                    .moved(360.0 * *other as f32)
                    .overlaps(window)
            })
            .count();
        let clip = if turns > 1 || own_window.contains(self.shape.extent) {
            None
        } else {
            Some(own_window)
        };
        let covers = self.fill.as_ref().is_none_or(|fill| {
            fill.window
                .is_none_or(|made| made.contains(visible.moved(-shift).within(self.shape.extent)))
        });
        let current = self.fill.as_ref().is_some_and(|fill| {
            fill.detail == self.shape.detail && (fill.window.is_none() || fill.window == clip)
        });
        if !current && (!covers || budget.allows()) {
            let rings: Vec<_> = self
                .shape
                .rings
                .iter()
                .map(|ring| {
                    clip.map_or_else(
                        || Cow::Borrowed(ring.as_slice()),
                        |bounds| crop(ring, bounds),
                    )
                })
                .filter(|ring| ring.len() >= 3)
                .collect();
            let geometry = if rings.is_empty() {
                None
            } else {
                let path = device.path(rings.iter().map(|ring| ring.as_ref()), true)?;
                // SAFETY: The line-only path has alternate fill and device-local ownership.
                Some(unsafe {
                    device.context.CreateFilledGeometryRealization(
                        &path,
                        device.tolerance(view.view_scale()),
                    )?
                })
            };
            self.fill = Some(Fill {
                geometry,
                detail: self.shape.detail,
                window: clip,
            });
        }
        Ok(self.fill.as_ref().and_then(|fill| fill.geometry.as_ref()))
    }
}

fn collect(
    world: &World,
    part: crate::geography::Part,
    detail: f32,
    start: bool,
    points: &mut Vec<Vector2>,
) {
    let section = &world.sections[part.section];
    for step in usize::from(!start)..section.len() {
        let vertex = section[if part.reversed {
            section.len() - 1 - step
        } else {
            step
        }];
        if vertex.level >= detail {
            points.push(vertex.position);
        }
    }
}

fn place(points: &[Vector2], shift: f32) -> Vec<Vector2> {
    points
        .iter()
        .map(|point| Vector2::new(point.X + shift, -point.Y))
        .collect()
}

struct Piece {
    shape: Rc<Line>,
    stroke: Option<Stroke>,
}

struct Line {
    lines: Vec<Vec<Vector2>>,
    extent: Extent,
    path: RefCell<Option<ID2D1PathGeometry1>>,
}

impl Line {
    fn new(lines: Vec<Vec<Vector2>>) -> Self {
        let extent = lines.iter().fold(Extent::empty(), |extent, line| {
            extent.with(Extent::of(line))
        });
        Self {
            lines,
            extent,
            path: RefCell::new(None),
        }
    }
}

impl Piece {
    const LENGTH: usize = 512;

    fn line(&self, device: &Device) -> Result<ID2D1PathGeometry1> {
        let mut path = self.shape.path.borrow_mut();
        if path.is_none() {
            *path = Some(device.path(self.shape.lines.iter().map(Vec::as_slice), false)?);
        }
        Ok(path.as_ref().unwrap().clone())
    }

    fn restroke(&mut self, device: &Device, scale: Vector2, width: f32) -> Result<()> {
        self.stroke = Some(device.stroke(&self.line(device)?, scale, width)?);
        Ok(())
    }
}

struct Fill {
    geometry: Option<ID2D1GeometryRealization>,
    detail: f32,
    window: Option<Extent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Extent {
    low: Vector2,
    high: Vector2,
}

impl Extent {
    fn empty() -> Self {
        Self {
            low: Vector2::new(f32::MAX, f32::MAX),
            high: Vector2::new(f32::MIN, f32::MIN),
        }
    }
    fn of(points: &[Vector2]) -> Self {
        points.iter().fold(Self::empty(), |extent, point| {
            extent.with(Self {
                low: *point,
                high: *point,
            })
        })
    }
    fn with(self, other: Self) -> Self {
        Self {
            low: Vector2::new(self.low.X.min(other.low.X), self.low.Y.min(other.low.Y)),
            high: Vector2::new(self.high.X.max(other.high.X), self.high.Y.max(other.high.Y)),
        }
    }
    fn moved(self, longitude: f32) -> Self {
        let offset = Vector2::new(longitude, 0.0);
        Self {
            low: self.low + offset,
            high: self.high + offset,
        }
    }
    fn grown(self, margin: Vector2) -> Self {
        Self {
            low: self.low - margin,
            high: self.high + margin,
        }
    }
    fn within(self, other: Self) -> Self {
        Self {
            low: Vector2::new(self.low.X.max(other.low.X), self.low.Y.max(other.low.Y)),
            high: Vector2::new(self.high.X.min(other.high.X), self.high.Y.min(other.high.Y)),
        }
    }
    fn overlaps(self, other: Self) -> bool {
        self.high.X >= other.low.X
            && self.low.X <= other.high.X
            && self.high.Y >= other.low.Y
            && self.low.Y <= other.high.Y
    }
    fn contains(self, other: Self) -> bool {
        other.low.X >= self.low.X
            && other.high.X <= self.high.X
            && other.low.Y >= self.low.Y
            && other.high.Y <= self.high.Y
    }
}

// Sutherland-Hodgman clips rings independently; alternate fill preserves holes.
fn crop(ring: &[Vector2], window: Extent) -> Cow<'_, [Vector2]> {
    let extent = Extent::of(ring);
    if window.contains(extent) {
        return Cow::Borrowed(ring);
    }
    if !window.overlaps(extent) {
        return Cow::Borrowed(&[]);
    }
    let mut points = ring.to_vec();
    for edge in 0..4 {
        if points.is_empty() {
            break;
        }
        let inside = |point: Vector2| match edge {
            0 => point.X >= window.low.X,
            1 => point.X <= window.high.X,
            2 => point.Y >= window.low.Y,
            _ => point.Y <= window.high.Y,
        };
        let across = |from: Vector2, to: Vector2| {
            let amount = match edge {
                0 => (window.low.X - from.X) / (to.X - from.X),
                1 => (window.high.X - from.X) / (to.X - from.X),
                2 => (window.low.Y - from.Y) / (to.Y - from.Y),
                _ => (window.high.Y - from.Y) / (to.Y - from.Y),
            };
            from + (to - from) * amount
        };
        let mut kept = Vec::new();
        let mut previous = *points.last().unwrap();
        for point in points {
            if inside(point) != inside(previous) {
                kept.push(across(previous, point));
            }
            if inside(point) {
                kept.push(point);
            }
            previous = point;
        }
        points = kept;
    }
    Cow::Owned(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_share_country_shapes_without_retaining_unused_detail() {
        let world = Arc::new(World::decode(include_bytes!("../../../assets/world.bin")).unwrap());
        let country = world
            .countries
            .iter()
            .position(|country| country.code.as_ref() == "ZA")
            .unwrap();
        let mut paths = Paths::new(world, 0.0);
        let first = paths.trace(country, 0.5);
        let second = paths.trace(country, 0.5);
        assert!(Rc::ptr_eq(&first, &second));
        let retired = Rc::downgrade(&first);
        drop(first);
        assert!(retired.upgrade().is_some());
        drop(second);
        assert!(retired.upgrade().is_none());
        let next = paths.trace(country, 0.25);
        assert_eq!(next.detail, 0.25);
        assert_eq!(paths.land[country].len(), 1);
    }

    #[test]
    fn a_ring_enclosing_the_crop_keeps_its_four_crossing_edges() {
        let ring = [
            Vector2::new(-5.0, -5.0),
            Vector2::new(5.0, -5.0),
            Vector2::new(5.0, 5.0),
            Vector2::new(-5.0, 5.0),
        ];
        let window = Extent {
            low: Vector2::new(-1.0, -1.0),
            high: Vector2::new(1.0, 1.0),
        };
        let cropped = crop(&ring, window);
        assert_eq!(cropped.len(), 4);
        assert_eq!(Extent::of(&cropped), window);
        assert!(crop(&ring, window.moved(30.0)).is_empty());
    }

    #[test]
    fn source_outline_keeps_holes_and_bounds_each_border_rebuild() {
        let world = World::decode(include_bytes!("../../../assets/world.bin")).unwrap();
        let south_africa = Shape::trace(&world, world.country("ZA").unwrap(), 0.01, 0.0);
        assert!(south_africa.rings.len() > 1, "Lesotho must remain a hole");
        for country in &world.countries {
            let outline = Shape::trace(&world, country, 0.01, 139.7);
            assert!(
                outline
                    .pieces
                    .iter()
                    .all(|piece| piece.lines.iter().map(Vec::len).sum::<usize>() <= Piece::LENGTH)
            );
        }
    }
}
