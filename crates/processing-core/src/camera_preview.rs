//! Camera motion preview helpers.
//!
//! Generates CSS-like transform samples so UI clients can preview camera
//! movement without running the renderer.

use grabme_project_model::timeline::Timeline;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraMotionFrame {
    pub time_secs: f64,
    pub translate_x_percent: f64,
    pub translate_y_percent: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

impl CameraMotionFrame {
    pub fn css_transform(&self) -> String {
        format!(
            "translate({:.3}%, {:.3}%) scale({:.4}, {:.4})",
            self.translate_x_percent, self.translate_y_percent, self.scale_x, self.scale_y
        )
    }
}

/// Simulate frame-by-frame camera transforms from a timeline.
pub fn simulate_camera_motion(
    timeline: &Timeline,
    duration_secs: f64,
    sample_rate_fps: f64,
) -> Vec<CameraMotionFrame> {
    let sample_rate_fps = sample_rate_fps.max(1.0);
    let step = 1.0 / sample_rate_fps;
    let duration_secs = duration_secs.max(0.0);
    let mut t = 0.0;
    let mut frames = Vec::new();

    while t <= duration_secs + f64::EPSILON {
        let viewport = timeline.viewport_at(t);

        // CSS-like transform for a full-size source frame.
        let scale_x = 1.0 / viewport.w.max(0.01);
        let scale_y = 1.0 / viewport.h.max(0.01);
        let translate_x_percent = -viewport.x * 100.0;
        let translate_y_percent = -viewport.y * 100.0;

        frames.push(CameraMotionFrame {
            time_secs: t,
            translate_x_percent,
            translate_y_percent,
            scale_x,
            scale_y,
        });

        t += step;
    }

    frames
}

#[cfg(test)]
mod tests {
    use grabme_project_model::timeline::{CameraKeyframe, EasingFunction, KeyframeSource};
    use grabme_project_model::viewport::Viewport;

    use super::*;

    #[test]
    fn preview_generates_frames() {
        let mut timeline = Timeline::new();
        timeline.keyframes = vec![
            CameraKeyframe {
                time_secs: 0.0,
                viewport: Viewport::FULL,
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
            CameraKeyframe {
                time_secs: 2.0,
                viewport: Viewport::new(0.25, 0.25, 0.5, 0.5),
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
        ];

        let frames = simulate_camera_motion(&timeline, 2.0, 10.0);
        assert!(!frames.is_empty());
        assert!((frames[0].scale_x - 1.0).abs() < 1e-9);
        assert!(frames.last().unwrap().scale_x > 1.5);
    }

    #[test]
    fn css_transform_string_is_stable() {
        let frame = CameraMotionFrame {
            time_secs: 1.0,
            translate_x_percent: -12.345,
            translate_y_percent: -9.876,
            scale_x: 1.5,
            scale_y: 1.4,
        };
        let css = frame.css_transform();
        assert!(css.contains("translate(-12.345%, -9.876%)"));
        assert!(css.contains("scale(1.5000, 1.4000)"));
    }
}
