//! Run Auto-Director analysis on a project.

use std::path::PathBuf;

use grabme_processing_core::auto_zoom::{AutoZoomAnalyzer, AutoZoomConfig};
use grabme_processing_core::cursor_smooth::{CursorSmoother, SmoothingAlgorithm};
use grabme_project_model::event::parse_events;
use grabme_project_model::LoadedProject;

pub fn run(path: PathBuf, chunk_secs: f64, vertical: bool) -> anyhow::Result<()> {
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
    let smoother = CursorSmoother::new(SmoothingAlgorithm::Ema { factor: 0.3 });
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
