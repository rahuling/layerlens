//! Maps world coordinates (mm) to screen pixels for the top-down view.
//!
//! The view is stateless apart from zoom and pan: each frame, the print's
//! bounding box is fitted to the drawing area, then zoom and pan are applied.

use eframe::egui::{Pos2, Rect, Vec2};

/// Share of the drawing area the print fills at zoom 1.0, leaving a margin.
const FIT_FRACTION: f32 = 0.9;
const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Zoom relative to the "fit" scale. 1.0 shows the whole print.
    pub zoom: f32,
    /// Pan offset in screen pixels from the centred view.
    pub pan: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

impl Viewport {
    /// Multiplies the zoom by `factor`, keeping it within sensible limits.
    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Builds the world-to-screen mapping for a print spanning `min..max` (in mm)
    /// drawn into `area`.
    pub fn transform(&self, min: [f32; 2], max: [f32; 2], area: Rect) -> Transform {
        let width = (max[0] - min[0]).max(1e-3);
        let height = (max[1] - min[1]).max(1e-3);
        let fit = (area.width() / width).min(area.height() / height) * FIT_FRACTION;
        Transform {
            world_center: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
            screen_center: area.center() + self.pan,
            scale: fit * self.zoom,
        }
    }
}

/// A fixed world-to-screen mapping for one frame. World y points up, screen y points down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    world_center: [f32; 2],
    screen_center: Pos2,
    /// Screen pixels per millimetre.
    scale: f32,
}

impl Transform {
    pub fn to_screen(self, point: [f32; 2]) -> Pos2 {
        Pos2::new(
            self.screen_center.x + (point[0] - self.world_center[0]) * self.scale,
            self.screen_center.y - (point[1] - self.world_center[1]) * self.scale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(900.0, 600.0))
    }

    #[test]
    fn centre_of_print_maps_to_centre_of_area() {
        let t = Viewport::default().transform([0.0, 0.0], [20.0, 20.0], area());
        let p = t.to_screen([10.0, 10.0]);
        assert!(
            (p.x - 450.0).abs() < 1e-3 && (p.y - 300.0).abs() < 1e-3,
            "{p:?}"
        );
    }

    #[test]
    fn print_fits_inside_area_with_margin() {
        let t = Viewport::default().transform([0.0, 0.0], [20.0, 20.0], area());
        // The limiting axis is height: 600 px * 0.9 over 20 mm.
        let top_left = t.to_screen([0.0, 20.0]);
        let bottom_right = t.to_screen([20.0, 0.0]);
        assert!((bottom_right.y - top_left.y - 540.0).abs() < 1e-3);
        assert!(top_left.y >= 30.0 - 1e-3 && bottom_right.y <= 570.0 + 1e-3);
    }

    #[test]
    fn y_axis_points_up_on_screen() {
        let t = Viewport::default().transform([0.0, 0.0], [20.0, 20.0], area());
        assert!(t.to_screen([0.0, 10.0]).y < t.to_screen([0.0, 0.0]).y);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut v = Viewport::default();
        v.zoom_by(1000.0);
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_by(0.0001);
        assert_eq!(v.zoom, MIN_ZOOM);
    }

    #[test]
    fn zoom_scales_around_the_centre() {
        let mut v = Viewport::default();
        v.zoom_by(2.0);
        let t = v.transform([0.0, 0.0], [20.0, 20.0], area());
        let p = t.to_screen([10.0, 10.0]);
        assert!((p.x - 450.0).abs() < 1e-3 && (p.y - 300.0).abs() < 1e-3);
    }
}
