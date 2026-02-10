//! Export configuration and job management.

use std::path::PathBuf;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_project_model::project::ExportConfig;

/// An export job ready to be rendered.
#[derive(Debug, Clone)]
pub struct ExportJob {
    /// Project root directory.
    pub project_dir: PathBuf,

    /// Output file path.
    pub output_path: PathBuf,

    /// Export configuration.
    pub config: ExportConfig,

    /// Start time offset (for partial exports).
    pub start_secs: Option<f64>,

    /// End time (for partial exports).
    pub end_secs: Option<f64>,
}

/// Progress callback for export rendering.
pub type ProgressCallback = Box<dyn Fn(ExportProgress) + Send>;

/// Export progress report.
#[derive(Debug, Clone)]
pub struct ExportProgress {
    /// Current progress [0.0, 1.0].
    pub progress: f64,

    /// Frames rendered so far.
    pub frames_rendered: u64,

    /// Total frames to render.
    pub total_frames: u64,

    /// Estimated time remaining in seconds.
    pub eta_secs: f64,

    /// Current stage.
    pub stage: ExportStage,
}

/// Stages of the export process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportStage {
    Preparing,
    Rendering,
    Encoding,
    Finalizing,
    Complete,
    Failed,
}

/// Trait for render backends (GStreamer, FFmpeg, etc.).
pub trait RenderBackend: Send {
    /// Execute the export job.
    fn render(&mut self, job: &ExportJob, progress: Option<ProgressCallback>) -> GrabmeResult<()>;

    /// Check if this backend is available on the system.
    fn is_available(&self) -> bool;

    /// Backend name.
    fn name(&self) -> &str;
}

/// Export the project to a video file.
///
/// This is the main entry point for rendering.
pub async fn export_project(job: ExportJob, progress: Option<ProgressCallback>) -> GrabmeResult<PathBuf> {
    tracing::info!(
        output = %job.output_path.display(),
        format = ?job.config.format,
        "Starting export"
    );

    // Validate inputs
    if !job.project_dir.exists() {
        return Err(GrabmeError::render("Project directory does not exist"));
    }

    // TODO: Initialize GStreamer/FFmpeg render pipeline
    // TODO: Apply zoom keyframes to crop/scale
    // TODO: Overlay synthetic cursor
    // TODO: Composite webcam
    // TODO: Burn subtitles if configured
    // TODO: Encode to target format

    if let Some(cb) = &progress {
        cb(ExportProgress {
            progress: 0.0,
            frames_rendered: 0,
            total_frames: 0,
            eta_secs: 0.0,
            stage: ExportStage::Preparing,
        });
    }

    tracing::warn!("Export pipeline not yet implemented — this is a Phase 4 feature");

    Err(GrabmeError::unsupported(
        "Export pipeline will be implemented in Phase 4",
    ))
}

/// Quick export to clipboard: render to temp file, then copy to clipboard.
pub async fn export_to_clipboard(job: ExportJob) -> GrabmeResult<()> {
    let temp_path = std::env::temp_dir().join("grabme_clipboard_export.mp4");
    let mut clipboard_job = job;
    clipboard_job.output_path = temp_path.clone();

    export_project(clipboard_job, None).await?;

    // TODO: Use xclip/wl-copy to put file handle in clipboard
    tracing::info!(path = %temp_path.display(), "Exported to clipboard");
    Ok(())
}
