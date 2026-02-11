//! Cursor motion smoothing algorithms.
//!
//! Transforms raw pointer data into smooth, professional-looking cursor paths.
//! Supports multiple algorithms selectable by the user.

use grabme_project_model::event::InputEvent;
use grabme_project_model::timeline::CursorConfig;
use grabme_project_model::viewport::Point2D;

/// Cursor smoothing engine.
pub struct CursorSmoother {
    algorithm: SmoothingAlgorithm,
}

/// Available smoothing algorithms.
#[derive(Debug, Clone, Copy)]
pub enum SmoothingAlgorithm {
    /// Exponential Moving Average with configurable strength.
    ///
    /// `strength` is in [0.0, 1.0], where larger values mean more smoothing.
    Ema { strength: f64 },

    /// Neighbor midpoint pull (preview-compatible "bezier" mode).
    ///
    /// `strength` is in [0.0, 1.0], where larger values mean more smoothing.
    Bezier { strength: f64 },

    /// 1D Kalman filter per axis (preview-compatible mode).
    ///
    /// `strength` is in [0.0, 1.0], where larger values mean more smoothing.
    Kalman { strength: f64 },

    /// Moving average over a window of N samples.
    MovingAverage { window: usize },

    /// No smoothing — pass through raw data.
    None,
}

impl CursorSmoother {
    /// Create a smoother with the given algorithm.
    pub fn new(algorithm: SmoothingAlgorithm) -> Self {
        Self { algorithm }
    }

    /// Create a smoother with sensible defaults (EMA, strength=0.3).
    pub fn default_ema() -> Self {
        Self::new(SmoothingAlgorithm::Ema { strength: 0.3 })
    }

    /// Build a smoothing algorithm from timeline cursor config.
    pub fn algorithm_from_cursor_config(config: &CursorConfig) -> SmoothingAlgorithm {
        let strength = clamp01(config.smoothing_factor);
        match config.smoothing {
            grabme_project_model::timeline::SmoothingAlgorithm::Ema => {
                SmoothingAlgorithm::Ema { strength }
            }
            grabme_project_model::timeline::SmoothingAlgorithm::Bezier => {
                SmoothingAlgorithm::Bezier { strength }
            }
            grabme_project_model::timeline::SmoothingAlgorithm::Kalman => {
                SmoothingAlgorithm::Kalman { strength }
            }
            grabme_project_model::timeline::SmoothingAlgorithm::None => SmoothingAlgorithm::None,
        }
    }

    /// Smooth a sequence of pointer positions extracted from events.
    ///
    /// Returns a vec of `(timestamp_ns, smoothed_x, smoothed_y)`.
    pub fn smooth(&self, events: &[InputEvent]) -> Vec<(u64, f64, f64)> {
        let raw: Vec<(u64, f64, f64)> = events
            .iter()
            .filter_map(|e| e.pointer_position().map(|(x, y)| (e.timestamp_ns, x, y)))
            .collect();

        match self.algorithm {
            SmoothingAlgorithm::Ema { strength } => self.smooth_ema(&raw, strength),
            SmoothingAlgorithm::Bezier { strength } => self.smooth_bezier(&raw, strength),
            SmoothingAlgorithm::Kalman { strength } => self.smooth_kalman(&raw, strength),
            SmoothingAlgorithm::MovingAverage { window } => {
                self.smooth_moving_average(&raw, window)
            }
            SmoothingAlgorithm::None => raw,
        }
    }

    /// Get a smoothed position at a specific time using interpolation.
    ///
    /// `smoothed_data` should come from `smooth()`.
    pub fn position_at(smoothed_data: &[(u64, f64, f64)], timestamp_ns: u64) -> Option<Point2D> {
        if smoothed_data.is_empty() {
            return None;
        }

        // Before first sample
        if timestamp_ns <= smoothed_data[0].0 {
            return Some(Point2D::new(smoothed_data[0].1, smoothed_data[0].2));
        }

        // After last sample
        if timestamp_ns >= smoothed_data.last().unwrap().0 {
            let last = smoothed_data.last().unwrap();
            return Some(Point2D::new(last.1, last.2));
        }

        // Binary search for surrounding samples
        let idx = smoothed_data
            .binary_search_by_key(&timestamp_ns, |(t, _, _)| *t)
            .unwrap_or_else(|i| i.saturating_sub(1));

        if idx + 1 >= smoothed_data.len() {
            let s = &smoothed_data[idx];
            return Some(Point2D::new(s.1, s.2));
        }

        let (t0, x0, y0) = smoothed_data[idx];
        let (t1, x1, y1) = smoothed_data[idx + 1];

        let duration = (t1 - t0) as f64;
        if duration < 1.0 {
            return Some(Point2D::new(x0, y0));
        }

        let t = (timestamp_ns - t0) as f64 / duration;
        Some(Point2D::new(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t))
    }

