// Ported from TinyTorrent2 MapViewport.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use crate::geography::Point;
use windows_numerics::{Matrix3x2, Vector2};

pub const MAX_ZOOM: f32 = 128.0;

#[derive(Clone, Copy, PartialEq)]
pub struct Viewport {
    pub size: Vector2,
    pub scale: f32,
    pub offset: Vector2,
    pub meridian: f32,
    pub zoom: f32,
    pub ratio: f32,
    pub center: Vector2,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            size: Vector2::zero(),
            scale: 0.0,
            offset: Vector2::zero(),
            meridian: 0.0,
            zoom: 1.0,
            ratio: 1.0,
            center: Vector2::zero(),
        }
    }
}

impl Viewport {
    pub fn resize(&mut self, width: f64, height: f64) {
        self.size = Vector2::new(width as f32, height as f32);
        self.scale = ((width - 24.0) / 360.0)
            .min((height - 20.0) / 180.0)
            .max(0.0) as f32;
        self.offset = self.size / 2.0;
        self.set_view(self.zoom, self.center);
    }

    pub fn fitted(&self) -> bool {
        self.zoom == 1.0 && self.center == Vector2::zero()
    }

    pub fn geographic_center(&self) -> Point {
        Point {
            latitude: -f64::from(self.center.Y),
            longitude: f64::from(wrap(self.center.X + self.meridian, 0.0)),
        }
    }

    pub fn view_scale(&self) -> f32 {
        self.scale * self.zoom
    }

    pub fn axis_scale(&self) -> Vector2 {
        Vector2::new(self.ratio, 1.0) * self.view_scale()
    }

    pub fn transform(&self) -> Matrix3x2 {
        let scale = self.axis_scale();
        Matrix3x2::scale(scale.X, scale.Y)
            * Matrix3x2::translation(
                self.offset.X - self.center.X * scale.X,
                self.offset.Y - self.center.Y * scale.Y,
            )
    }

    pub fn coordinates(&self, point: Point) -> Vector2 {
        Vector2::new(
            wrap((point.longitude - f64::from(self.meridian)) as f32, 0.0),
            -point.latitude as f32,
        )
    }

    pub fn screen(&self, point: Point) -> Vector2 {
        let mut mapped = self.coordinates(point);
        mapped.X = wrap(mapped.X, self.center.X);
        self.to_screen(mapped)
    }

    pub fn to_screen(self, point: Vector2) -> Vector2 {
        let matrix = self.transform();
        Vector2::new(
            point.X * matrix.M11 + point.Y * matrix.M21 + matrix.M31,
            point.X * matrix.M12 + point.Y * matrix.M22 + matrix.M32,
        )
    }

    pub fn to_map(self, point: Vector2) -> Vector2 {
        (point - self.offset) / self.axis_scale() + self.center
    }

    pub fn rebase(&mut self, meridian: f32) {
        if meridian == self.meridian {
            return;
        }
        if !self.fitted() {
            self.center.X = wrap(self.center.X + self.meridian - meridian, 0.0);
        }
        self.meridian = meridian;
    }

    fn constrain(&self, zoom: f32, mut center: Vector2) -> Vector2 {
        let limit = if self.scale > 0.0 {
            (90.0 - self.offset.Y / (self.scale * zoom)).max(0.0)
        } else {
            0.0
        };
        center.X = wrap(center.X, 0.0);
        center.Y = center.Y.clamp(-limit, limit);
        center
    }

    fn set_view(&mut self, zoom: f32, center: Vector2) {
        self.center = self.constrain(zoom, center);
        self.zoom = zoom;
        self.ratio = f64::from(self.center.Y).to_radians().cos() as f32;
    }

    pub(super) fn restore(&mut self, center: Vector2, ratio: f32) {
        self.center = self.constrain(self.zoom, center);
        self.ratio = ratio;
    }

    pub fn zoom_at(&mut self, factor: f32, anchor: Vector2) {
        if self.scale <= 0.0 {
            return;
        }
        let zoom = (self.zoom * factor).clamp(1.0, MAX_ZOOM);
        if zoom != self.zoom {
            self.hold(zoom, self.to_map(anchor), anchor);
        }
    }

