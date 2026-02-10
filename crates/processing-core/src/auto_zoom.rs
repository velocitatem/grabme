//! Auto-Zoom analysis: the "Heatmap-to-Viewport" algorithm.
//!
//! Analyzes mouse activity to automatically generate camera keyframes
//! that zoom into areas of interest and pan to follow the user's work.
//!
//! # Algorithm
//!
//! 1. **Chunk** the event timeline into configurable time windows (default 2s).
//! 2. **Centroid** calculation: average pointer position in each chunk.
//! 3. **Velocity** analysis: classify chunks as "hover" (zoom in) or "scan" (zoom out).
//! 4. **Keyframe** generation: create viewport keyframes from centroid + velocity data.
//! 5. **Smoothing** pass: apply moving average to prevent jerky camera motion.

use grabme_project_model::event::InputEvent;
use grabme_project_model::timeline::{CameraKeyframe, EasingFunction, KeyframeSource, Timeline};
use grabme_project_model::viewport::Viewport;

/// Configuration for the auto-zoom analyzer.
#[derive(Debug, Clone)]
pub struct AutoZoomConfig {
    /// Duration of each analysis chunk in seconds.
    pub chunk_duration_secs: f64,

    /// Minimum dwell time to trigger a zoom-in (seconds).
    pub dwell_threshold_secs: f64,

    /// Maximum pointer spread (normalized) to consider "hovering".
    /// If the pointer stays within this radius, it's a dwell.
    pub dwell_radius: f64,

    /// Zoom level when hovering (e.g., 0.4 = show 40% of screen).
    pub hover_zoom: f64,

    /// Zoom level when scanning (e.g., 0.8 = show 80% of screen).
    pub scan_zoom: f64,

    /// Smoothing window size for the camera path (number of keyframes).
    pub smoothing_window: usize,

    /// Minimum zoom level (viewport size). Prevents extreme zoom-in.
    pub min_viewport_size: f64,

    /// Maximum velocity considered "dwell" (normalized units per second).
    pub dwell_velocity_threshold: f64,

    /// Expected monitor count for ultra-wide or multi-monitor captures.
    /// Used with `focused_monitor_index` to keep analysis on one monitor region.
    pub monitor_count: usize,

    /// Zero-based monitor index to focus.
    pub focused_monitor_index: usize,
}

impl Default for AutoZoomConfig {
    fn default() -> Self {
        Self {
            chunk_duration_secs: 2.0,
            dwell_threshold_secs: 1.0,
            dwell_radius: 0.15,
            hover_zoom: 0.4,
            scan_zoom: 0.85,
            smoothing_window: 3,
            min_viewport_size: 0.25,
            dwell_velocity_threshold: 0.18,
            monitor_count: 1,
            focused_monitor_index: 0,
        }
    }
}

/// Analysis result for a single time chunk.
#[derive(Debug, Clone)]
pub struct ChunkAnalysis {
    /// Start time of the chunk (seconds).
    pub start_secs: f64,
    /// End time of the chunk (seconds).
    pub end_secs: f64,
    /// Average pointer position (centroid).
    pub centroid: (f64, f64),
    /// Maximum distance from centroid (spread).
    pub spread: f64,
    /// Average pointer velocity (normalized units per second).
    pub velocity: f64,
    /// Number of pointer samples in this chunk.
    pub sample_count: usize,
    /// Classification: is the user dwelling or scanning?
    pub activity: ActivityType,
}

/// Classification of user activity in a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityType {
    /// User is focused on a small area — zoom in.
    Dwell,
    /// User is moving across the screen — zoom out / pan.
    Scan,
    /// No pointer activity — hold previous state.
    Idle,
}

/// The auto-zoom analyzer.
pub struct AutoZoomAnalyzer {
    config: AutoZoomConfig,
}

impl AutoZoomAnalyzer {
    /// Create a new analyzer with the given configuration.
    pub fn new(config: AutoZoomConfig) -> Self {
        Self { config }
    }

