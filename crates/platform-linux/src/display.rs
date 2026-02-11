//! Display/monitor detection and DPI handling.

use grabme_common::error::GrabmeResult;
use grabme_platform_core::{
    denormalize_coords as core_denormalize_coords, normalize_coords as core_normalize_coords,
    virtual_desktop_bounds as core_virtual_desktop_bounds, DisplayServer, MonitorInfo,
};
use std::process::Command;

/// Detect connected monitors.
pub fn detect_monitors() -> GrabmeResult<Vec<MonitorInfo>> {
    tracing::debug!("Detecting monitors");

    let server = detect_display_server();
    let monitors = match server {
        DisplayServer::Wayland => parse_wlr_randr_output().or_else(parse_xrandr_output),
        DisplayServer::X11 => parse_xrandr_output().or_else(parse_wlr_randr_output),
        _ => parse_xrandr_output().or_else(parse_wlr_randr_output),
    }
    .unwrap_or_else(default_monitor);

    Ok(monitors)
}

/// Compute virtual desktop bounds that include all connected monitors.
/// Returns `(min_x, min_y, width, height)` in physical pixels.
pub fn virtual_desktop_bounds(monitors: &[MonitorInfo]) -> (i32, i32, u32, u32) {
    core_virtual_desktop_bounds(monitors)
}

/// Normalize absolute pixel coordinates to [0.0, 1.0] range
/// for a given monitor.
pub fn normalize_coords(pixel_x: i32, pixel_y: i32, monitor: &MonitorInfo) -> (f64, f64) {
    core_normalize_coords(pixel_x, pixel_y, monitor)
}

/// Denormalize [0.0, 1.0] coordinates back to absolute pixels
/// for a given monitor (used during rendering).
pub fn denormalize_coords(norm_x: f64, norm_y: f64, width: u32, height: u32) -> (i32, i32) {
    core_denormalize_coords(norm_x, norm_y, width, height)
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

fn parse_xrandr_output() -> Option<Vec<MonitorInfo>> {
    let output = Command::new("xrandr").arg("--query").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut monitors = Vec::new();

    for line in stdout.lines() {
        if !line.contains(" connected") {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let primary = line.contains(" primary ");

        let geometry_token = line
            .split_whitespace()
            .find(|token| token.contains('x') && token.contains('+'));
        let Some(geometry) = geometry_token else {
            continue;
        };

        let Some((wh, xy)) = geometry.split_once('+') else {
            continue;
        };
        let Some((w_str, h_str)) = wh.split_once('x') else {
            continue;
        };
        let mut xy_parts = xy.split('+');
        let (Some(x_str), Some(y_str)) = (xy_parts.next(), xy_parts.next()) else {
            continue;
        };

        let (Ok(width), Ok(height), Ok(x), Ok(y)) = (
            w_str.parse::<u32>(),
            h_str.parse::<u32>(),
            x_str.parse::<i32>(),
            y_str.parse::<i32>(),
        ) else {
            continue;
        };

        let refresh_rate_hz = parse_xrandr_refresh_rate(line).unwrap_or(60);

        monitors.push(MonitorInfo {
            name: name.to_string(),
            width,
            height,
            x,
            y,
            scale_factor: 1.0,
            refresh_rate_hz,
            primary,
        });
    }

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

fn parse_wlr_randr_output() -> Option<Vec<MonitorInfo>> {
    let output = Command::new("wlr-randr").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut monitors = Vec::new();

    let mut current_name: Option<String> = None;
    let mut current_width: Option<u32> = None;
    let mut current_height: Option<u32> = None;
    let mut current_x: i32 = 0;
    let mut current_y: i32 = 0;
    let mut current_scale: f64 = 1.0;
    let mut current_refresh: u32 = 60;

    let flush_current = |name: &mut Option<String>,
                         width: &mut Option<u32>,
                         height: &mut Option<u32>,
                         x: i32,
                         y: i32,
                         scale: f64,
                         refresh: u32,
                         monitors: &mut Vec<MonitorInfo>| {
        if let (Some(name), Some(width), Some(height)) = (name.take(), width.take(), height.take())
        {
            monitors.push(MonitorInfo {
                name,
                width,
                height,
                x,
                y,
                scale_factor: scale,
                refresh_rate_hz: refresh,
                primary: monitors.is_empty(),
            });
        }
    };

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') {
            flush_current(
                &mut current_name,
                &mut current_width,
                &mut current_height,
                current_x,
                current_y,
                current_scale,
                current_refresh,
                &mut monitors,
            );

            current_name = Some(line.trim_end_matches(':').to_string());
            current_width = None;
            current_height = None;
            current_x = 0;
            current_y = 0;
            current_scale = 1.0;
            current_refresh = 60;
            continue;
        }

        if let Some(rest) = line.strip_prefix("current ") {
            if let Some((res, hz_part)) = rest.split_once(" @ ") {
                if let Some((w_str, h_str)) = res.split_once('x') {
                    current_width = w_str.parse::<u32>().ok();
                    current_height = h_str.parse::<u32>().ok();
                }
                if let Some(hz) = hz_part.split_whitespace().next() {
                    current_refresh = hz.parse::<f64>().map(|v| v.round() as u32).unwrap_or(60);
                }
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("Position:") {
            let mut parts = rest.trim().split(',');
            current_x = parts
                .next()
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(0);
            current_y = parts
                .next()
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(0);
            continue;
        }

        if let Some(rest) = line.strip_prefix("Scale:") {
            current_scale = rest.trim().parse::<f64>().unwrap_or(1.0);
        }
    }

    flush_current(
        &mut current_name,
        &mut current_width,
        &mut current_height,
        current_x,
        current_y,
        current_scale,
        current_refresh,
        &mut monitors,
    );

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

fn parse_xrandr_refresh_rate(line: &str) -> Option<u32> {
    let mut seen_geometry = false;
    for token in line.split_whitespace() {
        if !seen_geometry {
            if token.contains('x') && token.contains('+') {
                seen_geometry = true;
            }
            continue;
        }

        let cleaned = token.trim_end_matches('*').trim_end_matches('+');
        if let Ok(rate) = cleaned.parse::<f64>() {
            return Some(rate.round() as u32);
        }
    }
    None
}

fn default_monitor() -> Vec<MonitorInfo> {
    vec![MonitorInfo {
        name: "default".to_string(),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_hz: 60,
        primary: true,
    }]
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

    #[test]
    fn test_virtual_desktop_bounds_combines_monitors() {
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
                name: "center".to_string(),
                width: 2560,
                height: 1440,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_hz: 60,
                primary: true,
            },
        ];

        let (min_x, min_y, width, height) = virtual_desktop_bounds(&monitors);
        assert_eq!(min_x, -1920);
        assert_eq!(min_y, 0);
        assert_eq!(width, 4480);
        assert_eq!(height, 1440);
    }
}
