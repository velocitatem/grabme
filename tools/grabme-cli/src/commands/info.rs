//! Show project information.

use std::path::PathBuf;

use grabme_project_model::LoadedProject;

pub fn run(path: PathBuf) -> anyhow::Result<()> {
    let project =
        LoadedProject::load(&path).map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    let p = &project.project;

    println!("Project: {}", p.name);
    println!("  ID: {}", p.id);
    println!("  Created: {}", p.created_at);
    println!("  Modified: {}", p.modified_at);
    println!();

    println!("Recording:");
    println!(
        "  Resolution: {}x{} @ {}fps",
        p.recording.capture_width, p.recording.capture_height, p.recording.fps
    );
    println!("  Scale factor: {}", p.recording.scale_factor);
    println!("  Display server: {:?}", p.recording.display_server);
    println!("  Cursor hidden: {}", p.recording.cursor_hidden);
    println!();

    println!("Tracks:");
    if let Some(ref t) = p.tracks.screen {
        println!(
            "  Screen: {} ({:.1}s, {})",
            t.path, t.duration_secs, t.codec
        );
    }
    if let Some(ref t) = p.tracks.webcam {
        println!(
            "  Webcam: {} ({:.1}s, {})",
            t.path, t.duration_secs, t.codec
        );
    }
    if let Some(ref t) = p.tracks.mic {
        println!("  Mic: {} ({:.1}s, {})", t.path, t.duration_secs, t.codec);
    }
    if let Some(ref t) = p.tracks.system_audio {
        println!(
            "  System audio: {} ({:.1}s, {})",
            t.path, t.duration_secs, t.codec
        );
    }
    println!();

    println!("Timeline:");
    println!("  Keyframes: {}", project.timeline.keyframes.len());
    println!("  Effects: {}", project.timeline.effects.len());
    println!("  Cuts: {}", project.timeline.cuts.len());
    println!(
        "  Cursor smoothing: {:?} (factor: {})",
        project.timeline.cursor_config.smoothing, project.timeline.cursor_config.smoothing_factor
    );
    println!();

    println!("Export config:");
    println!("  Format: {:?}", p.export.format);
    println!(
        "  Output: {}x{} @ {}fps",
        p.export.width, p.export.height, p.export.fps
    );

    Ok(())
}