    pub fn hold(&mut self, zoom: f32, point: Vector2, anchor: Vector2) {
        if self.scale <= 0.0 {
            return;
        }
        let mut center = self.constrain(zoom, point - (anchor - self.offset) / (self.scale * zoom));
        let ratio = f64::from(center.Y).to_radians().cos() as f32;
        center.X = point.X - (anchor.X - self.offset.X) / (self.scale * zoom * ratio);
        self.set_view(zoom, center);
    }

    pub fn pan(&mut self, delta: Vector2) {
        if self.scale > 0.0 {
            self.hold(
                self.zoom,
                self.center - delta / self.axis_scale(),
                self.offset,
            );
        }
    }

    pub fn fit(&mut self) {
        self.set_view(1.0, Vector2::zero());
    }

    pub fn reveal(&mut self, destination: Point) {
        if self.scale <= 0.0 {
            return;
        }
        let point = self.screen(destination);
        let inset = Vector2::new(self.offset.X.min(12.0), self.offset.Y.min(12.0));
        let visible = Vector2::new(
            point.X.clamp(inset.X, 2.0 * self.offset.X - inset.X),
            point.Y.clamp(inset.Y, 2.0 * self.offset.Y - inset.Y),
        );
        if visible != point {
            self.hold(self.zoom, self.to_map(point), visible);
        }
    }
}

pub fn wrap(longitude: f32, center: f32) -> f32 {
    longitude + 360.0 * ((center - longitude) / 360.0).round_ties_even()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Viewport {
        let mut view = Viewport::default();
        view.resize(1280.0, 720.0);
        view
    }

    fn near(actual: Vector2, expected: Vector2) {
        assert!(
            (actual - expected).length_squared() < 0.0001,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn half_turns_follow_source_midpoint_to_even_wrapping() {
        for (input, expected) in [
            (180.0, 180.0),
            (-180.0, -180.0),
            (540.0, -180.0),
            (-540.0, 180.0),
        ] {
            assert_eq!(wrap(input, 0.0), expected);
        }
    }

    #[test]
    fn zoom_keeps_the_geographic_pointer_anchor_at_the_same_pixel() {
        let mut view = view();
        let anchor = Vector2::new(800.0, 300.0);
        let map_point = view.to_map(anchor);
        view.zoom_at(4.0, anchor);
        near(view.to_screen(map_point), anchor);
        near(view.to_map(anchor), map_point);
    }

    #[test]
    fn a_full_longitude_turn_returns_to_the_same_view() {
        let mut view = view();
        view.zoom_at(8.0, view.offset);
        let original = view.center;
        view.pan(Vector2::new(360.0 * view.axis_scale().X, 0.0));
        near(view.center, original);
    }

    #[test]
    fn changing_origin_keeps_nonfitted_geographic_center_and_zoom() {
        let mut view = view();
        view.rebase(139.7);
        view.zoom_at(16.0, Vector2::new(780.0, 270.0));
        let longitude = wrap(view.center.X + view.meridian, 0.0);
        let latitude = view.center.Y;
        view.rebase(-58.4);
        assert!((wrap(view.center.X + view.meridian, longitude) - longitude).abs() < 0.0001);
        assert_eq!(view.center.Y, latitude);
        assert_eq!(view.zoom, 16.0);
    }

    #[test]
    fn fitted_origin_changes_follow_the_new_meridian() {
        let mut view = view();
        view.rebase(139.7);
        assert!(view.fitted());
        near(
            view.screen(Point {
                longitude: f64::from(view.meridian),
                latitude: 0.0,
            }),
            view.offset,
        );
    }

    #[test]
    fn poles_stay_finite_and_zoom_has_the_source_limit() {
        let mut view = view();
        view.zoom_at(1000.0, view.offset);
        view.pan(Vector2::new(0.0, 1e9));
        assert_eq!(view.zoom, MAX_ZOOM);
        assert!(view.ratio > 0.0 && view.ratio.is_finite());
        assert!(view.axis_scale().X > 0.0);
        view.fit();
        assert!(view.fitted());
    }

    #[test]
    fn zero_size_zoom_does_not_create_invalid_coordinates() {
        let mut view = Viewport::default();
        view.resize(0.0, 0.0);
        view.zoom_at(2.0, Vector2::new(1.0, 1.0));
        assert_eq!(view.zoom, 1.0);
        assert!(view.center.X.is_finite() && view.center.Y.is_finite());
    }
}