    /// EMA smoothing using preview-compatible strength semantics.
    ///
    /// `alpha = 1 - strength`, then `smoothed = alpha * current + (1 - alpha) * previous`.
    fn smooth_ema(&self, raw: &[(u64, f64, f64)], strength: f64) -> Vec<(u64, f64, f64)> {
        if raw.is_empty() {
            return vec![];
        }

        let alpha = clamp01(1.0 - strength);

        let mut result = Vec::with_capacity(raw.len());
        let mut prev_x = raw[0].1;
        let mut prev_y = raw[0].2;
        result.push(raw[0]);

        for &(t, x, y) in &raw[1..] {
            prev_x = alpha * x + (1.0 - alpha) * prev_x;
            prev_y = alpha * y + (1.0 - alpha) * prev_y;
            result.push((t, prev_x, prev_y));
        }

        result
    }

    /// Neighbor midpoint pull smoothing compatible with preview "bezier" mode.
    fn smooth_bezier(&self, raw: &[(u64, f64, f64)], strength: f64) -> Vec<(u64, f64, f64)> {
        if raw.len() < 3 {
            return raw.to_vec();
        }

        let pull = clamp01(strength);
        let mut result = Vec::with_capacity(raw.len());
        result.push(raw[0]);

        for i in 1..raw.len() - 1 {
            let prev = raw[i - 1];
            let curr = raw[i];
            let next = raw[i + 1];

            let cx = (prev.1 + next.1) * 0.5;
            let cy = (prev.2 + next.2) * 0.5;
            let x = curr.1 * (1.0 - pull) + cx * pull;
            let y = curr.2 * (1.0 - pull) + cy * pull;
            result.push((curr.0, x, y));
        }

        result.push(*raw.last().unwrap());
        result
    }

    /// Kalman smoothing compatible with preview "kalman" mode.
    fn smooth_kalman(&self, raw: &[(u64, f64, f64)], strength: f64) -> Vec<(u64, f64, f64)> {
        if raw.is_empty() {
            return vec![];
        }

        let strength = clamp01(strength);
        let q = 0.001 + (1.0 - strength) * 0.01;
        let r = 0.001 + strength * 0.04;

        let mut result = Vec::with_capacity(raw.len());
        let mut x = raw[0].1;
        let mut y = raw[0].2;
        let mut px = 1.0;
        let mut py = 1.0;

        for &(t, sx, sy) in raw {
            px += q;
            py += q;

            let kx = px / (px + r);
            let ky = py / (py + r);

            x = x + kx * (sx - x);
            y = y + ky * (sy - y);

            px = (1.0 - kx) * px;
            py = (1.0 - ky) * py;

            result.push((t, x, y));
        }

        result
    }

    /// Moving average smoothing over a window.
    fn smooth_moving_average(
        &self,
        raw: &[(u64, f64, f64)],
        window: usize,
    ) -> Vec<(u64, f64, f64)> {
        if raw.is_empty() || window == 0 {
            return raw.to_vec();
        }

        let mut result = Vec::with_capacity(raw.len());

        for i in 0..raw.len() {
            let start = i.saturating_sub(window / 2);
            let end = (i + window / 2 + 1).min(raw.len());
            let count = (end - start) as f64;

            let sum_x: f64 = raw[start..end].iter().map(|(_, x, _)| x).sum();
            let sum_y: f64 = raw[start..end].iter().map(|(_, _, y)| y).sum();

            result.push((raw[i].0, sum_x / count, sum_y / count));
        }

        result
    }
}

