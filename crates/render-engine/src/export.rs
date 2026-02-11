//! Export configuration and job management.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_processing_core::cursor_smooth::CursorSmoother;
use grabme_project_model::event::{parse_events, InputEvent};
use grabme_project_model::project::{ExportConfig, ExportFormat, LoadedProject};

use crate::compositor::compute_compositions;

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
pub async fn export_project(
    job: ExportJob,
    progress: Option<ProgressCallback>,
) -> GrabmeResult<PathBuf> {
    tracing::info!(
        output = %job.output_path.display(),
        format = ?job.config.format,
        "Starting export"
    );

    // Validate inputs
    if !job.project_dir.exists() {
        return Err(GrabmeError::render("Project directory does not exist"));
    }

    if let Some(parent) = job.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if let Some(cb) = &progress {
        cb(ExportProgress {
            progress: 0.0,
            frames_rendered: 0,
            total_frames: 0,
            eta_secs: 0.0,
            stage: ExportStage::Preparing,
        });
    }

    let mut backend: Box<dyn RenderBackend> = Box::new(FfmpegBackend::new());
    if !backend.is_available() {
        return Err(GrabmeError::unsupported(
            "No supported render backend found (expected ffmpeg in PATH)",
        ));
    }

    tracing::info!(backend = backend.name(), "Using render backend");
    backend.render(&job, progress)?;

    Ok(job.output_path)
}

// TODO: Initialize GStreamer/FFmpeg render pipeline
// TODO: Apply zoom keyframes to crop/scale
// TODO: Overlay synthetic cursor
// TODO: Composite webcam
// TODO: Burn subtitles if configured
// TODO: Encode to target format

#[derive(Debug, Clone)]
struct LoadedExportInputs {
    project: LoadedProject,
    screen_path: PathBuf,
    source_width: u32,
    source_height: u32,
    webcam_path: Option<PathBuf>,
    mic_path: Option<PathBuf>,
    system_audio_path: Option<PathBuf>,
    events: Vec<InputEvent>,
    duration_secs: f64,
}

#[derive(Debug, Clone)]
struct ExportPlan {
    ffmpeg_args: Vec<String>,
    total_frames: u64,
    expected_duration_secs: f64,
    smoothed_cursor: Vec<(u64, f64, f64)>,
    debug_report: String,
}

#[derive(Debug, Default)]
struct VerificationSummary {
    sampled_frames: usize,
    out_of_bounds_cursors: usize,
    cut_frames_skipped: usize,
}

struct FfmpegBackend;

const MAX_VIEWPORT_EXPR_POINTS: usize = 48;
#[allow(dead_code)]
const MIN_CURSOR_EXPR_POINTS: usize = 64;
#[allow(dead_code)]
const MAX_CURSOR_EXPR_POINTS: usize = 900;
#[allow(dead_code)]
const CURSOR_EXPR_POINTS_PER_SEC: f64 = 2.5;
#[allow(dead_code)]
const CURSOR_SIMPLIFY_TOLERANCE_PX: f64 = 0.8;

impl FfmpegBackend {
    fn new() -> Self {
        Self
    }

