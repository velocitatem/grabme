//! Display/monitor detection and DPI handling.

use grabme_common::error::GrabmeResult;
use serde::{Deserialize, Serialize};

/// Information about a connected monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Monitor name/identifier.
    pub name: String,

    /// Resolution in physical pixels.
    pub width: u32,
    pub height: u32,

    /// Position in the virtual desktop (pixels).
    pub x: i32,
    pub y: i32,

    /// Scale factor (e.g., 1.0, 1.25, 2.0).
    pub scale_factor: f64,

    /// Refresh rate in Hz.
    pub refresh_rate_hz: u32,

    /// Whether this is the primary monitor.
    pub primary: bool,
}

impl MonitorInfo {
    /// Logical resolution (physical / scale).
    pub fn logical_width(&self) -> u32 {
        (self.width as f64 / self.scale_factor) as u32
    }

    pub fn logical_height(&self) -> u32 {
        (self.height as f64 / self.scale_factor) as u32
    }
}

/// Detect connected monitors.
pub fn detect_monitors() -> GrabmeResult<Vec<MonitorInfo>> {
    tracing::debug!("Detecting monitors");

    // TODO: Phase 1 implementation:
    // - On Wayland: query compositor via protocol
    // - On X11: use xrandr or X11 APIs
    // For now, return a reasonable default
    Ok(vec![MonitorInfo {
        name: "default".to_string(),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_hz: 60,
        primary: true,
    }])
}

/// Normalize absolute pixel coordinates to [0.0, 1.0] range
/// for a given monitor.
pub fn normalize_coords(pixel_x: i32, pixel_y: i32, monitor: &MonitorInfo) -> (f64, f64) {
    let x = (pixel_x - monitor.x) as f64 / monitor.width as f64;
    let y = (pixel_y - monitor.y) as f64 / monitor.height as f64;
    (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
}

/// Denormalize [0.0, 1.0] coordinates back to absolute pixels
/// for a given monitor (used during rendering).
pub fn denormalize_coords(norm_x: f64, norm_y: f64, width: u32, height: u32) -> (i32, i32) {
    let x = (norm_x * width as f64) as i32;
    let y = (norm_y * height as f64) as i32;
    (x, y)
}

/// Detect the current display server.
pub fn detect_display_server() -> DisplayServer {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else if std::env::var("DISPLAY").is_ok() {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}

/// Display server type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_coords() {
        let monitor = MonitorInfo {
            name: "test".to_string(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_hz: 60,
            primary: true,
        };

        let (nx, ny) = normalize_coords(960, 540, &monitor);
        assert!((nx - 0.5).abs() < 1e-3);
        assert!((ny - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_denormalize_coords() {
        let (px, py) = denormalize_coords(0.5, 0.5, 1920, 1080);
        assert_eq!(px, 960);
        assert_eq!(py, 540);
    }

    #[test]
    fn test_normalize_clamps() {
        let monitor = MonitorInfo {
            name: "test".to_string(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_hz: 60,
            primary: true,
        };

        let (nx, ny) = normalize_coords(-100, 2000, &monitor);
        assert_eq!(nx, 0.0);
        assert_eq!(ny, 1.0);
    }
}
