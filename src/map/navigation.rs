// Ported from TinyTorrent2 PeerMap/Navigation.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::viewport::{MAX_ZOOM, Viewport};
use crate::geography::Point;
use windows_numerics::Vector2;

#[derive(Clone, Copy)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy)]
pub enum Command {
    ZoomIn,
    ZoomOut,
    Fit,
    Pan(Direction),
}

pub enum Input {
    Press(Vector2),
    Move { position: Vector2, left_down: bool },
    Captured { id: u64, success: bool },
    Release(Vector2),
    Cancel,
    Rebase(f32),
    Reveal(Point),
    Zoom { steps: f64, anchor: Vector2 },
    Command(Command),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Capture {
    Keep,
    Acquire(u64),
    Release,
}

#[derive(Default)]
pub struct Navigation {
    drag: Drag,
    generation: u64,
}

#[derive(Default)]
enum Drag {
    #[default]
    Idle,
    Pressed(Grip),
    Capturing {
        id: u64,
        grip: Grip,
        position: Vector2,
    },
    Panning(Grip),
}

struct Grip {
    start: Vector2,
    point: Vector2,
    center: Vector2,
    ratio: f32,
}

impl Navigation {
    pub fn pressed(&self) -> bool {
        !matches!(self.drag, Drag::Idle)
    }
    pub fn panning(&self) -> bool {
        matches!(self.drag, Drag::Panning(_))
    }

    pub fn capture(&self) -> Capture {
        match self.drag {
            Drag::Capturing { id, .. } => Capture::Acquire(id),
            Drag::Panning(_) => Capture::Keep,
            _ => Capture::Release,
        }
    }

    pub fn available(view: &Viewport, command: Command) -> bool {
        if view.scale <= 0.0 {
            return false;
        }
        match command {
            Command::ZoomIn => view.zoom < MAX_ZOOM,
            Command::ZoomOut => view.zoom > 1.0,
            Command::Fit => !view.fitted(),
            Command::Pan(_) => true,
        }
    }

    pub fn input(&mut self, view: &mut Viewport, input: Input) -> Capture {
        match input {
            Input::Press(start) if matches!(self.drag, Drag::Idle) && view.scale > 0.0 => {
                self.drag = Drag::Pressed(Grip {
                    start,
                    point: view.to_map(start),
                    center: view.center,
                    ratio: view.ratio,
                });
            }
            Input::Move {
                position,
                left_down,
            } => {
                if !left_down && !matches!(self.drag, Drag::Idle) {
                    return self.end(view, true);
                }
                match &mut self.drag {
                    Drag::Pressed(grip) if (position - grip.start).length_squared() >= 16.0 => {
                        let Drag::Pressed(grip) = std::mem::take(&mut self.drag) else {
                            unreachable!()
                        };
                        self.generation += 1;
                        self.drag = Drag::Capturing {
                            id: self.generation,
                            grip,
                            position,
                        };
                        return Capture::Acquire(self.generation);
                    }
                    Drag::Capturing {
                        position: latest, ..
                    } => *latest = position,
                    Drag::Panning(grip) => view.hold(view.zoom, grip.point, position),
                    _ => {}
                }
            }
            Input::Captured { id, success } => {
                // Native capture may dispatch messages. Accept its completion only
                // while the same pending transition still owns the drag.
                if !matches!(self.drag, Drag::Capturing { id: pending, .. } if pending == id) {
                    return Capture::Keep;
                }
                let Drag::Capturing { grip, position, .. } = std::mem::take(&mut self.drag) else {
                    unreachable!()
                };
                if !success {
                    return Capture::Release;
                }
                view.hold(view.zoom, grip.point, position);
                self.drag = Drag::Panning(grip);
            }
            Input::Release(position) => {
                if let Drag::Panning(grip) = &self.drag {
                    view.hold(view.zoom, grip.point, position);
                }
                return self.end(view, false);
            }
            Input::Cancel => return self.end(view, true),
            Input::Rebase(meridian) => {
                self.end(view, false);
                view.rebase(meridian);
                return Capture::Release;
            }
            Input::Zoom { steps, anchor } => {
                self.end(view, false);
                view.zoom_at(1.25f64.powf(steps) as f32, anchor);
                return Capture::Release;
            }
            Input::Reveal(point) => {
                self.end(view, false);
                view.reveal(point);
                return Capture::Release;
            }
            Input::Command(command) if Self::available(view, command) => {
                self.end(view, false);
                match command {
                    Command::ZoomIn => view.zoom_at(1.25, view.offset),
                    Command::ZoomOut => view.zoom_at(0.8, view.offset),
                    Command::Fit => view.fit(),
                    Command::Pan(direction) => {
                        let delta = match direction {
                            Direction::Left => Vector2::new(view.offset.X / 4.0, 0.0),
                            Direction::Right => Vector2::new(-view.offset.X / 4.0, 0.0),
                            Direction::Up => Vector2::new(0.0, view.offset.Y / 4.0),
                            Direction::Down => Vector2::new(0.0, -view.offset.Y / 4.0),
                        };
                        view.pan(delta);
                    }
                }
                return Capture::Release;
            }
            _ => {}
        }
        Capture::Keep
    }

