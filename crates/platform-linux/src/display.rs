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
///
/// Detection priority:
/// 1. `XDG_SESSION_TYPE` — the canonical session type set by the login manager.
///    This is the most reliable indicator.
/// 2. `WAYLAND_DISPLAY` — set when a Wayland compositor is running.
/// 3. `DISPLAY` — set when an X11 server is running.
///
/// Note: Some environments (e.g. running a Wayland-native app inside an X11
/// session via XWayland) may have both `WAYLAND_DISPLAY` and `DISPLAY` set.
/// `XDG_SESSION_TYPE` disambiguates this correctly.
pub fn detect_display_server() -> DisplayServer {
    // XDG_SESSION_TYPE is the most authoritative source
    match std::env::var("XDG_SESSION_TYPE")
        .as_deref()
        .map(str::to_lowercase)
        .as_deref()
    {
        Ok("wayland") => return DisplayServer::Wayland,
        Ok("x11" | "mir") => return DisplayServer::X11,
        _ => {}
    }

    // Fallback: heuristic based on which sockets are present.
    // Prefer Wayland when both are set (e.g. some hybrid setups).
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
    parse_xrandr_str(&stdout)
}

fn parse_xrandr_str(stdout: &str) -> Option<Vec<MonitorInfo>> {
    let mut monitors = Vec::new();
    let mut lines = stdout.lines().peekable();

    while let Some(line) = lines.next() {
        // Only process "connected" monitors (not "disconnected")
        if !line.contains(" connected") || line.contains(" disconnected") {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let primary = line.contains(" primary ");

        // xrandr geometry token: WxH+X+Y or (WxH+X+Y) for rotated monitors
        // The geometry token may appear after "connected" or after "primary".
        // We search all tokens for the WxH+X+Y pattern, which covers both
        // normal and rotated (inverted/left/right) modes.
        let geometry_token = line.split_whitespace().find(|token| {
            // Match tokens like 1920x1080+0+0 or 1080x1920+1920+0
            // Token must contain exactly one 'x' and at least two '+'
            let has_x = token.contains('x');
            let plus_count = token.chars().filter(|&c| c == '+').count();
            has_x && plus_count >= 2
        });

        let Some(geometry) = geometry_token else {
            // Monitor connected but no active mode (e.g. disabled) — skip
            tracing::debug!(
                monitor = name,
                "Skipping connected monitor with no active mode"
            );
            continue;
        };

        // Parse WxH+X+Y
        // Split on first '+' to separate dimensions from position
        let Some((wh, rest)) = geometry.split_once('+') else {
            continue;
        };
        let Some((w_str, h_str)) = wh.split_once('x') else {
            continue;
        };
        let mut pos_parts = rest.splitn(2, '+');
        let (Some(x_str), Some(y_str)) = (pos_parts.next(), pos_parts.next()) else {
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

        if width == 0 || height == 0 {
            tracing::debug!(
                monitor = name,
                width,
                height,
                "Skipping monitor with zero dimensions"
            );
            continue;
        }

        // Parse refresh rate from the mode lines that follow the header line.
        // Mode lines look like: "   1920x1080     60.00*+  50.00   ..."
        // The '*' marks the current mode, '+' marks the preferred mode.
        let refresh_rate_hz = parse_xrandr_current_refresh_rate(&mut lines).unwrap_or(60);

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

/// Parse the current refresh rate from xrandr mode lines following a monitor header.
///
/// Mode lines look like:
/// ```text
///    2560x1440    240.00*+  60.00 +  59.99 ...
///    1920x1080     60.00*+  50.00
/// ```
/// The `*` marks the currently active mode. The `+` marks the preferred mode.
/// These marker characters appear anywhere within the token (e.g. `240.00*+`),
/// so we strip all occurrences of `*` and `+` from the token before parsing,
/// rather than only from the edges.
fn parse_xrandr_current_refresh_rate<'a>(
    lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<u32> {
    while let Some(line) = lines.peek() {
        // Mode lines are indented; stop if we hit a non-indented line (next monitor header)
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        let line = lines.next().unwrap();
        // Look for a rate token containing '*' (active mode marker)
        for token in line.split_whitespace() {
            if token.contains('*') {
                // Remove ALL '*' and '+' characters from the token, not just from edges.
                // xrandr uses tokens like "240.00*+" where '*' is in the middle.
                let cleaned: String = token.chars().filter(|&c| c != '*' && c != '+').collect();
                if let Ok(rate) = cleaned.parse::<f64>() {
                    return Some(rate.round() as u32);
                }
            }
        }
    }
    None
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
    let mut current_enabled: bool = true; // assume enabled unless told otherwise

    let flush_current = |name: &mut Option<String>,
                         width: &mut Option<u32>,
                         height: &mut Option<u32>,
                         x: i32,
                         y: i32,
                         scale: f64,
                         refresh: u32,
                         enabled: bool,
                         monitors: &mut Vec<MonitorInfo>| {
        if let (Some(name), Some(width), Some(height)) = (name.take(), width.take(), height.take())
        {
            if !enabled {
                tracing::debug!(monitor = %name, "Skipping disabled monitor from wlr-randr");
                return;
            }
            if width == 0 || height == 0 {
                tracing::debug!(monitor = %name, "Skipping monitor with zero dimensions from wlr-randr");
                return;
            }
            // First non-disabled monitor is primary
            let primary = monitors.is_empty();
            monitors.push(MonitorInfo {
                name,
                width,
                height,
                x,
                y,
                scale_factor: scale,
                refresh_rate_hz: refresh,
                primary,
            });
        }
    };

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Non-indented lines are monitor name headers: "HDMI-A-1" or "HDMI-A-1 (HDMI-A-1)"
        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') {
            flush_current(
                &mut current_name,
                &mut current_width,
                &mut current_height,
                current_x,
                current_y,
                current_scale,
                current_refresh,
                current_enabled,
                &mut monitors,
            );

            // Strip trailing parenthetical and colon
            let name = line
                .split('(')
                .next()
                .unwrap_or(line)
                .trim_end_matches(':')
                .trim()
                .to_string();
            current_name = Some(name);
            current_width = None;
            current_height = None;
            current_x = 0;
            current_y = 0;
            current_scale = 1.0;
            current_refresh = 60;
            current_enabled = true;
            continue;
        }

        // "  Enabled: yes" / "  Enabled: no"
        if let Some(rest) = line.strip_prefix("Enabled:") {
            let val = rest.trim().to_lowercase();
            current_enabled = val == "yes" || val == "true" || val == "1";
            continue;
        }

        // "  current WxH @ Hz Hz" — the active mode
        if let Some(rest) = line.strip_prefix("current") {
            let rest = rest.trim();
            // Format: "WxH @ HZ Hz" or just "WxH @ HZ"
            if let Some((res, hz_part)) = rest.split_once(" @ ") {
                let res = res.trim();
                if let Some((w_str, h_str)) = res.split_once('x') {
                    current_width = w_str.trim().parse::<u32>().ok();
                    current_height = h_str.trim().parse::<u32>().ok();
                }
                if let Some(hz) = hz_part.split_whitespace().next() {
                    current_refresh = hz
                        .trim_end_matches("Hz")
                        .trim()
                        .parse::<f64>()
                        .map(|v| v.round() as u32)
                        .unwrap_or(60);
                }
            }
            continue;
        }

        // "  Position: X,Y"
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

        // "  Scale: 1.0"
        if let Some(rest) = line.strip_prefix("Scale:") {
            current_scale = rest.trim().parse::<f64>().unwrap_or(1.0);
        }
    }

    // Flush the last monitor
    flush_current(
        &mut current_name,
        &mut current_width,
        &mut current_height,
        current_x,
        current_y,
        current_scale,
        current_refresh,
        current_enabled,
        &mut monitors,
    );

    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
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

    /// Fixture that matches the real xrandr output format on the development machine:
    /// - eDP-2 at 2560x1440+1920+0 running at 240Hz
    /// - HDMI-1-0 at 1920x1080+0+0 running at 60Hz
    /// - Several disconnected ports (should be ignored)
    const XRANDR_FIXTURE: &str = "\
Screen 0: minimum 320 x 200, current 4480 x 1440, maximum 16384 x 16384
eDP-2 connected primary 2560x1440+1920+0 (normal left inverted right x axis y axis) 355mm x 200mm
   2560x1440    240.00*+  60.00 +  59.99    59.99
   1920x1080     60.01    59.97
HDMI-1-0 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080     60.00*+  75.00    50.00
   1280x720      60.00    50.00
DP-1-0 disconnected (normal left inverted right x axis y axis)
DP-1-1 disconnected (normal left inverted right x axis y axis)
";

    #[test]
    fn xrandr_parses_two_monitors_correct_geometry() {
        let monitors = parse_xrandr_str(XRANDR_FIXTURE).expect("should parse two monitors");
        assert_eq!(monitors.len(), 2, "expected 2 monitors, got {monitors:?}");

        let edp = &monitors[0];
        assert_eq!(edp.name, "eDP-2");
        assert_eq!(edp.width, 2560);
        assert_eq!(edp.height, 1440);
        assert_eq!(edp.x, 1920);
        assert_eq!(edp.y, 0);
        assert!(edp.primary);

        let hdmi = &monitors[1];
        assert_eq!(hdmi.name, "HDMI-1-0");
        assert_eq!(hdmi.width, 1920);
        assert_eq!(hdmi.height, 1080);
        assert_eq!(hdmi.x, 0);
        assert_eq!(hdmi.y, 0);
        assert!(!hdmi.primary);
    }

    #[test]
    fn xrandr_parses_refresh_rate_from_starred_token() {
        let monitors = parse_xrandr_str(XRANDR_FIXTURE).expect("should parse");
        // eDP-2 active mode is 240.00*+
        assert_eq!(
            monitors[0].refresh_rate_hz, 240,
            "eDP-2 refresh should be 240Hz"
        );
        // HDMI-1-0 active mode is 60.00*+
        assert_eq!(
            monitors[1].refresh_rate_hz, 60,
            "HDMI-1-0 refresh should be 60Hz"
        );
    }

    #[test]
    fn xrandr_skips_disconnected_monitors() {
        let monitors = parse_xrandr_str(XRANDR_FIXTURE).expect("should parse");
        let names: Vec<&str> = monitors.iter().map(|m| m.name.as_str()).collect();
        assert!(
            !names.contains(&"DP-1-0"),
            "disconnected monitor should be excluded"
        );
        assert!(
            !names.contains(&"DP-1-1"),
            "disconnected monitor should be excluded"
        );
    }

    #[test]
    fn xrandr_connected_no_mode_is_skipped() {
        // A monitor connected but with no active mode (no geometry token)
        let input = "\
Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384
HDMI-1 connected (normal left inverted right x axis y axis)
   1920x1080     60.00 +
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis)
   1920x1080     60.00*+
";
        let monitors = parse_xrandr_str(input).expect("should parse at least eDP-1");
        let names: Vec<&str> = monitors.iter().map(|m| m.name.as_str()).collect();
        // HDMI-1 has no geometry (not active) — should be skipped
        assert!(
            !names.contains(&"HDMI-1"),
            "inactive monitor should be excluded"
        );
        assert!(names.contains(&"eDP-1"), "active monitor must be present");
    }

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
