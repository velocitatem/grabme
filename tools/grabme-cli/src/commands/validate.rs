//! Validate a GrabMe project bundle.

use std::path::PathBuf;

use grabme_project_model::LoadedProject;

pub fn run(path: PathBuf) -> anyhow::Result<()> {
    println!("Validating project at: {}", path.display());

    let project =
        LoadedProject::load(&path).map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    println!("  Name: {}", project.project.name);
    println!("  Version: {}", project.project.version);
    println!(
        "  Resolution: {}x{}",
        project.project.recording.capture_width, project.project.recording.capture_height
    );
    println!("  FPS: {}", project.project.recording.fps);
    println!("  Timeline keyframes: {}", project.timeline.keyframes.len());

    // Check source files
    let errors = project.validate_sources();
    if errors.is_empty() {
        println!("  Sources: All present");
        println!("\nProject is valid.");
    } else {
        println!("\nValidation issues:");
        for error in &errors {
            println!("  - {error}");
        }
        println!(
            "\n{} issue(s) found. Project may not be fully usable.",
            errors.len()
        );
    }

    Ok(())
}
