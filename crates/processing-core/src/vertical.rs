//! Vertical / Social mode framing.
//!
//! Generates a 9:16 viewport that follows the cursor,
//! keeping it centered while maintaining stable framing.

use grabme_project_model::event::InputEvent;
use grabme_project_model::timeline::{CameraKeyframe, EasingFunction, KeyframeSource};
use grabme_project_model::viewport::Viewport;

/// Configuration for vertical mode processing.
#[derive(Debug, Clone)]
pub struct VerticalConfig {
    /// How tightly to follow the cursor (0 = static, 1 = instant tracking).
    pub tracking_responsiveness: f64,

    /// Size of the vertical viewport (height in normalized coords).
    /// Width is derived as height * 9/16.
    pub viewport_height: f64,

    /// Minimum cursor distance from viewport edge before triggering a pan.
    pub edge_threshold: f64,
}

impl Default for VerticalConfig {
    fn default() -> Self {
        Self {
            tracking_responsiveness: 0.15,
            viewport_height: 0.6,
            edge_threshold: 0.1,
        }
    }
}

/// Generate a vertical-mode timeline from pointer events.
pub fn generate_vertical_timeline(
    events: &[InputEvent],
    config: &VerticalConfig,
) -> Vec<CameraKeyframe> {
    let pointer_events: Vec<(f64, f64, f64)> = events
        .iter()
        .filter_map(|e| {
            e.pointer_position()
                .map(|(x, y)| (e.timestamp_secs(), x, y))
        })
        .collect();

    if pointer_events.is_empty() {
        return vec![CameraKeyframe {
            time_secs: 0.0,
            viewport: Viewport::vertical_centered(0.5, 0.5, config.viewport_height),
            easing: EasingFunction::EaseInOut,
            source: KeyframeSource::Auto,
        }];
    }

    let mut keyframes = vec![];
    let vp_width = config.viewport_height * 9.0 / 16.0;
    let mut cam_x = pointer_events[0].1;
    let mut cam_y = pointer_events[0].2;

    // Sample at ~2 Hz for keyframe generation
    let sample_interval_secs = 0.5;
    let mut next_sample = 0.0;

    for &(t, px, py) in &pointer_events {
        if t < next_sample {
            continue;
        }
        next_sample = t + sample_interval_secs;

        // Smoothly move camera toward cursor
        cam_x += (px - cam_x) * config.tracking_responsiveness;
        cam_y += (py - cam_y) * config.tracking_responsiveness;

        let viewport = Viewport::centered(cam_x, cam_y, vp_width, config.viewport_height);

        keyframes.push(CameraKeyframe {
            time_secs: t,
            viewport,
            easing: EasingFunction::EaseInOut,
            source: KeyframeSource::Auto,
        });
    }

    keyframes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertical_timeline_generation() {
        let events = vec![
            InputEvent::pointer(0, 0.5, 0.5),
            InputEvent::pointer(1_000_000_000, 0.3, 0.3),
            InputEvent::pointer(2_000_000_000, 0.7, 0.7),
        ];

        let config = VerticalConfig::default();
        let keyframes = generate_vertical_timeline(&events, &config);

        assert!(!keyframes.is_empty());

        // All viewports should have 9:16 aspect ratio
        for kf in &keyframes {
            let ratio = kf.viewport.w / kf.viewport.h;
            assert!(
                (ratio - 9.0 / 16.0).abs() < 0.05,
                "Aspect ratio {ratio} not close to 9:16"
            );
        }
    }

    #[test]
    fn test_empty_events_gives_centered() {
        let config = VerticalConfig::default();
        let keyframes = generate_vertical_timeline(&[], &config);
        assert_eq!(keyframes.len(), 1);
    }
}