    fn load_inputs(&self, job: &ExportJob) -> GrabmeResult<LoadedExportInputs> {
        let project = LoadedProject::load(&job.project_dir)
            .map_err(|e| GrabmeError::render(format!("Failed to load project: {e}")))?;

        let screen_track = project
            .project
            .tracks
            .screen
            .as_ref()
            .ok_or_else(|| GrabmeError::render("Project does not contain a screen track"))?;

        let screen_path = job.project_dir.join(&screen_track.path);
        if !screen_path.exists() {
            return Err(GrabmeError::FileNotFound { path: screen_path });
        }

        let (source_width, source_height) = probe_video_dimensions(&screen_path).unwrap_or((
            project.project.recording.capture_width,
            project.project.recording.capture_height,
        ));

        let webcam_path = project
            .project
            .tracks
            .webcam
            .as_ref()
            .map(|track| job.project_dir.join(&track.path))
            .filter(|path| path.exists());

        let mic_path = project
            .project
            .tracks
            .mic
            .as_ref()
            .map(|track| job.project_dir.join(&track.path))
            .filter(|path| path.exists());

        let system_audio_path = project
            .project
            .tracks
            .system_audio
            .as_ref()
            .map(|track| job.project_dir.join(&track.path))
            .filter(|path| path.exists());

        let events_path = job.project_dir.join("meta").join("events.jsonl");
        let events_content = std::fs::read_to_string(&events_path).map_err(|e| {
            GrabmeError::render(format!(
                "Failed to read events file {}: {e}",
                events_path.display()
            ))
        })?;
        let events_jsonl = strip_events_header(&events_content);
        let events = parse_events(&events_jsonl)
            .map_err(|e| GrabmeError::render(format!("Failed to parse events stream: {e}")))?;

        let mut duration_secs = screen_track.duration_secs;
        if let Some(end) = job.end_secs {
            duration_secs = duration_secs.min(end);
        }
        if let Some(start) = job.start_secs {
            duration_secs = (duration_secs - start).max(0.0);
        }

        Ok(LoadedExportInputs {
            project,
            screen_path,
            source_width,
            source_height,
            webcam_path,
            mic_path,
            system_audio_path,
            events,
            duration_secs,
        })
    }

    fn build_plan(&self, job: &ExportJob, inputs: &LoadedExportInputs) -> GrabmeResult<ExportPlan> {
        let plan_started = std::time::Instant::now();
        if inputs.duration_secs <= 0.0 {
            return Err(GrabmeError::render(
                "Export duration resolved to zero seconds",
            ));
        }

        let mut cursor_config = inputs.project.timeline.cursor_config.clone();
        if let Some(effect_strength) =
            inputs
                .project
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

        let smoothed_cursor = CursorSmoother::new(smoothing).smooth(&inputs.events);
        let fps = job.config.fps.max(1);
        let total_frames = (inputs.duration_secs * fps as f64).ceil() as u64;

        let viewport_points = if let Some(focused_viewport) = derive_static_focus_viewport(inputs, &job.config)
        {
            vec![(0.0, focused_viewport), (inputs.duration_secs, focused_viewport)]
        } else {
            sample_viewport_points(
                &inputs.project.timeline,
                inputs.duration_secs,
                MAX_VIEWPORT_EXPR_POINTS,
            )
        };
        let x_expr =
            build_piecewise_expr(viewport_points.iter().map(|(t, vp)| (*t, vp.x)).collect());
        let y_expr =
            build_piecewise_expr(viewport_points.iter().map(|(t, vp)| (*t, vp.y)).collect());
        let w_expr =
            build_piecewise_expr(viewport_points.iter().map(|(t, vp)| (*t, vp.w)).collect());
        let h_expr =
            build_piecewise_expr(viewport_points.iter().map(|(t, vp)| (*t, vp.h)).collect());

        let webcam_index = if inputs.webcam_path.is_some() {
            Some(1usize)
        } else {
            None
        };

        let mut next_input_index = if webcam_index.is_some() {
            2usize
        } else {
            1usize
        };
        let mic_index = if inputs.mic_path.is_some() {
            let idx = next_input_index;
            next_input_index += 1;
            Some(idx)
        } else {
            None
        };
        let system_audio_index = if inputs.system_audio_path.is_some() {
            Some(next_input_index)
        } else {
            None
        };

        let filter = build_filter_graph(
            &job.config,
            &x_expr,
            &y_expr,
            &w_expr,
            &h_expr,
            webcam_index,
        );
        let filter_len = filter.len();

        let audio_map = if let Some(mic) = mic_index {
            format!("{mic}:a:0?")
        } else if let Some(system) = system_audio_index {
            format!("{system}:a:0?")
        } else {
            "0:a?".to_string()
        };

        let mut args = vec![
            "-y".to_string(),
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-nostats".to_string(),
            "-progress".to_string(),
            "pipe:1".to_string(),
            "-i".to_string(),
            inputs.screen_path.display().to_string(),
        ];

        if let Some(webcam) = &inputs.webcam_path {
            args.push("-i".to_string());
            args.push(webcam.display().to_string());
        }

        if let Some(mic) = &inputs.mic_path {
            args.push("-i".to_string());
            args.push(mic.display().to_string());
        }

        if let Some(system_audio) = &inputs.system_audio_path {
            args.push("-i".to_string());
            args.push(system_audio.display().to_string());
        }

        args.push("-filter_complex".to_string());
        args.push(filter);
        args.push("-map".to_string());
        args.push("[vout]".to_string());
        args.push("-map".to_string());
        args.push(audio_map);
        args.push("-r".to_string());
        args.push(job.config.fps.to_string());
        args.push("-t".to_string());
        args.push(format!("{:.6}", inputs.duration_secs));

        let mut codec_args = codec_args_for_config(&job.config);
        args.append(&mut codec_args);

        args.push(job.output_path.display().to_string());

        let debug_report = format!(
            "duration_secs={:.3}\nframes={}\nviewport_keyframes={}\nviewport_points={}\nsource_width={}\nsource_height={}\nsmoothed_cursor_points={}\nexpr_len_x={}\nexpr_len_y={}\nexpr_len_w={}\nexpr_len_h={}\nfilter_len={}\nffmpeg_args={}\nplan_build_ms={}\n",
            inputs.duration_secs,
            total_frames,
            inputs.project.timeline.keyframes.len(),
            viewport_points.len(),
            inputs.source_width,
            inputs.source_height,
            smoothed_cursor.len(),
            x_expr.len(),
            y_expr.len(),
            w_expr.len(),
            h_expr.len(),
            filter_len,
            args.join(" "),
            plan_started.elapsed().as_millis(),
        );

        tracing::info!(
            duration_secs = inputs.duration_secs,
            frames = total_frames,
            viewport_keyframes = inputs.project.timeline.keyframes.len(),
            viewport_points = viewport_points.len(),
            smoothed_cursor_points = smoothed_cursor.len(),
            filter_len,
            "Export plan built"
        );

        Ok(ExportPlan {
            ffmpeg_args: args,
            total_frames,
            expected_duration_secs: inputs.duration_secs,
            smoothed_cursor,
            debug_report,
        })
    }

