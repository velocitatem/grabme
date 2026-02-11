//! Run Auto-Director analysis on a project.

use std::path::PathBuf;

use grabme_processing_core::auto_zoom::{AutoZoomAnalyzer, AutoZoomConfig};
use grabme_processing_core::cursor_smooth::CursorSmoother;
use grabme_project_model::event::parse_events;
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

    // Filter out header lines (starting with #)
    let event_lines: String = events_content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let events =
        parse_events(&event_lines).map_err(|e| anyhow::anyhow!("Failed to parse events: {e}"))?;

    println!("  Loaded {} events", events.len());

    if events.is_empty() {
        println!("  No events to analyze.");
        return Ok(());
    }

    // Run cursor smoothing
    let mut cursor_config = project.timeline.cursor_config.clone();
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
    let smoothed = smoother.smooth(&events);
    println!("  Smoothed {} pointer positions", smoothed.len());

    // Run auto-zoom analysis
    if vertical {
        println!("  Running vertical (9:16) analysis...");
        let config = grabme_processing_core::vertical::VerticalConfig::default();
        let keyframes =
            grabme_processing_core::vertical::generate_vertical_timeline(&events, &config);
        project.timeline.keyframes = keyframes;
        println!(
            "  Generated {} vertical keyframes",
            project.timeline.keyframes.len()
        );
    } else {
        println!("  Running auto-zoom analysis (chunk={chunk_secs}s)...");
        let config = AutoZoomConfig {
            chunk_duration_secs: chunk_secs,
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
        let timeline = analyzer.analyze(&events);
        project.timeline.keyframes = timeline.keyframes;
        println!("  Generated {} keyframes", project.timeline.keyframes.len());
    }

    // Save updated timeline
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
