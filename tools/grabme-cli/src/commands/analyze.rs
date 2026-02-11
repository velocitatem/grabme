//! Run Auto-Director analysis on a project.

use std::path::PathBuf;

use grabme_processing_core::auto_zoom::{AutoZoomAnalyzer, AutoZoomConfig};
use grabme_processing_core::cursor_smooth::CursorSmoother;
use grabme_project_model::event::{
    parse_events, EventKind, EventStreamHeader, InputEvent, PointerCoordinateSpace,
};
use grabme_project_model::project::RecordingConfig;
use grabme_project_model::timeline::SmoothingAlgorithm as TimelineSmoothingAlgorithm;
use grabme_project_model::LoadedProject;

#[allow(clippy::too_many_arguments)]
pub fn run(
    path: PathBuf,
    chunk_secs: f64,
    vertical: bool,
    hover_zoom: f64,
    scan_zoom: f64,
    dwell_radius: f64,
    dwell_velocity: f64,
    smooth_window: usize,
    cursor_smoothing: String,
    cursor_smoothing_factor: f64,
    monitor_count: usize,
    focused_monitor: usize,
) -> anyhow::Result<()> {
    println!("Analyzing project at: {}", path.display());

    let mut project =
        LoadedProject::load(&path).map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    // Load events
    let events_path = path.join("meta").join("events.jsonl");
    let events_content = std::fs::read_to_string(&events_path)
        .map_err(|_| anyhow::anyhow!("Events file not found: {}", events_path.display()))?;

    let events_header = parse_events_header(&events_content);

    // Filter out header lines (starting with #)
    let event_lines: String = events_content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let events =
        parse_events(&event_lines).map_err(|e| anyhow::anyhow!("Failed to parse events: {e}"))?;

    let (analysis_events, projection_model) = project_events_to_capture_space(
        &events,
        events_header.as_ref(),
        &project.project.recording,
    );

    println!("  Loaded {} events", events.len());
    println!("  Pointer mapping: {}", projection_model.as_str());

    if analysis_events.is_empty() {
        println!("  No events to analyze.");
        return Ok(());
    }

    // Run cursor smoothing
    let mut cursor_config = project.timeline.cursor_config.clone();
    cursor_config.smoothing = parse_cursor_smoothing(&cursor_smoothing)?;
    cursor_config.smoothing_factor = cursor_smoothing_factor.clamp(0.0, 1.0);
    if let Some(effect_strength) =
        project
            .timeline
            .effects
            .iter()
            .rev()
            .find_map(|effect| match effect {
                grabme_project_model::timeline::Effect::CursorSmooth { strength } => {
                    Some(*strength)
                }
                _ => None,
            })
    {
        cursor_config.smoothing_factor = effect_strength.clamp(0.0, 1.0);
    }

    let smoothing = CursorSmoother::algorithm_from_cursor_config(&cursor_config);
    let smoother = CursorSmoother::new(smoothing);
    let smoothed = smoother.smooth(&analysis_events);
    println!("  Smoothed {} pointer positions", smoothed.len());

    // Run auto-zoom analysis
    if vertical {
        println!("  Running vertical (9:16) analysis...");
        let config = grabme_processing_core::vertical::VerticalConfig::default();
        let keyframes =
            grabme_processing_core::vertical::generate_vertical_timeline(&analysis_events, &config);
        project.timeline.keyframes = keyframes;
        println!(
            "  Generated {} vertical keyframes",
            project.timeline.keyframes.len()
        );
    } else {
        let effective_chunk_secs = adaptive_chunk_secs(chunk_secs, &analysis_events);
        println!("  Running auto-zoom analysis (chunk={effective_chunk_secs}s)...");
        let config = AutoZoomConfig {
            chunk_duration_secs: effective_chunk_secs,
            hover_zoom,
            scan_zoom,
            dwell_radius,
            dwell_velocity_threshold: dwell_velocity,
            smoothing_window: smooth_window,
            monitor_count,
            focused_monitor_index: focused_monitor,
            ..Default::default()
        };
        let analyzer = AutoZoomAnalyzer::new(config);
        let timeline = analyzer.analyze(&analysis_events);
        project.timeline.keyframes = timeline.keyframes;
        println!("  Generated {} keyframes", project.timeline.keyframes.len());
    }

    // Save updated timeline
    project.timeline.cursor_config = cursor_config;
    project
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save timeline: {e}"))?;

    println!(
        "  Timeline saved to: {}",
        path.join("meta/timeline.json").display()
    );
    println!("\nAnalysis complete.");

    Ok(())
}