    fn run_ffmpeg(
        &self,
        plan: &ExportPlan,
        progress: Option<ProgressCallback>,
    ) -> GrabmeResult<()> {
        tracing::debug!(args = ?plan.ffmpeg_args, "Running ffmpeg");
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&plan.ffmpeg_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let start = std::time::Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| GrabmeError::render(format!("Failed to start ffmpeg: {e}")))?;

        tracing::info!(
            pid = child.id(),
            args_len = plan.ffmpeg_args.len(),
            total_frames = plan.total_frames,
            "ffmpeg process started"
        );

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GrabmeError::render("Failed to capture ffmpeg stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| GrabmeError::render("Failed to capture ffmpeg stderr"))?;

        // Drain stderr concurrently to avoid ffmpeg blocking on a full stderr pipe.
        let stderr_task = std::thread::spawn(move || -> String {
            let mut reader = BufReader::new(stderr);
            let mut output = String::new();
            match reader.read_to_string(&mut output) {
                Ok(_) => output,
                Err(err) => format!("<failed to read ffmpeg stderr: {err}>"),
            }
        });

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        let mut latest_progress = ProgressState::default();
        let mut last_progress_secs = 0.0f64;
        let mut last_progress_wall = std::time::Instant::now();
        loop {
            line.clear();
            let bytes = reader
                .read_line(&mut line)
                .map_err(|e| GrabmeError::render(format!("Failed reading ffmpeg progress: {e}")))?;
            if bytes == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((key, value)) = trimmed.split_once('=') {
                latest_progress.update(key, value);
                if key == "progress" {
                    let advanced = latest_progress.out_time_secs > last_progress_secs + 0.001;
                    if advanced {
                        last_progress_secs = latest_progress.out_time_secs;
                        last_progress_wall = std::time::Instant::now();
                    }
                    if let Some(cb) = &progress {
                        cb(progress_report(
                            &latest_progress,
                            plan.total_frames,
                            plan.expected_duration_secs,
                            start.elapsed().as_secs_f64(),
                        ));
                    }
                    if last_progress_wall.elapsed().as_secs() >= 10 {
                        tracing::warn!(
                            out_time_secs = latest_progress.out_time_secs,
                            elapsed_secs = start.elapsed().as_secs_f64(),
                            "No ffmpeg progress advancement for 10s"
                        );
                        last_progress_wall = std::time::Instant::now();
                    }
                }
            }
        }

        let status = child
            .wait()
            .map_err(|e| GrabmeError::render(format!("Failed to wait on ffmpeg: {e}")))?;

        let stderr_output = stderr_task
            .join()
            .unwrap_or_else(|_| "<failed to join stderr reader>".to_string());

        if !status.success() {
            return Err(GrabmeError::render(format!(
                "ffmpeg export failed (status {}): {}",
                status,
                stderr_output.trim()
            )));
        }

        if let Some(cb) = &progress {
            cb(ExportProgress {
                progress: 1.0,
                frames_rendered: plan.total_frames,
                total_frames: plan.total_frames,
                eta_secs: 0.0,
                stage: ExportStage::Complete,
            });
        }

        Ok(())
    }

