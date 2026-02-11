//! Windows platform scaffolding.
//!
//! This crate intentionally ships compile-safe placeholders so capture/input
//! backends can depend on stable interfaces before full implementation.

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;

/// Detect monitors on Windows.
///
/// TODO(platform/windows): replace with Win32 monitor enumeration.
pub fn detect_monitors() -> GrabmeResult<Vec<MonitorInfo>> {
    Err(GrabmeError::platform(
        "Windows monitor detection is not implemented yet",
    ))
}

/// Placeholder for future Windows Graphics Capture capabilities.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphicsCaptureSupport {
    pub available: bool,
}

/// Probe whether Windows Graphics Capture is available.
///
/// TODO(platform/windows): implement capability probing.
pub fn probe_graphics_capture_support() -> GraphicsCaptureSupport {
    GraphicsCaptureSupport { available: false }
}
