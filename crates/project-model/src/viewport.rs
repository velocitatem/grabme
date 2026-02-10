//! Viewport and region types for camera framing.
//!
//! All coordinates are normalized to `[0.0, 1.0]` range.

use serde::{Deserialize, Serialize};

/// A rectangular viewport within the capture region.
///
/// Coordinates are normalized: `(0.0, 0.0)` is top-left,
/// `(1.0, 1.0)` is bottom-right of the full capture area.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// Left edge (normalized).
    pub x: f64,
    /// Top edge (normalized).
    pub y: f64,
    /// Width (normalized).
    pub w: f64,
    /// Height (normalized).
    pub h: f64,
}

impl Viewport {
    /// Full-screen viewport (no zoom).
    pub const FULL: Viewport = Viewport {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    /// Create a new viewport, clamping values to valid range.
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            w: w.clamp(0.01, 1.0), // minimum 1% width
            h: h.clamp(0.01, 1.0), // minimum 1% height
        }
    }

    /// Create a viewport centered at `(cx, cy)` with given dimensions.
    /// Automatically clamps to stay within [0, 1] bounds.
    pub fn centered(cx: f64, cy: f64, w: f64, h: f64) -> Self {
        let w = w.clamp(0.01, 1.0);
        let h = h.clamp(0.01, 1.0);

        let x = (cx - w / 2.0).clamp(0.0, 1.0 - w);
        let y = (cy - h / 2.0).clamp(0.0, 1.0 - h);

        Self { x, y, w, h }
    }

    /// The center point of this viewport.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        (self.x + self.w).min(1.0)
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        (self.y + self.h).min(1.0)
    }

    /// Effective zoom factor (1.0 = no zoom, 2.0 = 200% zoom).
    pub fn zoom_factor(&self) -> f64 {
        1.0 / self.w.min(self.h)
    }

    /// Check if a normalized point is within this viewport.
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Linearly interpolate between two viewports.
    pub fn lerp(a: &Viewport, b: &Viewport, t: f64) -> Viewport {
        let t = t.clamp(0.0, 1.0);
        Viewport {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            w: a.w + (b.w - a.w) * t,
            h: a.h + (b.h - a.h) * t,
        }
    }

    /// Area of the viewport (0.0 to 1.0).
    pub fn area(&self) -> f64 {
        self.w * self.h
    }

    /// Convert a point from capture-space to viewport-local coordinates.
    /// Returns `None` if the point is outside the viewport.
    pub fn to_local(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        if !self.contains(px, py) {
            return None;
        }
        Some(((px - self.x) / self.w, (py - self.y) / self.h))
    }

    /// Create a 9:16 vertical viewport centered at the given point.
    pub fn vertical_centered(cx: f64, cy: f64, height: f64) -> Self {
        let w = height * 9.0 / 16.0;
        Self::centered(cx, cy, w, height)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::FULL
    }
}

/// A 2D normalized point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(&self, other: &Point2D) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Linear interpolation between two points.
    pub fn lerp(a: &Point2D, b: &Point2D, t: f64) -> Point2D {
        let t = t.clamp(0.0, 1.0);
        Point2D {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_viewport() {
        let vp = Viewport::FULL;
        assert_eq!(vp.zoom_factor(), 1.0);
        assert!(vp.contains(0.5, 0.5));
        assert!(vp.contains(0.0, 0.0));
        assert!(vp.contains(1.0, 1.0));
    }

    #[test]
    fn test_centered_viewport_clamps() {
        // Centered near edge should clamp
        let vp = Viewport::centered(0.1, 0.1, 0.5, 0.5);
        assert!(vp.x >= 0.0);
        assert!(vp.y >= 0.0);
        assert!(vp.right() <= 1.0);
        assert!(vp.bottom() <= 1.0);
    }

    #[test]
    fn test_zoom_factor() {
        let vp = Viewport::new(0.25, 0.25, 0.5, 0.5);
        assert!((vp.zoom_factor() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_lerp() {
        let a = Viewport::FULL;
        let b = Viewport::new(0.25, 0.25, 0.5, 0.5);
        let mid = Viewport::lerp(&a, &b, 0.5);
        assert!((mid.x - 0.125).abs() < 1e-9);
        assert!((mid.w - 0.75).abs() < 1e-9);
    }

    #[test]
    fn test_to_local() {
        let vp = Viewport::new(0.2, 0.3, 0.4, 0.4);
        let local = vp.to_local(0.4, 0.5).unwrap();
        assert!((local.0 - 0.5).abs() < 1e-9);
        assert!((local.1 - 0.5).abs() < 1e-9);

        assert!(vp.to_local(0.0, 0.0).is_none());
    }

    #[test]
    fn test_vertical_viewport() {
        let vp = Viewport::vertical_centered(0.5, 0.5, 0.8);
        let ratio = vp.w / vp.h;
        assert!((ratio - 9.0 / 16.0).abs() < 1e-9);
    }

    #[test]
    fn test_point2d_distance() {
        let a = Point2D::new(0.0, 0.0);
        let b = Point2D::new(1.0, 0.0);
        assert!((a.distance_to(&b) - 1.0).abs() < 1e-9);
    }
}
