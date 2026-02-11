//! Frame compositor: combines video, cursor, webcam, and effects.
//!
//! This module defines the composition operations that will be
//! applied frame-by-frame during export rendering.

use grabme_processing_core::cursor_smooth::CursorSmoother;
use grabme_project_model::project::{WebcamConfig, WebcamCorner};
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
    webcam_config: Option<WebcamConfig>,
) -> Vec<FrameComposition> {
    let total_frames = (duration_secs * fps as f64).ceil() as u64;
    let frame_duration_ns = 1_000_000_000u64 / fps as u64;
    let mut compositions = Vec::with_capacity(total_frames as usize);
    let webcam_overlay = webcam_config
        .filter(|cfg| cfg.enabled)
        .map(|cfg| compute_webcam_overlay(cfg, output_width, output_height));

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
            let cursor_pos = CursorSmoother::position_at(smoothed_cursor, time_ns);
            cursor_pos.map(|pos| {
                let cx = pos.x;
                let cy = pos.y;
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
            webcam: webcam_overlay.clone(),
        });
    }

    compositions
}

fn compute_webcam_overlay(
    config: WebcamConfig,
    output_width: u32,
    output_height: u32,
) -> WebcamOverlay {
    let size_ratio = config.size_ratio.clamp(0.08, 0.50);
    let margin_ratio = config.margin_ratio.clamp(0.0, 0.20);

    let width = even_dimension(output_width as f64 * size_ratio) as f64;
    let height = even_dimension(output_height as f64 * size_ratio) as f64;
    let margin_x = (output_width as f64 * margin_ratio).round();
    let margin_y = (output_height as f64 * margin_ratio).round();

    let (x, y) = match config.corner {
        WebcamCorner::TopLeft => (margin_x, margin_y),
        WebcamCorner::TopRight => (output_width as f64 - width - margin_x, margin_y),
        WebcamCorner::BottomLeft => (margin_x, output_height as f64 - height - margin_y),
        WebcamCorner::BottomRight => (
            output_width as f64 - width - margin_x,
            output_height as f64 - height - margin_y,
        ),
    };

    WebcamOverlay {
        x: x.max(0.0),
        y: y.max(0.0),
        width,
        height,
    }
}

fn even_dimension(raw: f64) -> u32 {
    let mut value = raw.round() as u32;
    value = value.max(2);
    if value % 2 != 0 {
        value = value.saturating_sub(1).max(2);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_uses_interpolated_position() {
        let timeline = Timeline::new();
        let cursor = vec![(0u64, 0.0, 0.0), (1_000_000_000u64, 1.0, 1.0)];

        let frames = compute_compositions(&timeline, &cursor, 100, 100, 2, 1.0, None);
        assert_eq!(frames.len(), 2);

        let mid = frames[1].cursor.as_ref().unwrap();
        assert!((mid.x - 50.0).abs() < 0.001);
        assert!((mid.y - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_webcam_overlay_defaults_to_bottom_right() {
        let timeline = Timeline::new();
        let frames = compute_compositions(
            &timeline,
            &[],
            1920,
            1080,
            30,
            1.0,
            Some(WebcamConfig::default()),
        );

        let webcam = frames[0].webcam.as_ref().expect("webcam overlay present");
        assert!((webcam.width - 460.0).abs() < 1.0);
        assert!((webcam.height - 258.0).abs() < 1.0);
        assert!((webcam.x - 1402.0).abs() < 1.0);
        assert!((webcam.y - 790.0).abs() < 1.0);
    }

    #[test]
    fn test_webcam_overlay_honors_corner_setting() {
        let timeline = Timeline::new();
        let webcam_cfg = WebcamConfig {
            corner: WebcamCorner::TopLeft,
            ..WebcamConfig::default()
        };

        let frames = compute_compositions(&timeline, &[], 1280, 720, 30, 1.0, Some(webcam_cfg));
        let webcam = frames[0].webcam.as_ref().expect("webcam overlay present");

        assert!((webcam.x - 38.0).abs() < 1.0);
        assert!((webcam.y - 22.0).abs() < 1.0);
    }
}