    fn run_visual_verification(
        &self,
        job: &ExportJob,
        inputs: &LoadedExportInputs,
        plan: &ExportPlan,
    ) -> GrabmeResult<VerificationSummary> {
        let compositions = compute_compositions(
            &inputs.project.timeline,
            &plan.smoothed_cursor,
            job.config.width,
            job.config.height,
            job.config.fps,
            inputs.duration_secs,
        );

        let mut summary = VerificationSummary {
            sampled_frames: compositions.len(),
            out_of_bounds_cursors: 0,
            cut_frames_skipped: plan.total_frames as usize - compositions.len(),
        };

        for comp in &compositions {
            if let Some(cursor) = &comp.cursor {
                if cursor.x < 0.0
                    || cursor.y < 0.0
                    || cursor.x > job.config.width as f64
                    || cursor.y > job.config.height as f64
                {
                    summary.out_of_bounds_cursors += 1;
                }
            }
        }

        let report_path = job.output_path.with_extension("verification.json");
        let report = serde_json::json!({
            "output": job.output_path,
            "sampled_frames": summary.sampled_frames,
            "cut_frames_skipped": summary.cut_frames_skipped,
            "out_of_bounds_cursors": summary.out_of_bounds_cursors,
            "status": if summary.out_of_bounds_cursors == 0 { "ok" } else { "warn" }
        });
        std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
        tracing::info!(report = %report_path.display(), "Wrote visual verification report");

        Ok(summary)
    }
}

impl RenderBackend for FfmpegBackend {
    fn render(&mut self, job: &ExportJob, progress: Option<ProgressCallback>) -> GrabmeResult<()> {
        let started = std::time::Instant::now();
        let inputs = self.load_inputs(job)?;
        tracing::info!(
            load_ms = started.elapsed().as_millis(),
            events = inputs.events.len(),
            duration_secs = inputs.duration_secs,
            "Export inputs loaded"
        );

        let plan = self.build_plan(job, &inputs)?;
        let debug_path = job.output_path.with_extension("ffmpeg-debug.txt");
        if let Err(err) = std::fs::write(&debug_path, &plan.debug_report) {
            tracing::warn!(error = %err, path = %debug_path.display(), "Failed to write ffmpeg debug report");
        } else {
            tracing::info!(path = %debug_path.display(), "Wrote ffmpeg debug report");
        }

        if let Some(cb) = &progress {
            cb(ExportProgress {
                progress: 0.0,
                frames_rendered: 0,
                total_frames: plan.total_frames,
                eta_secs: 0.0,
                stage: ExportStage::Preparing,
            });
        }

        self.run_ffmpeg(&plan, progress)?;
        let summary = self.run_visual_verification(job, &inputs, &plan)?;
        if summary.out_of_bounds_cursors > 0 {
            tracing::warn!(
                out_of_bounds = summary.out_of_bounds_cursors,
                "Visual verification found cursor coordinates outside output bounds"
            );
        }
        tracing::info!(
            elapsed_secs = started.elapsed().as_secs_f64(),
            "Export finished"
        );
        Ok(())
    }

