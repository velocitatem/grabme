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
///
/// The portal presents the user with a monitor/window selection dialog.
/// The `monitor_index` parameter is used only when multiple streams are
/// returned (e.g. when requesting multiple sources), but in practice the
/// portal returns exactly one stream per `select_sources` call and the
/// user has already made the selection — so we always use the first
/// returned stream to avoid mismatches.
pub async fn request_screencast(
    source_type: SourceType,
    cursor_mode: CursorMode,
    _monitor_index: usize,
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

    // `multiple = false` ensures the user selects exactly one source.
    // This is critical: if `multiple = true` the portal may return streams
    // in an arbitrary order that does not correspond to monitor_index.
    proxy
        .select_sources(
            &session,
            map_cursor_mode(cursor_mode),
            map_source_type(source_type).into(),
            false, // multiple = false: single source selection
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

    // Always use the first stream: the portal dialog is where the user
    // chooses the monitor/window. Indexing into streams by monitor_index
    // is wrong because the portal returns streams in portal-defined order,
    // not the OS monitor enumeration order.
    let stream = available_streams
        .first()
        .ok_or_else(|| GrabmeError::platform("Portal returned no screencast streams. User may have cancelled the dialog."))?;

    tracing::info!(
        node_id = stream.pipe_wire_node_id(),
        size = ?stream.size(),
        "Portal screencast stream selected"
    );

    let (width, height) = stream
        .size()
        .map(|(w, h)| (w as u32, h as u32))
        .unwrap_or((1920, 1080));

    if width == 0 || height == 0 {
        tracing::warn!(
            width,
            height,
            "Portal returned zero-size stream dimensions; will detect actual size from PipeWire"
        );
    }

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