    fn end(&mut self, view: &mut Viewport, cancel: bool) -> Capture {
        let previous = std::mem::take(&mut self.drag);
        if cancel && let Drag::Panning(grip) = previous {
            view.restore(grip.center, grip.ratio);
        }
        Capture::Release
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Viewport {
        let mut view = Viewport::default();
        view.resize(1280.0, 720.0);
        view.zoom_at(4.0, view.offset);
        view
    }

    #[test]
    fn drag_waits_for_four_dips_and_successful_capture_before_panning() {
        let mut view = view();
        let original = view;
        let mut navigation = Navigation::default();
        navigation.input(&mut view, Input::Press(Vector2::new(600.0, 300.0)));
        assert!(
            navigation.input(
                &mut view,
                Input::Move {
                    position: Vector2::new(603.0, 300.0),
                    left_down: true
                }
            ) == Capture::Keep
        );
        assert!(view == original);
        assert!(
            navigation.input(
                &mut view,
                Input::Move {
                    position: Vector2::new(604.0, 300.0),
                    left_down: true
                }
            ) == Capture::Acquire(1)
        );
        assert!(view == original);
        navigation.input(
            &mut view,
            Input::Captured {
                id: 1,
                success: false,
            },
        );
        assert!(!navigation.panning());
        assert!(view == original);
    }

    #[test]
    fn reentrant_pointer_move_is_kept_when_native_capture_completes() {
        let mut view = view();
        let mut navigation = Navigation::default();
        let start = Vector2::new(600.0, 300.0);
        let grip = view.to_map(start);
        navigation.input(&mut view, Input::Press(start));
        navigation.input(
            &mut view,
            Input::Move {
                position: start + Vector2::new(4.0, 0.0),
                left_down: true,
            },
        );
        let latest = start + Vector2::new(20.0, 10.0);
        navigation.input(
            &mut view,
            Input::Move {
                position: latest,
                left_down: true,
            },
        );
        navigation.input(
            &mut view,
            Input::Captured {
                id: 1,
                success: true,
            },
        );
        assert!(navigation.panning());
        assert!((view.to_screen(grip) - latest).length_squared() < 0.0001);
    }

    #[test]
    fn cancelling_a_pan_restores_the_view_but_a_late_capture_cannot_restore_an_old_origin() {
        let mut view = view();
        let original = view;
        let mut navigation = Navigation::default();
        let start = view.offset;
        navigation.input(&mut view, Input::Press(start));
        navigation.input(
            &mut view,
            Input::Move {
                position: start + Vector2::new(30.0, 20.0),
                left_down: true,
            },
        );
        navigation.input(
            &mut view,
            Input::Captured {
                id: 1,
                success: true,
            },
        );
        assert!(view != original);
        navigation.input(&mut view, Input::Cancel);
        assert!(view == original);
        navigation.input(&mut view, Input::Press(start));
        navigation.input(
            &mut view,
            Input::Move {
                position: start + Vector2::new(30.0, 0.0),
                left_down: true,
            },
        );
        navigation.input(&mut view, Input::Cancel);
        view.rebase(139.7);
        let rebased = view;
        assert!(
            navigation.input(
                &mut view,
                Input::Captured {
                    id: 2,
                    success: true
                }
            ) == Capture::Keep
        );
        assert!(view == rebased);
    }

    #[test]
    fn old_capture_completion_does_not_consume_a_new_drag() {
        let mut view = view();
        let mut navigation = Navigation::default();
        let start = view.offset;
        for id in [1, 2] {
            navigation.input(&mut view, Input::Press(start));
            assert!(
                navigation.input(
                    &mut view,
                    Input::Move {
                        position: start + Vector2::new(20.0, 0.0),
                        left_down: true,
                    }
                ) == Capture::Acquire(id)
            );
            if id == 1 {
                navigation.input(&mut view, Input::Cancel);
            }
        }
        let original = view;
        navigation.input(
            &mut view,
            Input::Captured {
                id: 1,
                success: true,
            },
        );
        assert!(view == original);
        navigation.input(
            &mut view,
            Input::Captured {
                id: 2,
                success: true,
            },
        );
        assert!(navigation.panning());
        assert!(view != original);
    }

    #[test]
    fn fractional_wheel_keeps_its_anchor_and_commands_respect_zoom_limits() {
        let mut view = view();
        let mut navigation = Navigation::default();
        let anchor = Vector2::new(720.0, 300.0);
        let point = view.to_map(anchor);
        navigation.input(&mut view, Input::Zoom { steps: 0.5, anchor });
        assert!((view.zoom - 4.0 * 1.25f32.sqrt()).abs() < 0.00001);
        assert!((view.to_screen(point) - anchor).length_squared() < 0.0001);
        for _ in 0..30 {
            navigation.input(&mut view, Input::Command(Command::ZoomIn));
        }
        assert_eq!(view.zoom, MAX_ZOOM);
        assert!(!Navigation::available(&view, Command::ZoomIn));
        navigation.input(&mut view, Input::Command(Command::Fit));
        assert!(view.fitted());
        assert!(!Navigation::available(&view, Command::Fit));
        assert!(!Navigation::available(&view, Command::ZoomOut));
    }

    #[test]
    fn origin_change_finishes_pan_at_current_geographic_center_without_late_rollback() {
        let mut view = view();
        let mut navigation = Navigation::default();
        let start = view.offset;
        navigation.input(&mut view, Input::Press(start));
        navigation.input(
            &mut view,
            Input::Move {
                position: start + Vector2::new(30.0, 20.0),
                left_down: true,
            },
        );
        navigation.input(
            &mut view,
            Input::Captured {
                id: 1,
                success: true,
            },
        );
        let longitude = super::super::viewport::wrap(view.center.X + view.meridian, 0.0);
        let latitude = view.center.Y;
        let zoom = view.zoom;
        assert!(navigation.input(&mut view, Input::Rebase(139.7)) == Capture::Release);
        assert!(!navigation.panning());
        assert_eq!(view.zoom, zoom);
        assert_eq!(view.center.Y, latitude);
        assert!(
            (super::super::viewport::wrap(view.center.X + view.meridian, 0.0) - longitude).abs()
                < 0.0001
        );
        let rebased = view;
        navigation.input(&mut view, Input::Cancel);
        navigation.input(&mut view, Input::Release(start));
        assert!(view == rebased);
    }
}