    fn is_available(&self) -> bool {
        command_exists("ffmpeg")
    }

    fn name(&self) -> &str {
        "ffmpeg"
    }
}

fn strip_events_header(events_content: &str) -> String {
    events_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sample_viewport_points(
    timeline: &grabme_project_model::timeline::Timeline,
    duration_secs: f64,
    max_points: usize,
) -> Vec<(f64, grabme_project_model::viewport::Viewport)> {
    let mut key_times: Vec<f64> = timeline.keyframes.iter().map(|k| k.time_secs).collect();
    key_times.push(duration_secs);
    key_times.sort_by(f64::total_cmp);
    key_times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    if key_times.is_empty() {
        key_times.push(0.0);
    }

    let points: Vec<(f64, grabme_project_model::viewport::Viewport)> = key_times
        .into_iter()
        .map(|t| (t, timeline.viewport_at(t)))
        .collect();

    downsample_timed_points(points, max_points.max(2))
}

fn derive_static_focus_viewport(
    inputs: &LoadedExportInputs,
    config: &ExportConfig,
) -> Option<grabme_project_model::viewport::Viewport> {
    if inputs.project.timeline.keyframes.len() != 1
        || inputs.project.timeline.keyframes[0].viewport
            != grabme_project_model::viewport::Viewport::FULL
    {
        return None;
    }

    let source_w = inputs.source_width.max(1) as f64;
    let source_h = inputs.source_height.max(1) as f64;
    let source_aspect = source_w / source_h;
    let target_aspect = config.width.max(1) as f64 / config.height.max(1) as f64;

    if source_aspect <= target_aspect * 1.05 {
        return None;
    }

    let focus_x = latest_pointer_axis(&inputs.events, true)
        .or_else(|| median_pointer_axis(&inputs.events, true))
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let focus_y = latest_pointer_axis(&inputs.events, false)
        .or_else(|| median_pointer_axis(&inputs.events, false))
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);

    let estimated_monitor_width = config.width.max(1) as f64;
    let estimated_count = (source_w / estimated_monitor_width).round() as usize;
    let monitor_count = estimated_count.clamp(1, 8);

    let slot_w = 1.0 / monitor_count as f64;
    let configured_monitor_index = inputs.project.project.recording.monitor_index;
    let monitor_index = if configured_monitor_index < monitor_count {
        configured_monitor_index
    } else {
        select_focus_monitor_index(&inputs.events, monitor_count, focus_x)
    };
    let slot_x = monitor_index as f64 * slot_w;

    let slot_aspect = source_aspect * slot_w;

    if slot_aspect > target_aspect {
        let w = (target_aspect / source_aspect).clamp(0.01, slot_w);
        let local_x = ((focus_x - slot_x) / slot_w).clamp(0.0, 1.0);
        let x = (slot_x + (slot_w - w) * local_x).clamp(slot_x, slot_x + slot_w - w);
        Some(grabme_project_model::viewport::Viewport::new(x, 0.0, w, 1.0))
    } else {
        let h = (slot_aspect / target_aspect).clamp(0.01, 1.0);
        let y = ((1.0 - h) * focus_y).clamp(0.0, 1.0 - h);
        Some(grabme_project_model::viewport::Viewport::new(slot_x, y, slot_w, h))
    }
}

fn median_pointer_axis(events: &[InputEvent], x_axis: bool) -> Option<f64> {
    let mut values: Vec<f64> = events
        .iter()
        .filter_map(|event| event.pointer_position())
        .map(|(x, y)| if x_axis { x } else { y })
        .collect();

    if values.is_empty() {
        return None;
    }

    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}

fn latest_pointer_axis(events: &[InputEvent], x_axis: bool) -> Option<f64> {
    events
        .iter()
        .rev()
        .filter_map(|event| event.pointer_position())
        .map(|(x, y)| if x_axis { x } else { y })
        .next()
}

