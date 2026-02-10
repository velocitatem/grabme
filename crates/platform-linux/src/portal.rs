//! XDG Desktop Portal integration for Wayland screen capture.
//!
//! On modern Linux (Wayland), screen capture must go through the
//! XDG Desktop Portal, which provides a user-consented, sandboxed
//! way to access screen content.
//!
//! # Flow
//!
//! 1. Connect to `org.freedesktop.portal.ScreenCast` via DBus
//! 2. Create a session
//! 3. Select sources (screen/window) with `cursor_mode = hidden`
//! 4. Start the stream → receive a PipeWire node ID
//! 5. Connect to PipeWire and receive video frames

use grabme_common::error::{GrabmeError, GrabmeResult};

/// Cursor mode for screen capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    /// Hide cursor from capture (recommended for GrabMe).
    Hidden,
    /// Show cursor embedded in the video stream.
    Embedded,
    /// Provide cursor as metadata (position + sprite, if supported).
    Metadata,
}

impl CursorMode {
    /// Convert to the portal's integer representation.
    pub fn to_portal_value(&self) -> u32 {
        match self {
            CursorMode::Hidden => 1,    // CURSOR_MODE_HIDDEN
            CursorMode::Embedded => 2,  // CURSOR_MODE_EMBEDDED
            CursorMode::Metadata => 4,  // CURSOR_MODE_METADATA
        }
    }
}

/// Source type for screen capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Monitor,
    Window,
}

impl SourceType {
    pub fn to_portal_value(&self) -> u32 {
        match self {
            SourceType::Monitor => 1,
            SourceType::Window => 2,
        }
    }
}

/// Result of a successful portal session setup.
#[derive(Debug, Clone)]
pub struct PortalSession {
    /// PipeWire node ID for the video stream.
    pub pipewire_node_id: u32,

    /// Stream dimensions.
    pub width: u32,
    pub height: u32,

    /// Portal session handle.
    pub session_handle: String,
}

/// Request a screen capture session through the XDG Desktop Portal.
pub async fn request_screencast(
    source_type: SourceType,
    cursor_mode: CursorMode,
) -> GrabmeResult<PortalSession> {
    tracing::info!(
        source = ?source_type,
        cursor = ?cursor_mode,
        "Requesting XDG ScreenCast session"
    );

    // TODO: Phase 1 implementation:
    // 1. Connect to DBus session bus
    // 2. Call org.freedesktop.portal.ScreenCast.CreateSession
    // 3. Call SelectSources with cursor_mode and source_type
    // 4. Call Start to get PipeWire node ID
    // 5. Return session info

    Err(GrabmeError::unsupported(
        "XDG Portal integration will be implemented in Phase 1",
    ))
}

/// Close an active portal session.
pub async fn close_session(session_handle: &str) -> GrabmeResult<()> {
    tracing::info!(handle = session_handle, "Closing portal session");

    // TODO: Close the DBus session
    Ok(())
}

/// Check if the XDG ScreenCast portal is available.
pub fn is_portal_available() -> bool {
    // Check if org.freedesktop.portal.Desktop is available on DBus
    // For now, check if we're running under Wayland
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
}
