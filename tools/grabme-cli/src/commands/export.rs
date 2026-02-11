//! Export a project to video.

use std::path::PathBuf;

use grabme_project_model::project::{AspectMode, ExportConfig, ExportFormat};
use grabme_project_model::LoadedProject;
use grabme_render_engine::export::{export_project, ExportJob, ExportProgress};

pub async fn run(
    path: PathBuf,
    output: Option<PathBuf>,
    format: String,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    println!("Exporting project at: {}", path.display());

    let project =
        LoadedProject::load(&path).map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    let output_path = output.unwrap_or_else(|| path.join("exports").join("output.mp4"));

    let export_format = match format.as_str() {
        "mp4-h264" => ExportFormat::Mp4H264,
        "mp4-h265" => ExportFormat::Mp4H265,
        "gif" => ExportFormat::Gif,
        "webm" => ExportFormat::Webm,
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown format: {format}. Use: mp4-h264, mp4-h265, gif, webm"
            ));
        }
    };

    let config = ExportConfig {
        format: export_format,
        width,
        height,
        fps: project.project.recording.fps,
        video_bitrate_kbps: 8000,
        audio_bitrate_kbps: 192,
        aspect_mode: AspectMode::Landscape,
        burn_subtitles: false,
        webcam: project.project.export.webcam.clone(),
    };

    println!("  Output: {}", output_path.display());
    println!("  Format: {:?}", export_format);
    println!("  Resolution: {width}x{height}");

    let job = ExportJob {
        project_dir: path,
        output_path: output_path.clone(),
        config,
        start_secs: None,
        end_secs: None,
    };

    let progress_cb: Box<dyn Fn(ExportProgress) + Send> = Box::new(|p| {
        print!(
            "\r  Progress: {:.1}% ({}/{} frames, ETA: {:.0}s)  ",
            p.progress * 100.0,
            p.frames_rendered,
            p.total_frames,
            p.eta_secs,
        );
    });

    match export_project(job, Some(progress_cb)).await {
        Ok(_) => {
            println!("\nExport complete: {}", output_path.display());
        }
        Err(e) => {
            println!("\nExport failed: {e}");
        }
    }

    Ok(())
}