fn select_focus_monitor_index(events: &[InputEvent], monitor_count: usize, fallback_x: f64) -> usize {
    if monitor_count <= 1 {
        return 0;
    }

    let mut votes = vec![0usize; monitor_count];
    let recent_positions: Vec<f64> = events
        .iter()
        .rev()
        .filter_map(|event| event.pointer_position().map(|(x, _)| x.clamp(0.0, 1.0)))
        .take(180)
        .collect();

    for x in recent_positions {
        let idx = ((x * monitor_count as f64).floor() as usize).min(monitor_count - 1);
        votes[idx] += 1;
    }

    if let Some((idx, count)) = votes
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(idx, count)| (idx, *count))
    {
        if count > 0 {
            return idx;
        }
    }

    ((fallback_x * monitor_count as f64).floor() as usize).min(monitor_count - 1)
}

#[allow(dead_code)]
fn sample_cursor_points(
    smoothed_cursor: &[(u64, f64, f64)],
    project: &LoadedProject,
    out_w: u32,
    out_h: u32,
    duration_secs: f64,
    fps: u32,
) -> Vec<(f64, f64, f64)> {
    if smoothed_cursor.is_empty() {
        return vec![(0.0, out_w as f64 / 2.0, out_h as f64 / 2.0)];
    }

    let mut points = Vec::with_capacity(smoothed_cursor.len() + 1);
    for &(ts, x, y) in smoothed_cursor {
        let t_secs = (ts as f64 / 1_000_000_000.0).clamp(0.0, duration_secs);
        let viewport = project.timeline.viewport_at(t_secs);
        let (px, py) = project_to_output_coords(x, y, viewport, out_w, out_h);
        points.push((t_secs, px, py));
    }

    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);

    if points.last().map(|(t, _, _)| *t).unwrap_or(0.0) < duration_secs {
        let last = *points.last().unwrap();
        points.push((duration_secs, last.1, last.2));
    }

    let point_budget = derive_cursor_expr_point_budget(duration_secs, fps).min(points.len());
    if points.len() <= point_budget {
        return points;
    }

    simplify_cursor_points(points, point_budget, CURSOR_SIMPLIFY_TOLERANCE_PX)
}

#[allow(dead_code)]
fn derive_cursor_expr_point_budget(duration_secs: f64, fps: u32) -> usize {
    let fps = fps.max(1) as usize;
    let expected = (duration_secs.max(1.0) * CURSOR_EXPR_POINTS_PER_SEC).round() as usize;
    let floor = MIN_CURSOR_EXPR_POINTS.max(fps * 2);
    let cap = MAX_CURSOR_EXPR_POINTS.min((duration_secs.max(1.0) * fps as f64).round() as usize);
    expected.max(floor).min(cap).max(2)
}

