#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use grabme_project_model::{event::InputEvent, LoadedProject};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct LoadedProjectBundle {
    name: String,
    width: u32,
    height: u32,
    fps: u32,
    screen_path: Option<String>,
    events: Vec<InputEvent>,
}

#[tauri::command]
fn load_project_bundle(project_path: String) -> Result<LoadedProjectBundle, String> {
    let root = resolve_project_path(&project_path);
    let loaded =
        LoadedProject::load(&root).map_err(|e| format!("Failed to load project metadata: {e}"))?;

    let events = read_events(&root.join("meta").join("events.jsonl"))?;

    let screen_path = loaded
        .project
        .tracks
        .screen
        .as_ref()
        .map(|track| root.join(&track.path))
        .map(|p| p.to_string_lossy().to_string());

    Ok(LoadedProjectBundle {
        name: loaded.project.name,
        width: loaded.project.recording.capture_width,
        height: loaded.project.recording.capture_height,
        fps: loaded.project.recording.fps,
        screen_path,
        events,
    })
}

fn resolve_project_path(project_path: &str) -> PathBuf {
    let path = PathBuf::from(project_path);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn read_events(path: &Path) -> Result<Vec<InputEvent>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read events at {}: {e}", path.display()))?;

    let mut events = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let event: InputEvent =
            serde_json::from_str(trimmed).map_err(|e| format!("Invalid event line: {e}"))?;
        events.push(event);
    }

    Ok(events)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_project_bundle])
        .run(tauri::generate_context!())
        .expect("error while running GrabMe desktop app");
}