/// Generate a "cursor loop" path that smoothly returns the cursor
/// from its end position back to its start position.
///
/// Used for looping GIFs or seamless video loops.
pub fn generate_cursor_loop(
    smoothed: &[(u64, f64, f64)],
    loop_duration_ns: u64,
) -> Vec<(u64, f64, f64)> {
    if smoothed.is_empty() {
        return vec![];
    }

    let (_, start_x, start_y) = smoothed[0];
    let (end_t, end_x, end_y) = *smoothed.last().unwrap();

    // Generate interpolated path from end back to start
    let steps = 60; // 60 frames for 1 second at 60fps
    let step_ns = loop_duration_ns / steps;

    let mut loop_path = Vec::with_capacity(steps as usize);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        // Use ease-in-out for natural motion
        let eased = if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        };

        let x = end_x + (start_x - end_x) * eased;
        let y = end_y + (start_y - end_y) * eased;
        loop_path.push((end_t + i * step_ns, x, y));
    }

    loop_path
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jittery_events() -> Vec<InputEvent> {
        // Simulate jittery mouse at roughly (0.5, 0.5) with noise
        vec![
            InputEvent::pointer(0, 0.50, 0.50),
            InputEvent::pointer(16_000_000, 0.53, 0.48),
            InputEvent::pointer(32_000_000, 0.48, 0.52),
            InputEvent::pointer(48_000_000, 0.52, 0.49),
            InputEvent::pointer(64_000_000, 0.49, 0.51),
            InputEvent::pointer(80_000_000, 0.51, 0.50),
            InputEvent::pointer(96_000_000, 0.50, 0.50),
        ]
    }

    #[test]
    fn test_ema_reduces_jitter() {
        let events = make_jittery_events();
        let smoother = CursorSmoother::default_ema();
        let smoothed = smoother.smooth(&events);

        assert_eq!(smoothed.len(), events.len());

        // Smoothed values should be closer to the center (0.5, 0.5)
        // than the raw jittery values
        for &(_, x, y) in &smoothed[2..] {
            assert!((x - 0.5).abs() < 0.04, "Smoothed x={x} too far from center");
            assert!((y - 0.5).abs() < 0.04, "Smoothed y={y} too far from center");
        }
    }

    #[test]
    fn test_no_smoothing_passes_through() {
        let events = make_jittery_events();
        let smoother = CursorSmoother::new(SmoothingAlgorithm::None);
        let smoothed = smoother.smooth(&events);

        for (i, event) in events.iter().enumerate() {
            if let Some((x, y)) = event.pointer_position() {
                assert_eq!(smoothed[i].1, x);
                assert_eq!(smoothed[i].2, y);
            }
        }
    }

    #[test]
    fn test_position_at_interpolation() {
        let data = vec![(0u64, 0.0, 0.0), (1_000_000_000, 1.0, 1.0)];

        let mid = CursorSmoother::position_at(&data, 500_000_000).unwrap();
        assert!((mid.x - 0.5).abs() < 1e-9);
        assert!((mid.y - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_cursor_loop() {
        let smoothed = vec![
            (0u64, 0.0, 0.0),
            (1_000_000_000, 0.5, 0.5),
            (2_000_000_000, 1.0, 1.0),
        ];

        let loop_path = generate_cursor_loop(&smoothed, 1_000_000_000);
        assert!(!loop_path.is_empty());

        // First point should be near the end position
        let first = loop_path.first().unwrap();
        assert!((first.1 - 1.0).abs() < 1e-9);

        // Last point should be near the start position
        let last = loop_path.last().unwrap();
        assert!((last.1 - 0.0).abs() < 0.01);
        assert!((last.2 - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_moving_average() {
        let events = make_jittery_events();
        let smoother = CursorSmoother::new(SmoothingAlgorithm::MovingAverage { window: 3 });
        let smoothed = smoother.smooth(&events);
        assert_eq!(smoothed.len(), events.len());
    }

    #[test]
    fn test_bezier_keeps_endpoints() {
        let events = make_jittery_events();
        let smoother = CursorSmoother::new(SmoothingAlgorithm::Bezier { strength: 0.7 });
        let smoothed = smoother.smooth(&events);
        assert_eq!(smoothed.len(), events.len());
        assert_eq!(
            smoothed.first().unwrap().1,
            events[0].pointer_position().unwrap().0
        );
        assert_eq!(
            smoothed.last().unwrap().1,
            events.last().unwrap().pointer_position().unwrap().0
        );
    }

    #[test]
    fn test_kalman_returns_same_length() {
        let events = make_jittery_events();
        let smoother = CursorSmoother::new(SmoothingAlgorithm::Kalman { strength: 0.5 });
        let smoothed = smoother.smooth(&events);
        assert_eq!(smoothed.len(), events.len());
    }
}
