//! macOS platform scaffolding.
//!
//! This crate provides compile-safe placeholders for ScreenCaptureKit and
//! Quartz input integrations planned for later milestones.

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;

/// Detect monitors on macOS.
///
/// TODO(platform/macos): replace with CoreGraphics display enumeration.
pub fn detect_monitors() -> GrabmeResult<Vec<MonitorInfo>> {
    Err(GrabmeError::platform(
        "macOS monitor detection is not implemented yet",
    ))
}

/// Placeholder for future ScreenCaptureKit support details.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenCaptureKitSupport {
    pub available: bool,
}

/// Probe whether ScreenCaptureKit is available.
///
/// TODO(platform/macos): implement runtime capability detection.
pub fn probe_screencapturekit_support() -> ScreenCaptureKitSupport {
    ScreenCaptureKitSupport { available: false }
}
