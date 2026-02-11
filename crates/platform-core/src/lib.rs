//! GrabMe platform core contracts.
//!
//! This crate contains cross-platform display/capture data structures used
//! by capture/input/render crates without coupling to a concrete OS backend.

use serde::{Deserialize, Serialize};

/// Information about a connected monitor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    /// Monitor name/identifier.
    pub name: String,
    /// Resolution in physical pixels.
    pub width: u32,
    pub height: u32,
    /// Position in the virtual desktop (pixels).
    pub x: i32,
    pub y: i32,
    /// Scale factor (for example 1.0, 1.25, 2.0).
    pub scale_factor: f64,
    /// Refresh rate in Hz.
    pub refresh_rate_hz: u32,
    /// Whether this monitor is primary.
    pub primary: bool,
}

impl MonitorInfo {
    /// Logical resolution (physical / scale).
    pub fn logical_width(&self) -> u32 {
        (self.width as f64 / self.scale_factor) as u32
    }

    /// Logical resolution (physical / scale).
    pub fn logical_height(&self) -> u32 {
        (self.height as f64 / self.scale_factor) as u32
    }
}

/// Display server / platform family used for capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DisplayServer {
    Wayland,
    X11,
    Windows,
    MacOS,
    #[default]
    Unknown,
}

/// Compute virtual desktop bounds that include all connected monitors.
/// Returns `(min_x, min_y, width, height)` in physical pixels.
pub fn virtual_desktop_bounds(monitors: &[MonitorInfo]) -> (i32, i32, u32, u32) {
    if monitors.is_empty() {
        return (0, 0, 1920, 1080);
    }

    let min_x = monitors.iter().map(|m| m.x).min().unwrap_or(0);
    let min_y = monitors.iter().map(|m| m.y).min().unwrap_or(0);
    let max_x = monitors
        .iter()
        .map(|m| m.x + m.width as i32)
        .max()
        .unwrap_or(1920);
    let max_y = monitors
        .iter()
        .map(|m| m.y + m.height as i32)
        .max()
        .unwrap_or(1080);

    let width = (max_x - min_x).max(1) as u32;
    let height = (max_y - min_y).max(1) as u32;
    (min_x, min_y, width, height)
}

/// Normalize absolute pixel coordinates to `[0.0, 1.0]` for a monitor.
pub fn normalize_coords(pixel_x: i32, pixel_y: i32, monitor: &MonitorInfo) -> (f64, f64) {
    let x = (pixel_x - monitor.x) as f64 / monitor.width.max(1) as f64;
    let y = (pixel_y - monitor.y) as f64 / monitor.height.max(1) as f64;
    (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
}

/// Denormalize `[0.0, 1.0]` coordinates back to absolute pixels.
pub fn denormalize_coords(norm_x: f64, norm_y: f64, width: u32, height: u32) -> (i32, i32) {
    let x = (norm_x.clamp(0.0, 1.0) * width.max(1) as f64) as i32;
    let y = (norm_y.clamp(0.0, 1.0) * height.max(1) as f64) as i32;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_bounds_cover_negative_origin_layout() {
        let monitors = vec![
            MonitorInfo {
                name: "left".to_string(),
                width: 1920,
                height: 1080,
                x: -1920,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_hz: 60,
                primary: false,
            },
            MonitorInfo {
                name: "main".to_string(),
                width: 2560,
                height: 1440,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_hz: 60,
                primary: true,
            },
        ];

        let (x, y, w, h) = virtual_desktop_bounds(&monitors);
        assert_eq!(x, -1920);
        assert_eq!(y, 0);
        assert_eq!(w, 4480);
        assert_eq!(h, 1440);
    }
}
