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

use ashpd::desktop::screencast::{
    CursorMode as AshCursorMode, Screencast, SourceType as AshSourceType,
};
use ashpd::desktop::PersistMode;
use ashpd::WindowIdentifier;
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
            CursorMode::Hidden => 1,   // CURSOR_MODE_HIDDEN
            CursorMode::Embedded => 2, // CURSOR_MODE_EMBEDDED
            CursorMode::Metadata => 4, // CURSOR_MODE_METADATA
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
    monitor_index: usize,
) -> GrabmeResult<PortalSession> {
    tracing::info!(
        source = ?source_type,
        cursor = ?cursor_mode,
        "Requesting XDG ScreenCast session"
    );

    let proxy = Screencast::new().await.map_err(|e| {
        GrabmeError::platform(format!("Failed to connect to XDG ScreenCast portal: {e}"))
    })?;

    let session = proxy
        .create_session()
        .await
        .map_err(|e| GrabmeError::platform(format!("Portal CreateSession failed: {e}")))?;

    proxy
        .select_sources(
            &session,
            map_cursor_mode(cursor_mode),
            map_source_type(source_type).into(),
            false,
            None,
            PersistMode::DoNot,
        )
        .await
        .map_err(|e| GrabmeError::platform(format!("Portal SelectSources failed: {e}")))?;

    let response = proxy
        .start(&session, &WindowIdentifier::default())
        .await
        .map_err(|e| GrabmeError::platform(format!("Portal Start failed: {e}")))?;

    let streams = response
        .response()
        .map_err(|e| GrabmeError::platform(format!("Portal start response failed: {e}")))?;

    let available_streams = streams.streams();
    let stream = available_streams
        .get(monitor_index)
        .or_else(|| available_streams.first())
        .ok_or_else(|| GrabmeError::platform("Portal returned no screencast streams"))?;

    let (width, height) = stream
        .size()
        .map(|(w, h)| (w as u32, h as u32))
        .unwrap_or((1920, 1080));

    Ok(PortalSession {
        pipewire_node_id: stream.pipe_wire_node_id(),
        width,
        height,
        session_handle: format!("{session:?}"),
    })
}

/// Close an active portal session.
pub async fn close_session(session_handle: &str) -> GrabmeResult<()> {
    tracing::info!(handle = session_handle, "Closing portal session");

    // Session lifecycle is managed by ashpd request objects and process scope.
    // If needed, explicit session management can be added later.
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

fn map_cursor_mode(mode: CursorMode) -> AshCursorMode {
    match mode {
        CursorMode::Hidden => AshCursorMode::Hidden,
        CursorMode::Embedded => AshCursorMode::Embedded,
        CursorMode::Metadata => AshCursorMode::Metadata,
    }
}

fn map_source_type(source_type: SourceType) -> AshSourceType {
    match source_type {
        SourceType::Monitor => AshSourceType::Monitor,
        SourceType::Window => AshSourceType::Window,
    }
}
