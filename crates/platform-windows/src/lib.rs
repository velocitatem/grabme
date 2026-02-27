//! Windows platform integration.

use std::process::Command;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawScreen {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "Width")]
    width: Option<u32>,
    #[serde(rename = "Height")]
    height: Option<u32>,
    #[serde(rename = "X")]
    x: Option<i32>,
    #[serde(rename = "Y")]
    y: Option<i32>,
    #[serde(rename = "Primary")]
    primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawScreens {
    One(RawScreen),
    Many(Vec<RawScreen>),
}

/// Detect monitors on Windows.
pub fn detect_monitors() -> GrabmeResult<Vec<MonitorInfo>> {
    let stdout = run_powershell_monitor_query().map_err(|e| {
        GrabmeError::platform(format!(
            "Failed to query Windows monitors via PowerShell: {e}"
        ))
    })?;

    let parsed: RawScreens = serde_json::from_str(&stdout).map_err(|e| {
        GrabmeError::platform(format!(
            "Failed to parse Windows monitor metadata from PowerShell: {e}"
        ))
    })?;

    let screens = match parsed {
        RawScreens::One(s) => vec![s],
        RawScreens::Many(v) => v,
    };

    let mut monitors: Vec<MonitorInfo> = screens
        .into_iter()
        .enumerate()
        .map(|(idx, s)| MonitorInfo {
            name: s.name.unwrap_or_else(|| format!("DISPLAY{}", idx + 1)),
            width: s.width.unwrap_or(1920).max(1),
            height: s.height.unwrap_or(1080).max(1),
            x: s.x.unwrap_or(0),
            y: s.y.unwrap_or(0),
            scale_factor: 1.0,
            refresh_rate_hz: 60,
            primary: s.primary.unwrap_or(idx == 0),
        })
        .collect();

    if monitors.is_empty() {
        return Err(GrabmeError::platform(
            "No displays reported by Windows monitor query",
        ));
    }

    if !monitors.iter().any(|m| m.primary) {
        monitors[0].primary = true;
    }

    Ok(monitors)
}

/// Runtime support hint for Windows Graphics Capture flow.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphicsCaptureSupport {
    pub available: bool,
}

pub fn probe_graphics_capture_support() -> GraphicsCaptureSupport {
    let available = run_powershell_monitor_query().is_ok();
    GraphicsCaptureSupport { available }
}

fn run_powershell_monitor_query() -> Result<String, String> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { [PSCustomObject]@{ Name = $_.DeviceName; Width = $_.Bounds.Width; Height = $_.Bounds.Height; X = $_.Bounds.X; Y = $_.Bounds.Y; Primary = $_.Primary } } | ConvertTo-Json -Compress";

    let shells = ["pwsh", "powershell"];
    let mut last_err = String::new();

    for shell in shells {
        let output = Command::new(shell)
            .args(["-NoProfile", "-Command", script])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !stdout.is_empty() {
                    return Ok(stdout);
                }
                last_err = format!("{shell} returned empty monitor list");
            }
            Ok(output) => {
                last_err = format!(
                    "{shell} exited with status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Err(e) => {
                last_err = format!("failed to launch {shell}: {e}");
            }
        }
    }

    Err(last_err)
}
