use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F},
    },
    core::Result,
};

use crate::connections::{Highlight, Rates};
use crate::geography::{Point, World};
use navigation::Navigation;
use viewport::Viewport;
use windows_numerics::Vector2;

pub(crate) mod model;
mod navigation;
pub(crate) mod palette;
mod renderer;
pub mod viewport;
pub use navigation::{Capture, Command, Direction, Input};
pub use palette::Palette;
#[cfg(test)]
pub(crate) use renderer::NativeRendering;

#[cfg(test)]
#[derive(Default)]
pub(crate) struct Presentations {
    pub returned: Cell<Option<Instant>>,
    pub attempts: Cell<u64>,
    pub latest: Cell<Option<PresentedFrame>>,
    pub pulses: RefCell<Vec<PresentedPulse>>,
    pub capture: RefCell<Option<std::path::PathBuf>>,
    pub captured: Cell<Option<PresentedFrame>>,
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct PresentedPulse {
    pub key: model::DestinationKey,
    pub sending: bool,
    pub position: Vector2,
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct PresentedFrame {
    pub target: HWND,
    pub revision: u64,
    pub at: Duration,
    pub model: usize,
}

pub struct Map {
    state: RefCell<State>,
    renderer: RefCell<Option<renderer::Renderer>>,
    target: Cell<Option<HWND>>,
    surfaces: RefCell<HashSet<isize>>,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            state: RefCell::default(),
            renderer: RefCell::new(Some(renderer::Renderer::default())),
            target: Cell::default(),
            surfaces: RefCell::default(),
        }
    }
}

#[derive(Default)]
struct State {
    viewport: Viewport,
    navigation: Navigation,
    world: Option<Arc<World>>,
    origin: Option<Point>,
    model: Arc<model::Model>,
    sampled: Option<(Duration, Instant)>,
    highlight: Option<Highlight>,
    appearance: Appearance,
    speeds: Speeds,
    navigation_bounds: Option<D2D_RECT_F>,
    revision: u64,
}

pub struct Prepared {
    pub viewport: Viewport,
    pub world: Option<Arc<World>>,
    pub model: Arc<model::Model>,
    pub origin: Option<Point>,
    pub now: Duration,
    pub highlight: Option<Highlight>,
    pub inspection: Inspection,
    pub appearance: Appearance,
    pub speeds: Speeds,
    pub navigation_bounds: Option<D2D_RECT_F>,
    revision: u64,
}

pub struct Update {
    pub changed: bool,
    pub capture: Capture,
}

