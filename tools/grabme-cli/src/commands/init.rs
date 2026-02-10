//! Initialize a new GrabMe project.

use std::path::PathBuf;

use grabme_project_model::LoadedProject;

pub fn run(name: String, output: PathBuf, width: u32, height: u32) -> anyhow::Result<()> {
    let project_dir = output.join(&name);
    println!("Creating project '{}' at {}", name, project_dir.display());

    let project = LoadedProject::create(&project_dir, &name, width, height, 60)
        .map_err(|e| anyhow::anyhow!("Failed to create project: {e}"))?;

    println!("Project created successfully:");
    println!("  Directory: {}", project.root.display());
    println!("  Resolution: {}x{}", width, height);
    println!("  FPS: 60");
    println!();
    println!("Directory structure:");
    println!("  {}/", name);
    println!("  ├── sources/     (raw media files)");
    println!("  ├── meta/        (project.json, timeline.json, events.jsonl)");
    println!("  ├── cache/       (waveforms, proxies)");
    println!("  └── exports/     (rendered output)");

    Ok(())
}