#[allow(dead_code)]
fn simplify_cursor_points(
    points: Vec<(f64, f64, f64)>,
    max_points: usize,
    tolerance_px: f64,
) -> Vec<(f64, f64, f64)> {
    if points.len() <= max_points {
        return points;
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;

    let mut stack = vec![(0usize, points.len() - 1usize)];
    while let Some((start, end)) = stack.pop() {
        if end <= start + 1 {
            continue;
        }

        let mut max_dist = 0.0;
        let mut max_idx = None;
        for idx in (start + 1)..end {
            let dist = point_line_distance_px(points[idx], points[start], points[end]);
            if dist > max_dist {
                max_dist = dist;
                max_idx = Some(idx);
            }
        }

        if max_dist > tolerance_px {
            if let Some(idx) = max_idx {
                keep[idx] = true;
                stack.push((start, idx));
                stack.push((idx, end));
            }
        }
    }

    let mut simplified: Vec<(f64, f64, f64)> = points
        .iter()
        .zip(keep.iter())
        .filter_map(|(p, &k)| if k { Some(*p) } else { None })
        .collect();

    if simplified.len() > max_points {
        let reduced: Vec<(f64, (f64, f64))> =
            simplified.iter().map(|(t, x, y)| (*t, (*x, *y))).collect();
        return downsample_timed_points(reduced, max_points)
            .into_iter()
            .map(|(t, (x, y))| (t, x, y))
            .collect();
    }

    if simplified.first().map(|(t, _, _)| *t).unwrap_or(0.0) > 0.0 {
        simplified.insert(0, points[0]);
    }

    simplified
}

#[allow(dead_code)]
fn point_line_distance_px(point: (f64, f64, f64), a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let (px, py) = (point.1, point.2);
    let (ax, ay) = (a.1, a.2);
    let (bx, by) = (b.1, b.2);

    let dx = bx - ax;
    let dy = by - ay;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }

    let t = (((px - ax) * dx) + ((py - ay) * dy)) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn downsample_timed_points<T: Clone>(points: Vec<(f64, T)>, max_points: usize) -> Vec<(f64, T)> {
    if points.len() <= max_points {
        return points;
    }

    let target = max_points.max(2);
    let last_idx = points.len() - 1;
    let mut selected = Vec::with_capacity(target);
    for i in 0..target {
        let idx = ((i as f64 / (target - 1) as f64) * last_idx as f64).round() as usize;
        selected.push(points[idx].clone());
    }
    selected
}

#[allow(dead_code)]
fn project_to_output_coords(
    x: f64,
    y: f64,
    viewport: grabme_project_model::viewport::Viewport,
    out_w: u32,
    out_h: u32,
) -> (f64, f64) {
    let local_x = ((x - viewport.x) / viewport.w).clamp(0.0, 1.0);
    let local_y = ((y - viewport.y) / viewport.h).clamp(0.0, 1.0);
    (local_x * out_w as f64, local_y * out_h as f64)
}

fn build_piecewise_expr(mut points: Vec<(f64, f64)>) -> String {
    if points.is_empty() {
        return "0".to_string();
    }

    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);

    if points.len() == 1 {
        return format!("{:.6}", points[0].1);
    }

    let mut expr = format!("{:.6}", points.last().unwrap().1);
    for idx in (0..points.len() - 1).rev() {
        let (t0, v0) = points[idx];
        let (t1, v1) = points[idx + 1];
        if (t1 - t0).abs() < 1e-9 {
            continue;
        }

        let interp = format!(
            "{v0:.6}+({delta:.6})*(t-{t0:.6})/{dur:.6}",
            delta = v1 - v0,
            dur = t1 - t0
        );
        expr = format!("if(lt(t,{t1:.6}),{interp},{tail})", tail = expr);
    }

    expr
}

fn build_filter_graph(
    config: &ExportConfig,
    x_expr: &str,
    y_expr: &str,
    w_expr: &str,
    h_expr: &str,
    webcam_index: Option<usize>,
) -> String {
    let mut graph = String::new();

    graph.push_str(&format!(
        "[0:v]crop=w='iw*({w})':h='ih*({h})':x='iw*({x})':y='ih*({y})',scale={out_w}:{out_h}:flags=lanczos,format=yuv420p[base]",
        w = w_expr,
        h = h_expr,
        x = x_expr,
        y = y_expr,
        out_w = config.width,
        out_h = config.height,
    ));

    if let Some(webcam_idx) = webcam_index {
        let webcam_w = (config.width as f64 * 0.24).round() as u32;
        let webcam_h = (config.height as f64 * 0.24).round() as u32;
        let margin_x = (config.width as f64 * 0.03).round() as u32;
        let margin_y = (config.height as f64 * 0.03).round() as u32;

        graph.push_str(&format!(
            ";[{webcam}:v]scale={webcam_w}:{webcam_h}:flags=lanczos,format=yuva420p,colorchannelmixer=aa=0.92[webcam];[base][webcam]overlay=x=W-w-{mx}:y=H-h-{my}[vout]",
            webcam = webcam_idx,
            webcam_w = webcam_w.max(2),
            webcam_h = webcam_h.max(2),
            mx = margin_x,
            my = margin_y,
        ));
    } else {
        graph.push_str(";[base]null[vout]");
    }

    graph
}