pub enum Frame {
    Idle,
    Again,
    After(Duration),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeedCorner {
    TopLeft,
    BottomLeft,
    #[default]
    BottomRight,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Speeds {
    pub corner: SpeedCorner,
    pub rates: Option<Rates>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Inspection {
    pub pointed: Option<model::DestinationKey>,
    pub selected: Option<model::Place>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Appearance {
    pub palette: Palette,
    pub contrast: Option<(D2D1_COLOR_F, D2D1_COLOR_F)>,
    pub animations: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            contrast: None,
            animations: true,
        }
    }
}

impl Appearance {
    pub fn colors(self) -> Palette {
        match self.contrast {
            Some((background, ink)) => Palette {
                background,
                ink,
                land: background,
                edge: ink,
                download: ink,
                upload: ink,
            },
            None => self.palette,
        }
    }
    pub fn static_marks(self) -> bool {
        self.contrast.is_some() || !self.animations
    }
}

impl Map {
    #[cfg(test)]
    pub(crate) fn native_revision(&self) -> u64 {
        self.state.borrow().revision
    }

    #[cfg(test)]
    pub(crate) fn native_rendering(&self) -> Option<NativeRendering> {
        self.renderer.borrow().as_ref()?.native_rendering()
    }

    #[cfg(test)]
    pub(crate) fn native_image(&self, path: &std::path::Path) -> Result<()> {
        let renderer = self.renderer.borrow();
        let renderer = renderer.as_ref().ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Native check requires an attached map renderer",
            )
        })?;
        renderer.native_image(path)
    }

    #[cfg(test)]
    pub(crate) fn observe_presentations(&self) -> Result<std::rc::Rc<Presentations>> {
        let observations = std::rc::Rc::new(Presentations::default());
        let attached = self.with_renderer(|renderer| {
            renderer.observe_presentations(std::rc::Rc::clone(&observations))
        })?;
        if attached.is_none() {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Native check requires an attached map renderer",
            ));
        }
        Ok(observations)
    }

    pub fn set_navigation_bounds(&self, bounds: Option<D2D_RECT_F>) {
        let mut state = self.state.borrow_mut();
        if state.navigation_bounds != bounds {
            state.navigation_bounds = bounds;
            state.revision += 1;
        }
    }
    pub fn set_speeds(&self, speeds: Speeds) -> bool {
        let mut state = self.state.borrow_mut();
        if state.speeds == speeds {
            return false;
        }
        state.speeds = speeds;
        state.revision += 1;
        true
    }
    pub fn appearance(&self) -> Appearance {
        self.state.borrow().appearance
    }

    pub fn set_appearance(&self, appearance: Appearance) -> bool {
        let mut state = self.state.borrow_mut();
        if state.appearance == appearance {
            return false;
        }
        state.appearance = appearance;
        state.revision += 1;
        true
    }
    pub fn attach(&self, target: HWND) -> Result<()> {
        self.target.set(Some(target));
        self.with_renderer(|renderer| renderer.attach(target))
            .map(|_| ())
    }

    pub fn graphics_mode(&self) -> Option<&'static str> {
        self.renderer
            .borrow()
            .as_ref()
            .filter(|_| self.target.get().is_some())
            .map(|renderer| renderer.graphics_mode())
    }

    pub fn detach(&self) {
        self.target.set(None);
        let _ = self.with_renderer(|_| Ok(()));
    }

    pub fn attach_surface(&self, target: HWND) -> Result<()> {
        self.surfaces.borrow_mut().insert(target.0 as isize);
        self.with_renderer(|renderer| renderer.attach(target))
            .map(|_| ())
    }

    pub fn detach_surface(&self, target: HWND) {
        self.surfaces.borrow_mut().remove(&(target.0 as isize));
        let _ = self.with_renderer(|_| Ok(()));
    }

    pub fn paint_surface(&self, target: HWND, width: u32, height: u32, dpi: u32) -> Result<Frame> {
        let mut prepared = self.prepared(Inspection::default());
        prepared.viewport.resize(
            f64::from(width) * 96.0 / f64::from(dpi.max(1)),
            f64::from(height) * 96.0 / f64::from(dpi.max(1)),
        );
        prepared.navigation_bounds = None;
        if !self.surfaces.borrow().contains(&(target.0 as isize)) {
            return Ok(Frame::Idle);
        }
        self.with_renderer(|renderer| renderer.paint_target(target, &prepared, width, height, dpi))
            .map(|frame| {
                if self.state.borrow().revision != prepared.revision {
                    Frame::Again
                } else {
                    frame.unwrap_or(Frame::Idle)
                }
            })
    }

    pub fn set_world(&self, world: Arc<World>) {
        let mut state = self.state.borrow_mut();
        state.world = Some(world);
        state.revision += 1;
    }

    pub fn viewport(&self) -> Viewport {
        self.state.borrow().viewport
    }

    pub fn world(&self) -> Option<Arc<World>> {
        self.state.borrow().world.clone()
    }

    pub fn take_world(&self) -> Option<Arc<World>> {
        self.state.borrow_mut().world.take()
    }

    pub fn unlocated(&self) -> usize {
        self.state.borrow().model.unlocated
    }

    pub fn set_model(
        &self,
        model: Arc<model::Model>,
        at: Duration,
        sampled: Instant,
    ) -> (bool, Arc<model::Model>) {
        let mut state = self.state.borrow_mut();
        let changed = !Arc::ptr_eq(&state.model, &model);
        let previous = std::mem::replace(&mut state.model, model);
        // Keep the collection clock anchor through lookup and UI delivery.
        // Accepting a delayed prepared scene must not replay its highlight.
        state.sampled = Some((at, sampled));
        if changed {
            state.revision += 1;
        }
        (changed, previous)
    }

    pub fn set_highlight(&self, highlight: Highlight) -> bool {
        let mut state = self.state.borrow_mut();
        if state
            .highlight
            .is_some_and(|old| old.bright == highlight.bright && old.fade == highlight.fade)
        {
            return false;
        }
        state.highlight = Some(highlight);
        state.revision += 1;
        true
    }

    pub fn set_origin(&self, origin: Option<Point>) -> Update {
        let mut state = self.state.borrow_mut();
        if state.origin == origin {
            return Update {
                changed: false,
                capture: Capture::Keep,
            };
        }
        state.origin = origin;
        let State {
            navigation,
            viewport,
            ..
        } = &mut *state;
        let capture = navigation.input(
            viewport,
            Input::Rebase(origin.map_or(0.0, |point| point.longitude as f32)),
        );
        state.revision += 1;
        Update {
            changed: true,
            capture,
        }
    }

    pub fn panning(&self) -> bool {
        self.state.borrow().navigation.panning()
    }

    pub fn pressed(&self) -> bool {
        self.state.borrow().navigation.pressed()
    }

    pub fn hit(&self, position: Vector2) -> Option<model::DestinationKey> {
        let state = self.state.borrow();
        let now = state.sampled.map_or(Duration::ZERO, |(at, sampled)| {
            at.saturating_add(sampled.elapsed())
        });
        self.renderer.borrow().as_ref()?.hit_target(
            self.target.get()?,
            &state.viewport,
            position,
            now,
            state.highlight?,
            state.appearance.static_marks(),
        )
    }

    pub fn place(&self, key: &model::DestinationKey) -> Option<model::Place> {
        let state = self.state.borrow();
        let destination = state
            .model
            .destinations
            .iter()
            .find(|destination| destination.key == *key)?;
        Some(state.model.groups[destination.group].place.clone())
    }

    pub fn has_place(&self, place: &model::Place) -> bool {
        self.state
            .borrow()
            .model
            .groups
            .iter()
            .any(|group| group.place == *place)
    }

    pub fn describe(&self, inspection: &Inspection) -> Option<String> {
        let state = self.state.borrow();
        let destination = state
            .model
            .destinations
            .iter()
            .find(|destination| inspection.pointed.as_ref() == Some(&destination.key))
            .or_else(|| {
                state.model.destinations.iter().find(|destination| {
                    inspection.selected.as_ref()
                        == Some(&state.model.groups[destination.group].place)
                })
            })?;
        Some(state.model.describe(destination))
    }

    pub fn cycle(&self, inspection: &Inspection, backwards: bool) -> Option<model::DestinationKey> {
        let state = self.state.borrow();
        if state.viewport.scale <= 0.0 {
            return None;
        }
        let now = state.sampled.map_or(Duration::ZERO, |(at, sampled)| {
            at.saturating_add(sampled.elapsed())
        });
        let highlight = state.highlight?;
        let destinations: Vec<_> = state
            .model
            .destinations
            .iter()
            .filter(|destination| {
                let group = &state.model.groups[destination.group];
                highlight.opacity(group.introduced, now, state.appearance.static_marks()) > 0.0
                    || group
                        .rates
                        .is_some_and(|rates| rates.received + rates.sent > 0.0)
            })
            .collect();
        if destinations.is_empty() {
            return None;
        }
        let current = destinations
            .iter()
            .position(|destination| inspection.pointed.as_ref() == Some(&destination.key))
            .or_else(|| {
                destinations.iter().position(|destination| {
                    inspection.selected.as_ref()
                        == Some(&state.model.groups[destination.group].place)
                })
            });
        let index = match current {
            Some(index) if backwards => (index + destinations.len() - 1) % destinations.len(),
            Some(index) => (index + 1) % destinations.len(),
            None if backwards => destinations.len() - 1,
            None => 0,
        };
        Some(destinations[index].key.clone())
    }

    pub fn reveal(&self, key: &model::DestinationKey) -> Update {
        let point = {
            let state = self.state.borrow();
            state
                .model
                .destinations
                .iter()
                .find(|destination| destination.key == *key)
                .map(|destination| destination.point)
        };
        point.map_or(
            Update {
                changed: false,
                capture: Capture::Keep,
            },
            |point| self.input(Input::Reveal(point)),
        )
    }

    pub fn zoom_inspection(&self, inspection: &Inspection, steps: f64) -> Update {
        let key = inspection.pointed.clone().or_else(|| {
            let state = self.state.borrow();
            state
                .model
                .destinations
                .iter()
                .find(|destination| {
                    inspection.selected.as_ref()
                        == Some(&state.model.groups[destination.group].place)
                })
                .map(|destination| destination.key.clone())
        });
        if let Some(key) = &key {
            self.reveal(key);
        }
        let anchor = {
            let state = self.state.borrow();
            key.as_ref()
                .and_then(|key| {
                    state
                        .model
                        .destinations
                        .iter()
                        .find(|destination| destination.key == *key)
                })
                .map_or(state.viewport.offset, |destination| {
                    state.viewport.screen(destination.point)
                })
        };
        self.input(Input::Zoom { steps, anchor })
    }

    pub fn capture(&self) -> Capture {
        self.state.borrow().navigation.capture()
    }

    pub fn available(&self, command: Command) -> bool {
        Navigation::available(&self.state.borrow().viewport, command)
    }

    pub fn input(&self, input: Input) -> Update {
        let mut state = self.state.borrow_mut();
        let before = state.viewport;
        let State {
            viewport,
            navigation,
            ..
        } = &mut *state;
        let capture = navigation.input(viewport, input);
        let changed = state.viewport != before;
        if changed {
            state.revision += 1;
        }
        Update { changed, capture }
    }

    pub fn resize(&self, width: u32, height: u32, dpi: u32) -> Update {
        let mut state = self.state.borrow_mut();
        let size = windows_numerics::Vector2::new(
            (f64::from(width) * 96.0 / f64::from(dpi)) as f32,
            (f64::from(height) * 96.0 / f64::from(dpi)) as f32,
        );
        if state.viewport.size == size {
            return Update {
                changed: false,
                capture: Capture::Keep,
            };
        }
        let State {
            viewport,
            navigation,
            ..
        } = &mut *state;
        let capture = navigation.input(viewport, Input::Cancel);
        state.viewport.resize(
            f64::from(width) * 96.0 / f64::from(dpi),
            f64::from(height) * 96.0 / f64::from(dpi),
        );
        state.revision += 1;
        Update {
            changed: true,
            capture,
        }
    }

    pub fn paint(
        &self,
        width: u32,
        height: u32,
        dpi: u32,
        inspection: Inspection,
    ) -> Result<Frame> {
        let prepared = self.prepared(inspection);
        let target = self.target.get();
        let Some(target) = target else {
            return Ok(Frame::Idle);
        };
        self.with_renderer(|renderer| renderer.paint_target(target, &prepared, width, height, dpi))
            .map(|frame| {
                if self.state.borrow().revision != prepared.revision {
                    Frame::Again
                } else {
                    frame.unwrap_or(Frame::Idle)
                }
            })
    }

    fn with_renderer<T>(
        &self,
        action: impl FnOnce(&mut renderer::Renderer) -> Result<T>,
    ) -> Result<Option<T>> {
        // Lease the single renderer across native calls. Nested messages can
        // change attachment intent, but cannot create or draw another device.
        let Some(mut renderer) = self.renderer.take() else {
            return Ok(None);
        };
        let result = action(&mut renderer);
        // Destruction may reenter and retire another target already examined.
        // Repeat only while resources were removed; no new targets are created here.
        while renderer.retain_targets(|window| {
            self.target.get() == Some(window)
                || self.surfaces.borrow().contains(&(window.0 as isize))
        }) {}
        self.renderer.replace(Some(renderer));
        result.map(Some)
    }

    fn prepared(&self, inspection: Inspection) -> Prepared {
        let state = self.state.borrow();
        Prepared {
            viewport: state.viewport,
            world: state.world.clone(),
            model: Arc::clone(&state.model),
            origin: state.origin,
            now: state.sampled.map_or(Duration::ZERO, |(at, sampled)| {
                at.saturating_add(sampled.elapsed())
            }),
            highlight: state.highlight,
            inspection,
            appearance: state.appearance,
            speeds: state.speeds,
            navigation_bounds: state.navigation_bounds,
            revision: state.revision,
        }
    }
}