fn adaptive_chunk_secs(requested_secs: f64, events: &[InputEvent]) -> f64 {
    let requested = requested_secs.max(0.25);
    if events.len() < 2 {
        return requested;
    }

    let start = events.first().map(|e| e.timestamp_ns).unwrap_or(0);
    let end = events.last().map(|e| e.timestamp_ns).unwrap_or(start);
    if end <= start {
        return requested;
    }

    let duration_secs = (end - start) as f64 / 1_000_000_000.0;
    let adaptive_target = (duration_secs / 10.0).max(0.35);
    requested.min(adaptive_target).max(0.25)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisPointerModel {
    CaptureNormalized,
    VirtualDesktopNormalized,
    VirtualDesktopRootOrigin,
}

impl AnalysisPointerModel {
    fn as_str(self) -> &'static str {
        match self {
            AnalysisPointerModel::CaptureNormalized => "capture_normalized",
            AnalysisPointerModel::VirtualDesktopNormalized => {
                "virtual_desktop_normalized -> capture_normalized"
            }
            AnalysisPointerModel::VirtualDesktopRootOrigin => {
                "virtual_desktop_root_origin -> capture_normalized"
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProjectionCandidate {
    model: AnalysisPointerModel,
    transform: PointerTransform,
}

#[derive(Debug, Clone, Copy)]
struct PointerTransform {
    scale_x: f64,
    scale_y: f64,
    tx: f64,
    ty: f64,
}

impl PointerTransform {
    fn identity() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    fn from_affine(scale_x: f64, scale_y: f64, tx: f64, ty: f64) -> Self {
        Self {
            scale_x,
            scale_y,
            tx,
            ty,
        }
    }

    fn project(self, x: f64, y: f64) -> (f64, f64) {
        (self.scale_x * x + self.tx, self.scale_y * y + self.ty)
    }
}

fn parse_events_header(events_content: &str) -> Option<EventStreamHeader> {
    let header_line = events_content
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('#'))?;
    let json = header_line.trim_start_matches('#').trim();
    serde_json::from_str::<EventStreamHeader>(json).ok()
}

fn project_events_to_capture_space(
    events: &[InputEvent],
    events_header: Option<&EventStreamHeader>,
    recording: &RecordingConfig,
) -> (Vec<InputEvent>, AnalysisPointerModel) {
    const EXPLICIT_PROJECTION_FALLBACK_DELTA: f64 = 0.35;

    if events.is_empty() {
        return (Vec::new(), AnalysisPointerModel::CaptureNormalized);
    }

    let explicit_space = events_header
        .map(|header| header.pointer_coordinate_space)
        .filter(|space| *space != PointerCoordinateSpace::LegacyUnspecified)
        .or_else(|| {
            let space = recording.pointer_coordinate_space;
            if space == PointerCoordinateSpace::LegacyUnspecified {
                None
            } else {
                Some(space)
            }
        });

    let best_fit = select_best_projection_candidate(events, recording);

    let selected = if let Some(space) = explicit_space {
        if let Some(explicit) = projection_candidate_for_space(space, recording) {
            let explicit_score = score_projection_candidate(explicit, events);
            let best_fit_score = score_projection_candidate(best_fit, events);
            if best_fit_score > explicit_score + EXPLICIT_PROJECTION_FALLBACK_DELTA {
                best_fit
            } else {
                explicit
            }
        } else {
            best_fit
        }
    } else {
        best_fit
    };

    let projected_events = events
        .iter()
        .map(|event| project_event(event, selected.transform))
        .collect();

    (projected_events, selected.model)
}

fn projection_candidate_for_space(
    space: PointerCoordinateSpace,
    recording: &RecordingConfig,
) -> Option<ProjectionCandidate> {
    match space {
        PointerCoordinateSpace::CaptureNormalized => Some(ProjectionCandidate {
            model: AnalysisPointerModel::CaptureNormalized,
            transform: PointerTransform::identity(),
        }),
        PointerCoordinateSpace::VirtualDesktopNormalized => {
            virtual_desktop_projection_candidates(recording)
                .into_iter()
                .find(|candidate| candidate.model == AnalysisPointerModel::VirtualDesktopNormalized)
        }
        PointerCoordinateSpace::VirtualDesktopRootOrigin => {
            virtual_desktop_projection_candidates(recording)
                .into_iter()
                .find(|candidate| candidate.model == AnalysisPointerModel::VirtualDesktopRootOrigin)
        }
        PointerCoordinateSpace::LegacyUnspecified => None,
    }
}

fn select_best_projection_candidate(
    events: &[InputEvent],
    recording: &RecordingConfig,
) -> ProjectionCandidate {
    let capture_candidate = ProjectionCandidate {
        model: AnalysisPointerModel::CaptureNormalized,
        transform: PointerTransform::identity(),
    };

    let mut candidates = vec![capture_candidate];
    candidates.extend(virtual_desktop_projection_candidates(recording));

    let mut best = capture_candidate;
    let mut best_score = score_projection_candidate(capture_candidate, events);
    for candidate in candidates.into_iter().skip(1) {
        let score = score_projection_candidate(candidate, events);
        if score > best_score {
            best = candidate;
            best_score = score;
        }
    }

    best
}

fn virtual_desktop_projection_candidates(recording: &RecordingConfig) -> Vec<ProjectionCandidate> {
    let monitor_w = recording.monitor_width as f64;
    let monitor_h = recording.monitor_height as f64;
    let virtual_w = recording.virtual_width as f64;
    let virtual_h = recording.virtual_height as f64;
    if monitor_w <= 0.0 || monitor_h <= 0.0 || virtual_w <= 0.0 || virtual_h <= 0.0 {
        return vec![];
    }

    let scale_x = virtual_w / monitor_w;
    let scale_y = virtual_h / monitor_h;
    let tx_bounds = (recording.virtual_x as f64 - recording.monitor_x as f64) / monitor_w;
    let ty_bounds = (recording.virtual_y as f64 - recording.monitor_y as f64) / monitor_h;
    let tx_root = -(recording.monitor_x as f64) / monitor_w;
    let ty_root = -(recording.monitor_y as f64) / monitor_h;

    let bounds_candidate = ProjectionCandidate {
        model: AnalysisPointerModel::VirtualDesktopNormalized,
        transform: PointerTransform::from_affine(scale_x, scale_y, tx_bounds, ty_bounds),
    };

    if (tx_bounds - tx_root).abs() < 1e-9 && (ty_bounds - ty_root).abs() < 1e-9 {
        return vec![bounds_candidate];
    }

    vec![
        bounds_candidate,
        ProjectionCandidate {
            model: AnalysisPointerModel::VirtualDesktopRootOrigin,
            transform: PointerTransform::from_affine(scale_x, scale_y, tx_root, ty_root),
        },
    ]
}

fn score_projection_candidate(candidate: ProjectionCandidate, events: &[InputEvent]) -> f64 {
    let sample_stride = ((events.len() as f64) / 1024.0).ceil() as usize;
    let sample_stride = sample_stride.max(1);

    let mut sampled = 0usize;
    let mut in_bounds = 0usize;
    let mut near_border = 0usize;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (idx, event) in events.iter().enumerate() {
        if idx % sample_stride != 0 {
            continue;
        }
        let Some((x, y)) = event.pointer_position() else {
            continue;
        };

        let (px, py) = candidate.transform.project(x, y);
        if !px.is_finite() || !py.is_finite() {
            continue;
        }

        sampled += 1;
        if (0.0..=1.0).contains(&px) && (0.0..=1.0).contains(&py) {
            in_bounds += 1;
            min_x = min_x.min(px);
            max_x = max_x.max(px);
            min_y = min_y.min(py);
            max_y = max_y.max(py);
            if px <= 0.01 || px >= 0.99 || py <= 0.01 || py >= 0.99 {
                near_border += 1;
            }
        }
    }

    if sampled == 0 {
        return -1.0;
    }

    let in_bounds_ratio = in_bounds as f64 / sampled as f64;
    let span_x = if min_x.is_finite() && max_x.is_finite() {
        (max_x - min_x).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let span_y = if min_y.is_finite() && max_y.is_finite() {
        (max_y - min_y).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let span_score = (span_x + span_y).clamp(0.0, 1.5);
    let border_ratio = if in_bounds > 0 {
        near_border as f64 / in_bounds as f64
    } else {
        1.0
    };

    in_bounds_ratio * 4.0 + span_score - border_ratio * 0.75
}

fn project_event(event: &InputEvent, transform: PointerTransform) -> InputEvent {
    let kind = match &event.kind {
        EventKind::Pointer { x, y } => {
            let (px, py) = project_pointer_xy(*x, *y, transform);
            EventKind::Pointer { x: px, y: py }
        }
        EventKind::Click {
            button,
            state,
            x,
            y,
        } => {
            let (px, py) = project_pointer_xy(*x, *y, transform);
            EventKind::Click {
                button: *button,
                state: *state,
                x: px,
                y: py,
            }
        }
        EventKind::Scroll { dx, dy, x, y } => {
            let (px, py) = project_pointer_xy(*x, *y, transform);
            EventKind::Scroll {
                dx: *dx,
                dy: *dy,
                x: px,
                y: py,
            }
        }
        other => other.clone(),
    };

    InputEvent {
        timestamp_ns: event.timestamp_ns,
        kind,
    }
}

fn project_pointer_xy(x: f64, y: f64, transform: PointerTransform) -> (f64, f64) {
    let (px, py) = transform.project(x, y);
    (px.clamp(0.0, 1.0), py.clamp(0.0, 1.0))
}

fn parse_cursor_smoothing(raw: &str) -> anyhow::Result<TimelineSmoothingAlgorithm> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "ema" => Ok(TimelineSmoothingAlgorithm::Ema),
        "bezier" => Ok(TimelineSmoothingAlgorithm::Bezier),
        "kalman" => Ok(TimelineSmoothingAlgorithm::Kalman),
        "none" => Ok(TimelineSmoothingAlgorithm::None),
        other => Err(anyhow::anyhow!(
            "Invalid cursor smoothing algorithm: {other}. Use one of: ema, bezier, kalman, none"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grabme_project_model::project::Project;

    #[test]
    fn test_adaptive_chunk_secs_short_recording_uses_finer_chunks() {
        let events = vec![
            InputEvent::pointer(0, 0.1, 0.1),
            InputEvent::pointer(4_000_000_000, 0.2, 0.2),
        ];
        let chunk = adaptive_chunk_secs(2.0, &events);
        assert!((chunk - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_adaptive_chunk_secs_long_recording_preserves_requested_chunk() {
        let events = vec![
            InputEvent::pointer(0, 0.1, 0.1),
            InputEvent::pointer(60_000_000_000, 0.2, 0.2),
        ];
        let chunk = adaptive_chunk_secs(2.0, &events);
        assert!((chunk - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_project_events_to_capture_space_uses_virtual_bounds_mapping() {
        let mut project = Project::new("test", 1920, 1080, 60);
        project.recording.monitor_x = 0;
        project.recording.monitor_y = 0;
        project.recording.monitor_width = 1920;
        project.recording.monitor_height = 1080;
        project.recording.virtual_x = 0;
        project.recording.virtual_y = 0;
        project.recording.virtual_width = 4480;
        project.recording.virtual_height = 1440;
        project.recording.pointer_coordinate_space =
            PointerCoordinateSpace::VirtualDesktopNormalized;

        let events = vec![InputEvent::pointer(0, 0.25, 0.5)];
        let (mapped, model) = project_events_to_capture_space(&events, None, &project.recording);

        assert_eq!(model, AnalysisPointerModel::VirtualDesktopNormalized);
        let (x, y) = mapped[0].pointer_position().unwrap();
        assert!((x - 0.583333).abs() < 1e-5);
        assert!((y - 0.666666).abs() < 1e-5);
    }

    #[test]
    fn test_project_events_to_capture_space_honors_explicit_root_origin_header() {
        let mut project = Project::new("test", 2560, 1440, 60);
        project.recording.monitor_x = -1920;
        project.recording.monitor_y = 0;
        project.recording.monitor_width = 2560;
        project.recording.monitor_height = 1440;
        project.recording.virtual_x = -1920;
        project.recording.virtual_y = 0;
        project.recording.virtual_width = 4480;
        project.recording.virtual_height = 1440;
        project.recording.pointer_coordinate_space = PointerCoordinateSpace::LegacyUnspecified;

        let events = vec![
            InputEvent::pointer(0, 0.1429, 0.3000),
            InputEvent::pointer(16_000_000, 0.2143, 0.3200),
            InputEvent::pointer(32_000_000, 0.2679, 0.3500),
        ];

        let header = EventStreamHeader {
            schema_version: "1.0".to_string(),
            epoch_monotonic_ns: 0,
            epoch_wall: "2026-01-01T00:00:00Z".to_string(),
            capture_width: 2560,
            capture_height: 1440,
            scale_factor: 1.0,
            pointer_sample_rate_hz: 60,
            pointer_coordinate_space: PointerCoordinateSpace::VirtualDesktopRootOrigin,
        };

        let (_mapped, model) =
            project_events_to_capture_space(&events, Some(&header), &project.recording);
        assert_eq!(model, AnalysisPointerModel::VirtualDesktopRootOrigin);
    }

    #[test]
    fn test_project_events_to_capture_space_falls_back_when_explicit_mapping_is_inconsistent() {
        let mut project = Project::new("test", 1920, 1080, 60);
        project.recording.monitor_x = 0;
        project.recording.monitor_y = 0;
        project.recording.monitor_width = 1920;
        project.recording.monitor_height = 1080;
        project.recording.virtual_x = 0;
        project.recording.virtual_y = 0;
        project.recording.virtual_width = 4480;
        project.recording.virtual_height = 1440;
        project.recording.pointer_coordinate_space =
            PointerCoordinateSpace::VirtualDesktopNormalized;

        // These coordinates resemble a stream normalized against a narrower width,
        // where forcing virtual_desktop projection would push many points out of bounds.
        let events = vec![
            InputEvent::pointer(0, 0.38, 0.45),
            InputEvent::pointer(16_000_000, 0.44, 0.47),
            InputEvent::pointer(32_000_000, 0.52, 0.50),
        ];

        let (_mapped, model) = project_events_to_capture_space(&events, None, &project.recording);
        assert_eq!(model, AnalysisPointerModel::CaptureNormalized);
    }
}