    /// Create an analyzer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(AutoZoomConfig::default())
    }

    /// Analyze events and generate a timeline with camera keyframes.
    pub fn analyze(&self, events: &[InputEvent]) -> Timeline {
        let (timeline, _) = self.analyze_with_chunks(events);
        timeline
    }

    /// Analyze events and return both timeline and chunk diagnostics.
    pub fn analyze_with_chunks(&self, events: &[InputEvent]) -> (Timeline, Vec<ChunkAnalysis>) {
        let chunks = self.chunk_events(events);
        let raw_keyframes = self.generate_raw_keyframes(&chunks);
        let smoothed = self.smooth_keyframes(&raw_keyframes);

        let mut timeline = Timeline::new();
        timeline.keyframes = smoothed;

        (timeline, chunks)
    }

    /// Chunk events into time windows and compute per-chunk statistics.
    pub fn chunk_events(&self, events: &[InputEvent]) -> Vec<ChunkAnalysis> {
        if events.is_empty() {
            return vec![];
        }

        let focused_events = self.apply_focus_monitor_filter(events);
        if focused_events.is_empty() {
            return vec![];
        }

        let start_ns = focused_events[0].timestamp_ns;
        let end_ns = focused_events.last().unwrap().timestamp_ns;
        let chunk_ns = (self.config.chunk_duration_secs * 1e9) as u64;

        let mut chunks = vec![];
        let mut chunk_start = start_ns;

        while chunk_start < end_ns {
            let chunk_end = chunk_start + chunk_ns;

            // Collect pointer positions in this chunk
            let positions: Vec<(f64, f64)> = focused_events
                .iter()
                .filter(|e| e.timestamp_ns >= chunk_start && e.timestamp_ns < chunk_end)
                .filter_map(|e| e.pointer_position())
                .collect();

            let start_secs = (chunk_start - start_ns) as f64 / 1e9;
            let end_secs = (chunk_end - start_ns) as f64 / 1e9;

            if positions.is_empty() {
                chunks.push(ChunkAnalysis {
                    start_secs,
                    end_secs,
                    centroid: (0.5, 0.5),
                    spread: 0.0,
                    velocity: 0.0,
                    sample_count: 0,
                    activity: ActivityType::Idle,
                });
            } else {
                let centroid = Self::compute_centroid(&positions);
                let spread = Self::compute_spread(&positions, centroid);
                let velocity = Self::compute_velocity(&positions, self.config.chunk_duration_secs);

                let activity = if spread <= self.config.dwell_radius
                    && velocity <= self.config.dwell_velocity_threshold
                {
                    ActivityType::Dwell
                } else {
                    ActivityType::Scan
                };

                chunks.push(ChunkAnalysis {
                    start_secs,
                    end_secs,
                    centroid,
                    spread,
                    velocity,
                    sample_count: positions.len(),
                    activity,
                });
            }

            chunk_start = chunk_end;
        }

        chunks
    }

    /// Generate raw keyframes from chunk analysis.
    fn generate_raw_keyframes(&self, chunks: &[ChunkAnalysis]) -> Vec<CameraKeyframe> {
        let mut keyframes = vec![];
        let mut dwell_streak_secs = 0.0;

        // Always start with full viewport
        keyframes.push(CameraKeyframe {
            time_secs: 0.0,
            viewport: Viewport::FULL,
            easing: EasingFunction::EaseInOut,
            source: KeyframeSource::Auto,
        });

        for chunk in chunks {
            dwell_streak_secs = match chunk.activity {
                ActivityType::Dwell => dwell_streak_secs + (chunk.end_secs - chunk.start_secs),
                _ => 0.0,
            };

            let activity = if chunk.activity == ActivityType::Dwell
                && dwell_streak_secs >= self.config.dwell_threshold_secs
            {
                ActivityType::Dwell
            } else {
                chunk.activity
            };

            let viewport_size = match activity {
                ActivityType::Dwell => self.config.hover_zoom.max(self.config.min_viewport_size),
                ActivityType::Scan => self.config.scan_zoom,
                ActivityType::Idle => continue, // skip idle chunks
            };

            let viewport = Viewport::centered(
                chunk.centroid.0,
                chunk.centroid.1,
                viewport_size,
                viewport_size,
            );

            let keyframe = CameraKeyframe {
                time_secs: chunk.start_secs,
                viewport,
                easing: EasingFunction::EaseInOut,
                source: KeyframeSource::Auto,
            };

            if keyframes
                .last()
                .map(|existing| {
                    (existing.time_secs - keyframe.time_secs).abs() < 1e-6
                        && existing.viewport == keyframe.viewport
                })
                .unwrap_or(false)
            {
                continue;
            }

            keyframes.push(keyframe);
        }

        keyframes
    }

    /// Smooth keyframes using a moving average on viewport parameters.
    fn smooth_keyframes(&self, keyframes: &[CameraKeyframe]) -> Vec<CameraKeyframe> {
        if keyframes.len() <= 2 || self.config.smoothing_window <= 1 {
            return keyframes.to_vec();
        }

        let window = self.config.smoothing_window;
        let mut smoothed = Vec::with_capacity(keyframes.len());

        // Keep first keyframe as-is
        smoothed.push(keyframes[0].clone());

        for i in 1..keyframes.len() - 1 {
            let start = i.saturating_sub(window / 2);
            let end = (i + window / 2 + 1).min(keyframes.len());

            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut sum_w = 0.0;
            let mut sum_h = 0.0;
            let count = (end - start) as f64;

            for kf in keyframes.iter().take(end).skip(start) {
                sum_x += kf.viewport.x;
                sum_y += kf.viewport.y;
                sum_w += kf.viewport.w;
                sum_h += kf.viewport.h;
            }

            smoothed.push(CameraKeyframe {
                time_secs: keyframes[i].time_secs,
                viewport: Viewport::new(sum_x / count, sum_y / count, sum_w / count, sum_h / count),
                easing: keyframes[i].easing,
                source: KeyframeSource::Auto,
            });
        }

        // Keep last keyframe as-is
        if let Some(last) = keyframes.last() {
            smoothed.push(last.clone());
        }

        smoothed
    }

    /// Compute the centroid (average position) of a set of points.
    fn compute_centroid(positions: &[(f64, f64)]) -> (f64, f64) {
        let n = positions.len() as f64;
        let sum_x: f64 = positions.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = positions.iter().map(|(_, y)| y).sum();
        (sum_x / n, sum_y / n)
    }

    /// Compute the maximum spread (distance from centroid).
    fn compute_spread(positions: &[(f64, f64)], centroid: (f64, f64)) -> f64 {
        positions
            .iter()
            .map(|(x, y)| ((x - centroid.0).powi(2) + (y - centroid.1).powi(2)).sqrt())
            .fold(0.0_f64, f64::max)
    }

    /// Compute average velocity (normalized units per second).
    fn compute_velocity(positions: &[(f64, f64)], duration_secs: f64) -> f64 {
        if positions.len() < 2 || duration_secs <= 0.0 {
            return 0.0;
        }

        let total_distance: f64 = positions
            .windows(2)
            .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
            .sum();

        total_distance / duration_secs
    }

    fn apply_focus_monitor_filter<'a>(&self, events: &'a [InputEvent]) -> Vec<&'a InputEvent> {
        if self.config.monitor_count <= 1 {
            return events.iter().collect();
        }

        let monitor_count = self.config.monitor_count.max(1);
        let focused_monitor_index = self
            .config
            .focused_monitor_index
            .min(monitor_count.saturating_sub(1));

        let slot_width = 1.0 / monitor_count as f64;
        let min_x = focused_monitor_index as f64 * slot_width;
        let max_x = (focused_monitor_index + 1) as f64 * slot_width;

        events
            .iter()
            .filter(|event| {
                if let Some((x, _)) = event.pointer_position() {
                    x >= min_x && x <= max_x
                } else {
                    true
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pointer_events(positions: &[(u64, f64, f64)]) -> Vec<InputEvent> {
        positions
            .iter()
            .map(|(t, x, y)| InputEvent::pointer(*t, *x, *y))
            .collect()
    }

    #[test]
    fn test_empty_events() {
        let analyzer = AutoZoomAnalyzer::with_defaults();
        let timeline = analyzer.analyze(&[]);
        assert_eq!(timeline.keyframes.len(), 1); // just the default full viewport
    }

    #[test]
    fn test_dwell_detection() {
        // Mouse hovering in the top-left for 3 seconds
        let events = make_pointer_events(&[
            (0, 0.1, 0.1),
            (500_000_000, 0.11, 0.09),
            (1_000_000_000, 0.1, 0.1),
            (1_500_000_000, 0.12, 0.11),
            (2_000_000_000, 0.1, 0.1),
            (2_500_000_000, 0.11, 0.1),
            (3_000_000_000, 0.1, 0.11),
        ]);

        let analyzer = AutoZoomAnalyzer::with_defaults();
        let chunks = analyzer.chunk_events(&events);

        assert!(chunks.len() >= 1);
        assert_eq!(chunks[0].activity, ActivityType::Dwell);
    }

    #[test]
    fn test_scan_detection() {
        // Mouse moving across the screen
        let events = make_pointer_events(&[
            (0, 0.0, 0.0),
            (500_000_000, 0.25, 0.25),
            (1_000_000_000, 0.5, 0.5),
            (1_500_000_000, 0.75, 0.75),
            (2_000_000_000, 1.0, 1.0),
        ]);

        let analyzer = AutoZoomAnalyzer::with_defaults();
        let chunks = analyzer.chunk_events(&events);

        assert!(chunks.len() >= 1);
        assert_eq!(chunks[0].activity, ActivityType::Scan);
    }

    #[test]
    fn test_centroid_calculation() {
        let positions = vec![(0.0, 0.0), (1.0, 1.0)];
        let centroid = AutoZoomAnalyzer::compute_centroid(&positions);
        assert!((centroid.0 - 0.5).abs() < 1e-9);
        assert!((centroid.1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_keyframe_generation() {
        let events = make_pointer_events(&[
            (0, 0.1, 0.1),
            (500_000_000, 0.1, 0.1),
            (1_000_000_000, 0.1, 0.1),
            (2_000_000_000, 0.1, 0.1),
            (3_000_000_000, 0.9, 0.9),
            (3_500_000_000, 0.9, 0.9),
            (4_000_000_000, 0.9, 0.9),
        ]);

        let analyzer = AutoZoomAnalyzer::with_defaults();
        let timeline = analyzer.analyze(&events);

        // Should have keyframes
        assert!(timeline.keyframes.len() >= 2);
        // First should be full viewport
        assert_eq!(timeline.keyframes[0].viewport, Viewport::FULL);
    }

    #[test]
    fn test_smoothing_preserves_endpoints() {
        let analyzer = AutoZoomAnalyzer::with_defaults();
        let keyframes = vec![
            CameraKeyframe {
                time_secs: 0.0,
                viewport: Viewport::FULL,
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
            CameraKeyframe {
                time_secs: 2.0,
                viewport: Viewport::new(0.1, 0.1, 0.5, 0.5),
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
            CameraKeyframe {
                time_secs: 4.0,
                viewport: Viewport::new(0.3, 0.3, 0.4, 0.4),
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
            CameraKeyframe {
                time_secs: 6.0,
                viewport: Viewport::new(0.5, 0.5, 0.5, 0.5),
                easing: EasingFunction::Linear,
                source: KeyframeSource::Auto,
            },
        ];

        let smoothed = analyzer.smooth_keyframes(&keyframes);
        assert_eq!(smoothed.first().unwrap().viewport, Viewport::FULL);
        assert_eq!(
            smoothed.last().unwrap().viewport,
            keyframes.last().unwrap().viewport
        );
    }

    #[test]
    fn test_monitor_focus_filters_other_regions() {
        let events = make_pointer_events(&[
            (0, 0.1, 0.2),
            (500_000_000, 0.15, 0.25),
            (1_000_000_000, 0.75, 0.8),
            (1_500_000_000, 0.8, 0.82),
        ]);

        let analyzer = AutoZoomAnalyzer::new(AutoZoomConfig {
            monitor_count: 2,
            focused_monitor_index: 0,
            ..Default::default()
        });

        let chunks = analyzer.chunk_events(&events);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|chunk| chunk.centroid.0 <= 0.5));
    }

    #[test]
    fn test_dwell_velocity_threshold_prevents_false_zoom() {
        let events = make_pointer_events(&[
            (0, 0.0, 0.0),
            (500_000_000, 0.5, 0.5),
            (1_000_000_000, 1.0, 1.0),
        ]);

        let analyzer = AutoZoomAnalyzer::new(AutoZoomConfig {
            dwell_radius: 0.8,
            dwell_velocity_threshold: 0.05,
            ..Default::default()
        });

        let chunks = analyzer.chunk_events(&events);
        assert_eq!(chunks[0].activity, ActivityType::Scan);
    }
}
