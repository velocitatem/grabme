//! Frame compositor: combines video, cursor, webcam, and effects.
//!
//! This module defines the composition operations that will be
//! applied frame-by-frame during export rendering.

use grabme_project_model::timeline::Timeline;
use grabme_project_model::viewport::Viewport;

/// A single frame's composition instructions.
#[derive(Debug, Clone)]
pub struct FrameComposition {
    /// Frame number.
    pub frame_index: u64,

    /// Time in seconds.
    pub time_secs: f64,

    /// The viewport to crop from the source video.
    pub viewport: Viewport,

    /// Cursor position in output coordinates.
    pub cursor: Option<CursorOverlay>,

    /// Webcam overlay position and size.
    pub webcam: Option<WebcamOverlay>,
}

/// Cursor rendering instruction for a single frame.
#[derive(Debug, Clone)]
pub struct CursorOverlay {
    /// X position in output pixel coordinates.
    pub x: f64,
    /// Y position in output pixel coordinates.
    pub y: f64,
    /// Scale factor for the cursor asset.
    pub scale: f64,
    /// Whether a click animation should be shown.
    pub clicking: bool,
}

/// Webcam overlay instruction for a single frame.
#[derive(Debug, Clone)]
pub struct WebcamOverlay {
    /// X position in output pixel coordinates.
    pub x: f64,
    /// Y position in output pixel coordinates.
    pub y: f64,
    /// Width in output pixels.
    pub width: f64,
    /// Height in output pixels.
    pub height: f64,
}

/// Compute the composition for each frame in the export.
pub fn compute_compositions(
    timeline: &Timeline,
    smoothed_cursor: &[(u64, f64, f64)],
    output_width: u32,
    output_height: u32,
    fps: u32,
    duration_secs: f64,
) -> Vec<FrameComposition> {
    let total_frames = (duration_secs * fps as f64).ceil() as u64;
    let frame_duration_ns = 1_000_000_000u64 / fps as u64;
    let mut compositions = Vec::with_capacity(total_frames as usize);

    for frame in 0..total_frames {
        let time_secs = frame as f64 / fps as f64;
        let time_ns = frame * frame_duration_ns;

        // Skip cut segments
        if timeline.is_cut(time_secs) {
            continue;
        }

        // Get viewport from timeline
        let viewport = timeline.viewport_at(time_secs);

        // Get cursor position
        let cursor = if !smoothed_cursor.is_empty() {
            let cursor_pos = find_closest_cursor(smoothed_cursor, time_ns);
            cursor_pos.map(|(cx, cy)| {
                // Transform cursor from capture coords to output coords
                let local = viewport.to_local(cx, cy);
                match local {
                    Some((lx, ly)) => CursorOverlay {
                        x: lx * output_width as f64,
                        y: ly * output_height as f64,
                        scale: viewport.zoom_factor(),
                        clicking: false, // TODO: check click events
                    },
                    None => CursorOverlay {
                        // Cursor outside viewport, clamp to edge
                        x: ((cx - viewport.x) / viewport.w).clamp(0.0, 1.0) * output_width as f64,
                        y: ((cy - viewport.y) / viewport.h).clamp(0.0, 1.0) * output_height as f64,
                        scale: viewport.zoom_factor(),
                        clicking: false,
                    },
                }
            })
        } else {
            None
        };

        compositions.push(FrameComposition {
            frame_index: frame,
            time_secs,
            viewport,
            cursor,
            webcam: None, // TODO: webcam positioning
        });
    }

    compositions
}

/// Find the closest cursor position to a given timestamp.
fn find_closest_cursor(data: &[(u64, f64, f64)], target_ns: u64) -> Option<(f64, f64)> {
    if data.is_empty() {
        return None;
    }

    let idx = data
        .binary_search_by_key(&target_ns, |(t, _, _)| *t)
        .unwrap_or_else(|i| i.min(data.len() - 1));

    Some((data[idx].1, data[idx].2))
}