fn codec_args_for_config(config: &ExportConfig) -> Vec<String> {
    let video_bitrate = format!("{}k", config.video_bitrate_kbps.max(1000));
    let audio_bitrate = format!("{}k", config.audio_bitrate_kbps.max(64));

    match config.format {
        ExportFormat::Mp4H264 => vec![
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-b:v".to_string(),
            video_bitrate,
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            audio_bitrate,
            "-movflags".to_string(),
            "+faststart".to_string(),
        ],
        ExportFormat::Mp4H265 => vec![
            "-c:v".to_string(),
            "libx265".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-b:v".to_string(),
            video_bitrate,
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            audio_bitrate,
            "-movflags".to_string(),
            "+faststart".to_string(),
        ],
        ExportFormat::Gif => vec![
            "-vf".to_string(),
            "fps=15,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".to_string(),
        ],
        ExportFormat::Webm => vec![
            "-c:v".to_string(),
            "libvpx-vp9".to_string(),
            "-b:v".to_string(),
            video_bitrate,
            "-c:a".to_string(),
            "libopus".to_string(),
            "-b:a".to_string(),
            "128k".to_string(),
        ],
    }
}

fn command_exists(binary: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {binary} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn probe_video_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
        ])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?;
    let line = raw.lines().next()?.trim();
    let (w, h) = line.split_once('x')?;
    let width = w.parse::<u32>().ok()?;
    let height = h.parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

#[derive(Debug, Default)]
struct ProgressState {
    out_time_secs: f64,
    complete: bool,
}

impl ProgressState {
    fn update(&mut self, key: &str, value: &str) {
        match key {
            "out_time_ms" => {
                if let Ok(ms) = value.parse::<f64>() {
                    self.out_time_secs = ms / 1_000_000.0;
                }
            }
            "out_time_us" => {
                if let Ok(us) = value.parse::<f64>() {
                    self.out_time_secs = us / 1_000_000.0;
                }
            }
            "progress" => {
                self.complete = value == "end";
            }
            _ => {}
        }
    }
}

fn progress_report(
    state: &ProgressState,
    total_frames: u64,
    expected_duration_secs: f64,
    elapsed_secs: f64,
) -> ExportProgress {
    let progress = if expected_duration_secs <= 0.0 {
        0.0
    } else {
        (state.out_time_secs / expected_duration_secs).clamp(0.0, 1.0)
    };

    let frames_rendered = (progress * total_frames as f64).round() as u64;
    let eta_secs = if progress > 0.0 {
        (elapsed_secs / progress) - elapsed_secs
    } else {
        0.0
    }
    .max(0.0);

    ExportProgress {
        progress: if state.complete { 1.0 } else { progress },
        frames_rendered,
        total_frames,
        eta_secs,
        stage: if state.complete {
            ExportStage::Finalizing
        } else {
            ExportStage::Rendering
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_events_header() {
        let input = "# header\n\n{\"t\":0,\"type\":\"pointer\",\"x\":0.5,\"y\":0.5}\n";
        let output = strip_events_header(input);
        assert_eq!(output, "{\"t\":0,\"type\":\"pointer\",\"x\":0.5,\"y\":0.5}");
    }

    #[test]
    fn test_piecewise_expr_single_point() {
        let expr = build_piecewise_expr(vec![(0.0, 0.42)]);
        assert_eq!(expr, "0.420000");
    }

    #[test]
    fn test_cursor_expr_budget_scales_with_duration() {
        let short = derive_cursor_expr_point_budget(4.0, 60);
        let long = derive_cursor_expr_point_budget(120.0, 60);
        assert!(long > short);
        assert!(short >= MIN_CURSOR_EXPR_POINTS);
        assert!(long <= MAX_CURSOR_EXPR_POINTS);
    }

    #[test]
    fn test_simplify_cursor_points_preserves_endpoints() {
        let mut points = Vec::new();
        for i in 0..100 {
            let t = i as f64 * 0.1;
            let x = i as f64;
            let y = (i as f64 * 0.2).sin() * 20.0;
            points.push((t, x, y));
        }

        let simplified = simplify_cursor_points(points.clone(), 20, 0.5);
        assert!(simplified.len() <= 20);
        assert_eq!(simplified.first().unwrap(), points.first().unwrap());
        assert_eq!(simplified.last().unwrap(), points.last().unwrap());
    }
}
