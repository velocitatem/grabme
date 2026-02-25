use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use grabme_capture_engine::{
    AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig, SessionConfig,
};
use grabme_platform_linux::{detect_monitors, MonitorInfo};
use grabme_processing_core::auto_zoom::{AutoZoomAnalyzer, AutoZoomConfig};
use grabme_project_model::event::{
    parse_events, EventKind, EventStreamHeader, InputEvent, PointerCoordinateSpace,
};
use grabme_project_model::project::{
    AspectMode, ExportConfig, ExportFormat, LoadedProject, RecordingConfig,
};
use grabme_project_model::timeline::{CameraKeyframe, EasingFunction, KeyframeSource};
use grabme_project_model::viewport::Viewport;
use grabme_render_engine::export::{export_project, ExportJob, ExportProgress};

mod webcam_preview;
use webcam_preview::WebcamPreview;

// ── Dimensions (logical pixels – intentionally small) ────────────────────────
//
// These are egui logical pixels. On a 2x HiDPI display the window manager will
// scale them up, so we keep them compact. The actual on-screen pixel size
// depends on the monitor scale factor.

const BUBBLE_HEIGHT: f32 = 36.0;
const BUBBLE_EXPANDED_HEIGHT: f32 = 332.0;
const BUBBLE_WIDTH_IDLE: f32 = 320.0;
const BUBBLE_WIDTH_RECORDING: f32 = 138.0;
const BUBBLE_WIDTH_POST: f32 = 320.0;
const CIRCLE_RADIUS: f32 = 10.0;
const PADDING: f32 = 6.0;
const DROPDOWN_MAX_HEIGHT: f32 = 220.0;

const RED_IDLE: Color32 = Color32::from_rgb(200, 52, 52);
const RED_RECORDING: Color32 = Color32::from_rgb(255, 60, 60);
const RED_PULSE_DIM: Color32 = Color32::from_rgb(140, 30, 30);
const BG_COLOR: Color32 = Color32::from_rgba_premultiplied(245, 248, 252, 242);
const BG_EXPANDED_COLOR: Color32 = Color32::from_rgba_premultiplied(236, 242, 250, 230);
const BORDER_COLOR: Color32 = Color32::from_rgb(197, 208, 224);
const TEXT_COLOR: Color32 = Color32::from_rgb(32, 42, 56);
const TEXT_DIM: Color32 = Color32::from_rgb(97, 112, 132);
const ACCENT: Color32 = Color32::from_rgb(37, 121, 220);

// ── Timer presets ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum CountdownPreset {
    None,
    FiveSec,
    TenSec,
}

impl CountdownPreset {
    fn label(&self) -> &'static str {
        match self {
            Self::None => "No delay",
            Self::FiveSec => "5s",
            Self::TenSec => "10s",
        }
    }

    fn seconds(&self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::FiveSec => 5.0,
            Self::TenSec => 10.0,
        }
    }

    const ALL: [CountdownPreset; 3] = [Self::None, Self::FiveSec, Self::TenSec];
}

// ── Workflow stages ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Countdown,
    Starting,
    Recording,
    Stopping,
    PostRecord,
    Rendering,
}

impl Stage {
    /// Target window width for this stage.
    fn bubble_width(self) -> f32 {
        match self {
            Stage::Idle | Stage::Countdown | Stage::Starting => BUBBLE_WIDTH_IDLE,
            Stage::Recording | Stage::Stopping => BUBBLE_WIDTH_RECORDING,
            Stage::PostRecord | Stage::Rendering => BUBBLE_WIDTH_POST,
        }
    }
}

// ── Render messages (background thread -> UI) ────────────────────────────────

#[derive(Debug)]
enum RenderMessage {
    Progress {
        percent: f64,
        #[allow(dead_code)]
        frames_rendered: u64,
        #[allow(dead_code)]
        total_frames: u64,
        eta_secs: f64,
    },
    Complete {
        output: PathBuf,
    },
    Failed {
        error: String,
    },
}

// ── Application state ────────────────────────────────────────────────────────

struct OverlayApp {
    runtime: tokio::runtime::Runtime,

    // Recording
    session: Option<CaptureSession>,
    start_task: Option<tokio::task::JoinHandle<Result<CaptureSession, String>>>,
    stop_task: Option<tokio::task::JoinHandle<Result<PathBuf, String>>>,
    stage: Stage,
    prev_stage: Stage, // track transitions to avoid per-frame resizes

    // Config
    project_name: String,
    output_dir: String,
    fps: u32,
    mic: bool,
    system_audio: bool,
    webcam: bool,
    webcam_preview_enabled: bool,

    // Dropdowns
    countdown_preset: CountdownPreset,
    monitors: Vec<MonitorInfo>,
    selected_monitor: usize,

    // Countdown
    countdown_started: Option<Instant>,

    // Status
    status: String,
    active_project_path: Option<PathBuf>,
    last_export_path: Option<PathBuf>,

    // Render progress
    render_receiver: Option<Receiver<RenderMessage>>,
    render_percent: f64,
    render_eta_secs: f64,

    // Optional external live webcam preview process.
    webcam_preview: WebcamPreview,

    // Window behavior
    centered_once: bool,
    menus_open: bool,
    prev_window_size: Vec2,
    last_window_monitor: Option<usize>,
    recording_monitor_index: Option<usize>,
    relocated_for_recording: bool,
    pre_record_outer_pos: Option<Pos2>,
}

impl Default for OverlayApp {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime should initialize");

        let monitors = detect_monitors().unwrap_or_default();
        let monitor_count = monitors.len();

        Self {
            runtime,
            session: None,
            start_task: None,
            stop_task: None,
            stage: Stage::Idle,
            prev_stage: Stage::Idle,
            project_name: "recording".to_string(),
            output_dir: ".".to_string(),
            fps: 60,
            mic: true,
            system_audio: true,
            webcam: false,
            webcam_preview_enabled: false,
            countdown_preset: CountdownPreset::None,
            monitors,
            selected_monitor: 0.min(monitor_count.saturating_sub(1)),
            countdown_started: None,
            status: String::new(),
            active_project_path: None,
            last_export_path: None,
            render_receiver: None,
            render_percent: 0.0,
            render_eta_secs: 0.0,
            webcam_preview: WebcamPreview::new(),
            centered_once: false,
            menus_open: false,
            prev_window_size: Vec2::new(BUBBLE_WIDTH_IDLE, BUBBLE_HEIGHT),
            last_window_monitor: None,
            recording_monitor_index: None,
            relocated_for_recording: false,
            pre_record_outer_pos: None,
        }
    }
}

// ── Recording logic ──────────────────────────────────────────────────────────

impl OverlayApp {
    fn build_session_config(&self) -> SessionConfig {
        SessionConfig {
            name: self.project_name.trim().to_string(),
            output_dir: PathBuf::from(self.output_dir.trim()),
            screen: ScreenCaptureConfig {
                mode: CaptureMode::FullScreen {
                    monitor_index: self.selected_monitor,
                },
                hide_cursor: true,
            },
            audio: AudioCaptureConfig {
                mic: self.mic,
                system: self.system_audio,
                app_isolation: None,
                sample_rate: 48_000,
            },
            webcam: self.webcam,
            fps: self.fps,
            pointer_sample_rate_hz: 60,
        }
    }

    fn initiate_recording(&mut self) {
        if self.project_name.trim().is_empty() {
            self.status = "Project name required".to_string();
            return;
        }
        let delay = self.countdown_preset.seconds();
        if delay > 0.0 {
            self.countdown_started = Some(Instant::now());
            self.stage = Stage::Countdown;
            self.status = format!("Starting in {delay:.0}s...");
        } else {
            self.start_recording_now();
        }
    }

    fn start_recording_now(&mut self) {
        if self.start_task.is_some() || self.stop_task.is_some() || self.session.is_some() {
            return;
        }

        self.status = "Starting...".to_string();
        self.stage = Stage::Starting;

        let config = self.build_session_config();
        let handle = self.runtime.handle().clone();
        self.start_task = Some(handle.spawn(async move {
            let mut session = CaptureSession::new(config);
            session.start().await.map_err(|e| e.to_string())?;
            Ok(session)
        }));
    }

    fn stop_recording(&mut self) {
        if self.start_task.is_some() || self.stop_task.is_some() {
            return;
        }

        self.webcam_preview.stop();

        let Some(mut session) = self.session.take() else {
            return;
        };

        self.status = "Stopping...".to_string();
        self.stage = Stage::Stopping;

        let handle = self.runtime.handle().clone();
        self.stop_task =
            Some(handle.spawn(async move { session.stop().await.map_err(|e| e.to_string()) }));
    }

    fn poll_session_tasks(&mut self) {
        let start_finished = self
            .start_task
            .as_ref()
            .map(|task| task.is_finished())
            .unwrap_or(false);
        if start_finished {
            let task = self
                .start_task
                .take()
                .expect("start task exists if finished");
            match self.runtime.block_on(task) {
                Ok(Ok(session)) => {
                    self.status = String::new();
                    self.last_export_path = None;
                    self.active_project_path = None;
                    self.stage = Stage::Recording;
                    self.session = Some(session);
                    self.recording_monitor_index = Some(self.selected_monitor);
                    self.relocated_for_recording = false;
                    self.pre_record_outer_pos = None;

                    if self.webcam && self.webcam_preview_enabled {
                        if let Err(err) = self.webcam_preview.start() {
                            self.webcam_preview_enabled = false;
                            self.status =
                                format!("Webcam preview unavailable: {err} (device may be busy)");
                        }
                    }
                }
                Ok(Err(err)) => {
                    self.webcam_preview.stop();
                    self.status = format!("Failed: {err}");
                    self.stage = Stage::Idle;
                    self.recording_monitor_index = None;
                }
                Err(err) => {
                    self.webcam_preview.stop();
                    self.status = format!("Failed: {err}");
                    self.stage = Stage::Idle;
                    self.recording_monitor_index = None;
                }
            }
        }

        let stop_finished = self
            .stop_task
            .as_ref()
            .map(|task| task.is_finished())
            .unwrap_or(false);
        if stop_finished {
            let task = self.stop_task.take().expect("stop task exists if finished");
            match self.runtime.block_on(task) {
                Ok(Ok(path)) => {
                    self.webcam_preview.stop();
                    self.active_project_path = Some(path);
                    self.stage = Stage::PostRecord;
                    self.status = "Stopped".to_string();
                    self.recording_monitor_index = None;
                }
                Ok(Err(err)) => {
                    self.webcam_preview.stop();
                    self.status = format!("Stop failed: {err}");
                    self.stage = Stage::Idle;
                    self.recording_monitor_index = None;
                }
                Err(err) => {
                    self.webcam_preview.stop();
                    self.status = format!("Stop failed: {err}");
                    self.stage = Stage::Idle;
                    self.recording_monitor_index = None;
                }
            }
        }
    }

    fn run_auto_direct(&mut self) {
        let Some(project_path) = self.active_project_path.as_ref() else {
            self.status = "No project".to_string();
            return;
        };
        match auto_direct_project(project_path) {
            Ok(n) => self.status = format!("{n} keyframes"),
            Err(err) => self.status = format!("Auto-Direct failed: {err}"),
        }
    }

    fn start_render(&mut self) {
        let Some(project_path) = self.active_project_path.clone() else {
            return;
        };

        let auto_keyframes = match auto_direct_project(&project_path) {
            Ok(count) => count,
            Err(err) => {
                self.status = format!("Render blocked: Auto-Direct failed ({err})");
                return;
            }
        };

        let (tx, rx) = mpsc::channel::<RenderMessage>();
        self.render_receiver = Some(rx);
        self.render_percent = 0.0;
        self.render_eta_secs = 0.0;
        self.stage = Stage::Rendering;
        self.status = format!("Rendering... ({auto_keyframes} keyframes)");

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    let _ = tx.send(RenderMessage::Failed {
                        error: format!("Runtime failed: {err}"),
                    });
                    return;
                }
            };

            let result = runtime.block_on(async {
                let loaded = LoadedProject::load(&project_path)
                    .map_err(|e| anyhow::anyhow!("Load failed: {e}"))?;

                let output_path = project_path.join("exports").join("output.mp4");
                let config = ExportConfig {
                    format: ExportFormat::Mp4H264,
                    width: loaded.project.export.width,
                    height: loaded.project.export.height,
                    fps: loaded.project.recording.fps,
                    video_bitrate_kbps: loaded.project.export.video_bitrate_kbps,
                    audio_bitrate_kbps: loaded.project.export.audio_bitrate_kbps,
                    aspect_mode: AspectMode::Landscape,
                    burn_subtitles: loaded.project.export.burn_subtitles,
                    webcam: loaded.project.export.webcam.clone(),
                    canvas: loaded.project.export.canvas.clone(),
                };

                let tx_progress = tx.clone();
                let progress_cb: Box<dyn Fn(ExportProgress) + Send> = Box::new(move |p| {
                    let _ = tx_progress.send(RenderMessage::Progress {
                        percent: p.progress,
                        frames_rendered: p.frames_rendered,
                        total_frames: p.total_frames,
                        eta_secs: p.eta_secs,
                    });
                });

                let job = ExportJob {
                    project_dir: project_path.clone(),
                    output_path: output_path.clone(),
                    config,
                    start_secs: None,
                    end_secs: None,
                };

                export_project(job, Some(progress_cb))
                    .await
                    .map_err(|e| anyhow::anyhow!("Render failed: {e}"))
            });

            match result {
                Ok(path) => {
                    let _ = tx.send(RenderMessage::Complete { output: path });
                }
                Err(err) => {
                    let _ = tx.send(RenderMessage::Failed {
                        error: err.to_string(),
                    });
                }
            }
        });
    }

    fn poll_render_messages(&mut self) {
        let Some(receiver) = self.render_receiver.as_ref() else {
            return;
        };
        loop {
            match receiver.try_recv() {
                Ok(RenderMessage::Progress {
                    percent, eta_secs, ..
                }) => {
                    self.render_percent = percent;
                    self.render_eta_secs = eta_secs;
                    self.status = format!("{:.0}% (ETA {eta_secs:.0}s)", percent * 100.0);
                }
                Ok(RenderMessage::Complete { output }) => {
                    self.last_export_path = Some(output);
                    self.stage = Stage::PostRecord;
                    self.status = "Render complete".to_string();
                    self.render_receiver = None;
                    break;
                }
                Ok(RenderMessage::Failed { error }) => {
                    self.stage = Stage::PostRecord;
                    self.status = error;
                    self.render_receiver = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stage = Stage::PostRecord;
                    self.status = "Render worker disconnected".to_string();
                    self.render_receiver = None;
                    break;
                }
            }
        }
    }

    fn tick_countdown(&mut self) {
        if self.stage != Stage::Countdown {
            return;
        }
        let Some(started) = self.countdown_started else {
            self.stage = Stage::Idle;
            return;
        };
        let elapsed = started.elapsed().as_secs_f64();
        let total = self.countdown_preset.seconds();
        let remaining = (total - elapsed).max(0.0);

        if remaining <= 0.0 {
            self.countdown_started = None;
            self.start_recording_now();
        } else {
            self.status = format!("{remaining:.0}s");
        }
    }

    fn countdown_remaining(&self) -> f64 {
        let Some(started) = self.countdown_started else {
            return 0.0;
        };
        let total = self.countdown_preset.seconds();
        (total - started.elapsed().as_secs_f64()).max(0.0)
    }

    fn elapsed_secs(&self) -> f64 {
        self.session
            .as_ref()
            .map(|s| s.elapsed_secs())
            .unwrap_or(0.0)
    }

    fn monitor_label(&self, idx: usize) -> String {
        if let Some(m) = self.monitors.get(idx) {
            format!("{} {}x{}", m.name, m.width, m.height)
        } else {
            format!("Monitor {idx}")
        }
    }

    fn monitor_index_for_point(&self, point: Pos2) -> Option<usize> {
        if self.monitors.is_empty() {
            return None;
        }

        for (idx, monitor) in self.monitors.iter().enumerate() {
            let left = monitor.x as f32;
            let top = monitor.y as f32;
            let right = left + monitor.width as f32;
            let bottom = top + monitor.height as f32;

            if point.x >= left && point.x < right && point.y >= top && point.y < bottom {
                return Some(idx);
            }
        }

        self.monitors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let acx = a.x as f32 + a.width as f32 * 0.5;
                let acy = a.y as f32 + a.height as f32 * 0.5;
                let bcx = b.x as f32 + b.width as f32 * 0.5;
                let bcy = b.y as f32 + b.height as f32 * 0.5;

                let adx = point.x - acx;
                let ady = point.y - acy;
                let bdx = point.x - bcx;
                let bdy = point.y - bcy;

                let ad2 = adx * adx + ady * ady;
                let bd2 = bdx * bdx + bdy * bdy;
                ad2.partial_cmp(&bd2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
    }

    fn sync_selected_monitor_from_window(&mut self, ctx: &egui::Context) {
        if self.stage != Stage::Idle {
            return;
        }

        let Some(window_rect) = ctx.input(|i| i.viewport().outer_rect) else {
            return;
        };

        let Some(window_monitor) = self.monitor_index_for_point(window_rect.center()) else {
            return;
        };

        if self.last_window_monitor != Some(window_monitor) {
            self.last_window_monitor = Some(window_monitor);
            self.selected_monitor = window_monitor;
        }
    }

    fn maybe_relocate_overlay_for_recording(&mut self, ctx: &egui::Context) {
        if self.relocated_for_recording {
            return;
        }

        let Some(recording_monitor) = self.recording_monitor_index else {
            return;
        };

        if self.monitors.len() <= 1 {
            return;
        }

        let Some((target_idx, target_monitor)) = self
            .monitors
            .iter()
            .enumerate()
            .find(|(idx, _)| *idx != recording_monitor)
        else {
            return;
        };

        let current_pos = ctx.input(|i| i.viewport().outer_rect.map(|r| r.left_top()));
        if self.pre_record_outer_pos.is_none() {
            self.pre_record_outer_pos = current_pos;
        }

        let target_size = self.target_window_size();
        let margin = 20.0;
        let max_x =
            (target_monitor.x as f32 + target_monitor.width as f32 - target_size.x - margin)
                .max(target_monitor.x as f32 + margin);
        let max_y =
            (target_monitor.y as f32 + target_monitor.height as f32 - target_size.y - margin)
                .max(target_monitor.y as f32 + margin);
        let pos = Pos2::new(
            (target_monitor.x as f32 + margin).min(max_x),
            (target_monitor.y as f32 + margin).min(max_y),
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));

        self.last_window_monitor = Some(target_idx);
        self.relocated_for_recording = true;
    }

    fn maybe_restore_overlay_after_recording(&mut self, ctx: &egui::Context) {
        if !self.relocated_for_recording {
            return;
        }

        if let Some(pos) = self.pre_record_outer_pos.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }

        self.relocated_for_recording = false;
    }

    fn target_window_size(&self) -> Vec2 {
        let height = if self.stage == Stage::Idle && self.menus_open {
            BUBBLE_EXPANDED_HEIGHT
        } else {
            BUBBLE_HEIGHT
        };
        Vec2::new(self.stage.bubble_width(), height)
    }

    fn center_on_current_monitor(&self, ctx: &egui::Context, size: Vec2) {
        let monitor_size = ctx.input(|i| i.viewport().monitor_size);
        if let Some(monitor_size) = monitor_size {
            let pos = Pos2::new(
                ((monitor_size.x - size.x) * 0.5).max(0.0),
                ((monitor_size.y - size.y) * 0.5).max(0.0),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
    }

    fn resize_preserving_center(&self, ctx: &egui::Context, size: Vec2) {
        let old_center = ctx.input(|i| i.viewport().outer_rect.map(|r| r.center()));

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));

        if let Some(center) = old_center {
            let new_pos = Pos2::new(center.x - size.x * 0.5, center.y - size.y * 0.5);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(new_pos));
        }
    }

    fn resize_preserving_top_left(&self, ctx: &egui::Context, size: Vec2) {
        let old_top_left = ctx.input(|i| i.viewport().outer_rect.map(|r| r.left_top()));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        if let Some(top_left) = old_top_left {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(top_left));
        }
    }

    fn apply_window_size_if_needed(&mut self, ctx: &egui::Context) {
        let target_size = self.target_window_size();
        let size_changed = (target_size.x - self.prev_window_size.x).abs() > 0.1
            || (target_size.y - self.prev_window_size.y).abs() > 0.1;
        if !size_changed {
            return;
        }

        let only_idle_menu_toggle = self.stage == Stage::Idle
            && self.prev_stage == Stage::Idle
            && (target_size.x - self.prev_window_size.x).abs() <= 0.1;

        if only_idle_menu_toggle {
            self.resize_preserving_top_left(ctx, target_size);
        } else {
            self.resize_preserving_center(ctx, target_size);
        }

        self.prev_window_size = target_size;
    }
}

// ── UI rendering ─────────────────────────────────────────────────────────────

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint at ~16 fps – enough for animations, not wasteful
        ctx.request_repaint_after(std::time::Duration::from_millis(60));
        self.tick_countdown();
        self.poll_session_tasks();
        self.poll_render_messages();
        let preview_running = self.webcam_preview.is_running();
        if self.stage == Stage::Recording && self.webcam_preview_enabled && !preview_running {
            self.webcam_preview_enabled = false;
            if self.status.is_empty() {
                self.status = "Webcam preview stopped (device may be in use)".to_string();
            }
        }

        // Center once on startup.
        if !self.centered_once {
            let size = self.target_window_size();
            self.center_on_current_monitor(ctx, size);
            self.prev_window_size = size;
            self.centered_once = true;
        }

        if self.stage == Stage::Recording {
            self.maybe_relocate_overlay_for_recording(ctx);
        } else {
            self.maybe_restore_overlay_after_recording(ctx);
            self.sync_selected_monitor_from_window(ctx);
        }

        // ── Draw ─────────────────────────────────────────────────────────
        let expanded_background = self.stage == Stage::Idle && self.menus_open;
        self.menus_open = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let full_rect = ui.max_rect();
                let bubble_rect =
                    Rect::from_min_size(full_rect.min, Vec2::new(full_rect.width(), BUBBLE_HEIGHT));

                let expanded = expanded_background;
                let card_rect = if expanded { full_rect } else { bubble_rect };
                let card_rounding = if expanded {
                    Rounding::same(12.0)
                } else {
                    Rounding::same(BUBBLE_HEIGHT / 2.0)
                };
                let card_color = if expanded {
                    BG_EXPANDED_COLOR
                } else {
                    BG_COLOR
                };

                // Background pill
                ui.painter()
                    .rect_filled(card_rect, card_rounding, card_color);
                ui.painter()
                    .rect_stroke(card_rect, card_rounding, Stroke::new(1.0, BORDER_COLOR));

                // Drag the window from anywhere on the background
                let drag_resp =
                    ui.interact(bubble_rect, ui.id().with("drag"), Sense::click_and_drag());
                if drag_resp.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                match self.stage {
                    Stage::Idle => self.draw_idle(ui, bubble_rect),
                    Stage::Countdown => self.draw_countdown(ui, bubble_rect),
                    Stage::Starting => self.draw_starting(ui, bubble_rect),
                    Stage::Recording => self.draw_recording(ui, bubble_rect),
                    Stage::Stopping => self.draw_stopping(ui, bubble_rect),
                    Stage::PostRecord => self.draw_post_record(ui, bubble_rect),
                    Stage::Rendering => self.draw_rendering(ui, bubble_rect),
                }
            });

        self.apply_window_size_if_needed(ctx);
        self.prev_stage = self.stage;
    }
}

impl OverlayApp {
    // ── Idle: [RED CIRCLE]  [timer v]  [monitor v] ──────────────────────────

    fn draw_idle(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let cx = rect.left() + PADDING + CIRCLE_RADIUS + 2.0;
        let cy = rect.center().y;

        // Red circle (record) button
        let circle_rect =
            Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(CIRCLE_RADIUS * 2.0));
        let resp = ui.interact(circle_rect, ui.id().with("rec_btn"), Sense::click());
        ui.painter()
            .circle_filled(Pos2::new(cx, cy), CIRCLE_RADIUS, RED_IDLE);
        if resp.hovered() {
            ui.painter().circle_stroke(
                Pos2::new(cx, cy),
                CIRCLE_RADIUS + 2.0,
                Stroke::new(1.5, Color32::from_rgb(255, 120, 120)),
            );
        }
        if resp.clicked() {
            self.initiate_recording();
        }

        let row_top = rect.top() + 3.0;
        let row_height = BUBBLE_HEIGHT - 6.0;
        let row_bottom = row_top + row_height;

        let left_anchor = cx + CIRCLE_RADIUS + 10.0;
        let right_anchor = rect.right() - PADDING;

        let cam_btn_w = 30.0;
        let preview_btn_w = 34.0;
        let btn_gap = 4.0;

        let preview_rect = Rect::from_min_max(
            Pos2::new(right_anchor - preview_btn_w, row_top + 2.0),
            Pos2::new(right_anchor, row_bottom - 2.0),
        );
        let cam_rect = Rect::from_min_max(
            Pos2::new(preview_rect.left() - btn_gap - cam_btn_w, row_top + 2.0),
            Pos2::new(preview_rect.left() - btn_gap, row_bottom - 2.0),
        );

        let mut timer_width = 64.0;
        let min_monitor_width = 68.0;
        let monitor_right = cam_rect.left() - btn_gap;
        let mut monitor_left = left_anchor + timer_width + btn_gap;
        let mut monitor_width = monitor_right - monitor_left;

        if monitor_width < min_monitor_width {
            let needed = min_monitor_width - monitor_width;
            timer_width = (timer_width - needed).max(48.0);
            monitor_left = left_anchor + timer_width + btn_gap;
            monitor_width = monitor_right - monitor_left;
        }

        monitor_width = monitor_width.max(40.0);

        // Timer dropdown
        let timer_rect = Rect::from_min_size(
            Pos2::new(left_anchor, row_top),
            Vec2::new(timer_width.max(48.0), row_height),
        );
        let mut timer_child =
            ui.child_ui(timer_rect, egui::Layout::left_to_right(egui::Align::Center));
        let timer_id = timer_child.make_persistent_id("timer_cb");
        egui::ComboBox::from_id_source("timer_cb")
            .width((timer_rect.width() - 6.0).max(40.0))
            .height(DROPDOWN_MAX_HEIGHT)
            .selected_text(self.countdown_preset.label())
            .show_ui(&mut timer_child, |ui: &mut egui::Ui| {
                for preset in CountdownPreset::ALL {
                    ui.selectable_value(&mut self.countdown_preset, preset, preset.label());
                }
            });
        let timer_open = timer_child.memory(|m| m.is_popup_open(timer_id.with("popup")));

        // Monitor dropdown
        let mon_rect = Rect::from_min_size(
            Pos2::new(monitor_left, row_top),
            Vec2::new(monitor_width, row_height),
        );
        let mut monitor_child =
            ui.child_ui(mon_rect, egui::Layout::left_to_right(egui::Align::Center));
        let monitor_id = monitor_child.make_persistent_id("monitor_cb");
        let sel_label = ellipsize_label(
            &self.monitor_label(self.selected_monitor),
            ((mon_rect.width() / 7.0) as usize).clamp(8, 28),
        );
        egui::ComboBox::from_id_source("monitor_cb")
            .width((mon_rect.width() - 6.0).max(44.0))
            .height(DROPDOWN_MAX_HEIGHT)
            .selected_text(&sel_label)
            .show_ui(&mut monitor_child, |ui: &mut egui::Ui| {
                for i in 0..self.monitors.len() {
                    let label = self.monitor_label(i);
                    ui.selectable_value(&mut self.selected_monitor, i, label);
                }
            });

        let cam_resp = ui.interact(cam_rect, ui.id().with("cam_toggle"), Sense::click());
        let cam_color = if self.webcam {
            ACCENT.linear_multiply(0.95)
        } else {
            Color32::from_rgb(217, 226, 240)
        };
        let cam_text_color = if self.webcam {
            Color32::WHITE
        } else {
            Color32::from_rgb(74, 92, 116)
        };
        ui.painter()
            .rect_filled(cam_rect, Rounding::same(6.0), cam_color);
        ui.painter().text(
            cam_rect.center(),
            egui::Align2::CENTER_CENTER,
            "CAM",
            egui::FontId::proportional(9.0),
            cam_text_color,
        );
        if cam_resp.clicked() {
            self.webcam = !self.webcam;
            if !self.webcam {
                self.webcam_preview.stop();
                self.webcam_preview_enabled = false;
            }
        }

        let preview_enabled = self.webcam && self.webcam_preview_enabled;
        let preview_color = if preview_enabled {
            Color32::from_rgb(69, 156, 242)
        } else if self.webcam {
            Color32::from_rgb(181, 211, 242)
        } else {
            Color32::from_rgb(217, 226, 240)
        };
        let preview_text_color = if preview_enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(74, 92, 116)
        };
        let preview_resp =
            ui.interact(preview_rect, ui.id().with("preview_toggle"), Sense::click());
        ui.painter()
            .rect_filled(preview_rect, Rounding::same(6.0), preview_color);
        ui.painter().text(
            preview_rect.center(),
            egui::Align2::CENTER_CENTER,
            "PIP",
            egui::FontId::proportional(9.0),
            preview_text_color,
        );
        if preview_resp.clicked() && self.webcam {
            self.webcam_preview_enabled = !self.webcam_preview_enabled;
            if !self.webcam_preview_enabled {
                self.webcam_preview.stop();
            } else if self.stage == Stage::Recording {
                if let Err(err) = self.webcam_preview.start() {
                    self.status = format!("Webcam preview unavailable: {err} (device may be busy)");
                    self.webcam_preview_enabled = false;
                }
            }
        }

        let monitor_open = monitor_child.memory(|m| m.is_popup_open(monitor_id.with("popup")));
        self.menus_open = timer_open || monitor_open;
    }

    // ── Countdown: [pulsing number]  "Starting in Xs" ───────────────────────

    fn draw_countdown(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let cx = rect.left() + PADDING + CIRCLE_RADIUS + 2.0;
        let cy = rect.center().y;
        let remaining = self.countdown_remaining();

        // Pulsing circle
        let t = (remaining.fract() * std::f64::consts::TAU).sin().abs() as f32;
        let color = lerp_color(RED_PULSE_DIM, RED_RECORDING, t);
        ui.painter()
            .circle_filled(Pos2::new(cx, cy), CIRCLE_RADIUS, color);

        // Number inside
        ui.painter().text(
            Pos2::new(cx, cy),
            egui::Align2::CENTER_CENTER,
            format!("{}", remaining.ceil() as u32),
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );

        // Click to cancel
        let circle_rect =
            Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(CIRCLE_RADIUS * 2.0));
        if ui
            .interact(circle_rect, ui.id().with("cancel_btn"), Sense::click())
            .clicked()
        {
            self.countdown_started = None;
            self.stage = Stage::Idle;
            self.status = String::new();
        }

        ui.painter().text(
            Pos2::new(cx + CIRCLE_RADIUS + 10.0, cy),
            egui::Align2::LEFT_CENTER,
            format!("Starting in {:.0}s...", remaining),
            egui::FontId::proportional(12.0),
            TEXT_DIM,
        );
    }

    // ── Recording: compact stop control + timer ─────────────────────────────

    fn draw_recording(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let cx = rect.left() + PADDING + CIRCLE_RADIUS + 2.0;
        let cy = rect.center().y;
        let elapsed = self.elapsed_secs();

        // Pulsing dot
        let t = ((elapsed * 2.0).sin().abs()) as f32;
        let color = lerp_color(RED_PULSE_DIM, RED_RECORDING, t);
        let r = CIRCLE_RADIUS * 0.7;
        ui.painter().circle_filled(Pos2::new(cx, cy), r, color);

        // Stop button
        let dot_rect = Rect::from_center_size(Pos2::new(cx, cy), Vec2::splat(r * 2.0));
        let resp = ui.interact(dot_rect, ui.id().with("stop_dot"), Sense::click());
        if resp.hovered() {
            ui.painter().circle_stroke(
                Pos2::new(cx, cy),
                r + 1.8,
                Stroke::new(1.3, RED_RECORDING.linear_multiply(0.9)),
            );
        }
        if resp.clicked() {
            self.stop_recording();
        }

        ui.painter().text(
            Pos2::new(cx + CIRCLE_RADIUS + 10.0, cy),
            egui::Align2::LEFT_CENTER,
            format!("REC {}", format_elapsed_mm_ss(elapsed)),
            egui::FontId::proportional(11.0),
            TEXT_COLOR,
        );
    }

    fn draw_starting(&self, ui: &mut egui::Ui, rect: Rect) {
        let cy = rect.center().y;
        let cx = rect.left() + PADDING + CIRCLE_RADIUS + 2.0;

        ui.painter()
            .circle_filled(Pos2::new(cx, cy), CIRCLE_RADIUS * 0.7, RED_IDLE);
        ui.painter().text(
            Pos2::new(cx + CIRCLE_RADIUS + 10.0, cy),
            egui::Align2::LEFT_CENTER,
            "Starting...",
            egui::FontId::proportional(12.0),
            TEXT_DIM,
        );
    }

    fn draw_stopping(&self, ui: &mut egui::Ui, rect: Rect) {
        let cx = rect.center().x;
        let cy = rect.center().y;

        ui.painter()
            .circle_filled(Pos2::new(cx, cy), CIRCLE_RADIUS * 0.7, RED_PULSE_DIM);
    }

    // ── PostRecord: [Auto-Direct]  [Render]  [New] ──────────────────────────

    fn draw_post_record(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let cy = rect.center().y;
        let btn_h = BUBBLE_HEIGHT - 12.0;
        let btn_y = rect.top() + 6.0;
        let mut x = rect.left() + PADDING + 2.0;

        x = self.draw_pill_button(ui, x, btn_y, btn_h, "Auto-Direct", ACCENT, "ad_btn");
        x += 4.0;
        x = self.draw_pill_button(
            ui,
            x,
            btn_y,
            btn_h,
            "Render",
            Color32::from_rgb(80, 200, 120),
            "render_btn",
        );
        x += 4.0;
        self.draw_pill_button(
            ui,
            x,
            btn_y,
            btn_h,
            "New",
            Color32::from_rgb(100, 100, 120),
            "new_btn",
        );

        if !self.status.is_empty() {
            ui.painter().text(
                Pos2::new(rect.right() - PADDING, cy),
                egui::Align2::RIGHT_CENTER,
                &self.status,
                egui::FontId::proportional(10.0),
                TEXT_DIM,
            );
        }
    }

    // ── Rendering: progress bar ─────────────────────────────────────────────

    fn draw_rendering(&self, ui: &mut egui::Ui, rect: Rect) {
        let cy = rect.center().y;
        let bar_left = rect.left() + PADDING + 2.0;
        let bar_right = rect.right() - 70.0;
        let bar_h = 6.0;
        let bar_rect = Rect::from_min_max(
            Pos2::new(bar_left, cy - bar_h / 2.0),
            Pos2::new(bar_right, cy + bar_h / 2.0),
        );

        ui.painter().rect_filled(
            bar_rect,
            Rounding::same(3.0),
            Color32::from_rgb(212, 222, 236),
        );

        let fill_w = bar_rect.width() * self.render_percent as f32;
        let fill_rect = Rect::from_min_max(
            bar_rect.left_top(),
            Pos2::new(bar_rect.left() + fill_w, bar_rect.bottom()),
        );
        ui.painter()
            .rect_filled(fill_rect, Rounding::same(3.0), ACCENT);

        ui.painter().text(
            Pos2::new(rect.right() - PADDING, cy),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}%", self.render_percent * 100.0),
            egui::FontId::proportional(12.0),
            TEXT_COLOR,
        );
    }

    // ── Pill button helper ──────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn draw_pill_button(
        &mut self,
        ui: &mut egui::Ui,
        x: f32,
        y: f32,
        h: f32,
        label: &str,
        color: Color32,
        id_str: &str,
    ) -> f32 {
        let text_width = ui
            .painter()
            .layout_no_wrap(label.to_string(), egui::FontId::proportional(11.0), color)
            .rect
            .width();
        let w = text_width + 16.0;
        let btn_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        let resp = ui.interact(btn_rect, ui.id().with(id_str), Sense::click());

        let bg = if resp.hovered() {
            Color32::from_rgb(230, 238, 248)
        } else {
            Color32::from_rgb(241, 246, 252)
        };
        ui.painter()
            .rect_filled(btn_rect, Rounding::same(h / 2.0), bg);
        ui.painter().rect_stroke(
            btn_rect,
            Rounding::same(h / 2.0),
            Stroke::new(1.0, color.linear_multiply(0.4)),
        );
        ui.painter().text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            color,
        );

        if resp.clicked() {
            match id_str {
                "ad_btn" => self.run_auto_direct(),
                "render_btn" => self.start_render(),
                "new_btn" => {
                    self.webcam_preview.stop();
                    self.stage = Stage::Idle;
                    self.active_project_path = None;
                    self.last_export_path = None;
                    self.status = String::new();
                    self.recording_monitor_index = None;
                }
                _ => {}
            }
        }

        x + w
    }
}

// ── Color lerp ───────────────────────────────────────────────────────────────

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn format_elapsed_mm_ss(elapsed_secs: f64) -> String {
    let total_secs = elapsed_secs.max(0.0).floor() as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

fn ellipsize_label(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let keep = max_chars.saturating_sub(3).max(1);
    let prefix: String = text.chars().take(keep).collect();
    format!("{prefix}...")
}

// ── Auto-Director ────────────────────────────────────────────────────────────

fn auto_direct_project(project_path: &Path) -> anyhow::Result<usize> {
    let mut loaded = LoadedProject::load(project_path)
        .map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    let events_path = project_path.join("meta").join("events.jsonl");
    let events_raw = std::fs::read_to_string(&events_path)
        .map_err(|e| anyhow::anyhow!("Failed to read events: {e}"))?;
    let events =
        parse_events(&events_raw).map_err(|e| anyhow::anyhow!("Failed to parse events: {e}"))?;

    let prepared_events =
        remap_events_for_auto_director(&events, &events_raw, &loaded.project.recording);

    if prepared_events.is_empty() {
        loaded.timeline.keyframes = vec![CameraKeyframe {
            time_secs: 0.0,
            viewport: Viewport::FULL,
            easing: EasingFunction::EaseInOut,
            source: KeyframeSource::Auto,
        }];
    } else {
        let config = AutoZoomConfig {
            dwell_threshold_secs: 1.25,
            dwell_radius: 0.09,
            hover_zoom: 0.90,
            scan_zoom: 1.0,
            smoothing_window: 5,
            min_viewport_size: 0.85,
            dwell_velocity_threshold: 0.12,
            monitor_count: 1,
            focused_monitor_index: 0,
            ..Default::default()
        };

        let analyzer = AutoZoomAnalyzer::new(config);
        let mut timeline = analyzer.analyze(&prepared_events);
        clamp_timeline_to_visible_bounds(&mut timeline.keyframes, 0.85);
        loaded.timeline.keyframes = timeline.keyframes;
    }

    loaded
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save timeline: {e}"))?;
    Ok(loaded.timeline.keyframes.len())
}

fn remap_events_for_auto_director(
    events: &[InputEvent],
    raw_events_file: &str,
    recording: &RecordingConfig,
) -> Vec<InputEvent> {
    let header_space = parse_events_header(raw_events_file)
        .map(|header| header.pointer_coordinate_space)
        .filter(|space| *space != PointerCoordinateSpace::LegacyUnspecified);

    let preferred_space = header_space.or_else(|| {
        let space = recording.pointer_coordinate_space;
        if space == PointerCoordinateSpace::LegacyUnspecified {
            None
        } else {
            Some(space)
        }
    });

    let pointer_space = select_pointer_space_for_auto_director(preferred_space, events, recording);
    remap_pointer_events(events, recording, pointer_space)
}

fn parse_events_header(raw_events_file: &str) -> Option<EventStreamHeader> {
    let line = raw_events_file
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('#'))?;
    serde_json::from_str::<EventStreamHeader>(line.trim_start_matches('#').trim()).ok()
}

fn select_pointer_space_for_auto_director(
    preferred: Option<PointerCoordinateSpace>,
    events: &[InputEvent],
    recording: &RecordingConfig,
) -> PointerCoordinateSpace {
    const MIN_VALID_RATIO: f64 = 0.55;

    if let Some(space) = preferred {
        if pointer_space_in_bounds_ratio(space, events, recording) >= MIN_VALID_RATIO {
            return space;
        }
    }

    let mut candidates = vec![PointerCoordinateSpace::CaptureNormalized];
    if recording.monitor_width > 0
        && recording.monitor_height > 0
        && recording.virtual_width > 0
        && recording.virtual_height > 0
    {
        candidates.push(PointerCoordinateSpace::VirtualDesktopNormalized);
        if recording.virtual_x != 0 || recording.virtual_y != 0 {
            candidates.push(PointerCoordinateSpace::VirtualDesktopRootOrigin);
        }
    }

    let mut best = PointerCoordinateSpace::CaptureNormalized;
    let mut best_score = f64::NEG_INFINITY;
    for candidate in candidates {
        let score = pointer_space_score(candidate, events, recording);
        if score > best_score {
            best = candidate;
            best_score = score;
        }
    }

    best
}

fn pointer_space_score(
    space: PointerCoordinateSpace,
    events: &[InputEvent],
    recording: &RecordingConfig,
) -> f64 {
    let mut sampled = 0usize;
    let mut in_bounds = 0usize;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let sample_stride = ((events.len() as f64) / 800.0).ceil() as usize;
    let sample_stride = sample_stride.max(1);

    for (idx, event) in events.iter().enumerate() {
        if idx % sample_stride != 0 {
            continue;
        }

        let Some((x, y)) = event.pointer_position() else {
            continue;
        };

        let Some((mx, my)) = map_pointer_to_recorded_monitor(x, y, recording, space) else {
            continue;
        };

        sampled += 1;
        if (0.0..=1.0).contains(&mx) && (0.0..=1.0).contains(&my) {
            in_bounds += 1;
            min_x = min_x.min(mx);
            max_x = max_x.max(mx);
            min_y = min_y.min(my);
            max_y = max_y.max(my);
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

    in_bounds_ratio * 3.0 + span_x + span_y
}

fn pointer_space_in_bounds_ratio(
    space: PointerCoordinateSpace,
    events: &[InputEvent],
    recording: &RecordingConfig,
) -> f64 {
    let mut sampled = 0usize;
    let mut in_bounds = 0usize;

    for event in events {
        let Some((x, y)) = event.pointer_position() else {
            continue;
        };

        let Some((mx, my)) = map_pointer_to_recorded_monitor(x, y, recording, space) else {
            continue;
        };

        sampled += 1;
        if (0.0..=1.0).contains(&mx) && (0.0..=1.0).contains(&my) {
            in_bounds += 1;
        }
    }

    if sampled == 0 {
        0.0
    } else {
        in_bounds as f64 / sampled as f64
    }
}

fn remap_pointer_events(
    events: &[InputEvent],
    recording: &RecordingConfig,
    pointer_space: PointerCoordinateSpace,
) -> Vec<InputEvent> {
    const EDGE_TOLERANCE: f64 = 0.04;

    events
        .iter()
        .filter_map(|event| {
            let remap_point = |x: f64, y: f64| -> Option<(f64, f64)> {
                let (mx, my) = map_pointer_to_recorded_monitor(x, y, recording, pointer_space)?;
                if mx < -EDGE_TOLERANCE
                    || mx > 1.0 + EDGE_TOLERANCE
                    || my < -EDGE_TOLERANCE
                    || my > 1.0 + EDGE_TOLERANCE
                {
                    return None;
                }
                Some((mx.clamp(0.0, 1.0), my.clamp(0.0, 1.0)))
            };

            let kind = match &event.kind {
                EventKind::Pointer { x, y } => {
                    let (nx, ny) = remap_point(*x, *y)?;
                    EventKind::Pointer { x: nx, y: ny }
                }
                EventKind::Click {
                    button,
                    state,
                    x,
                    y,
                } => {
                    let (nx, ny) = remap_point(*x, *y)?;
                    EventKind::Click {
                        button: *button,
                        state: *state,
                        x: nx,
                        y: ny,
                    }
                }
                EventKind::Scroll { dx, dy, x, y } => {
                    let (nx, ny) = remap_point(*x, *y)?;
                    EventKind::Scroll {
                        dx: *dx,
                        dy: *dy,
                        x: nx,
                        y: ny,
                    }
                }
                EventKind::Key { code, state } => EventKind::Key {
                    code: code.clone(),
                    state: *state,
                },
                EventKind::WindowFocus {
                    window_title,
                    app_id,
                } => EventKind::WindowFocus {
                    window_title: window_title.clone(),
                    app_id: app_id.clone(),
                },
            };

            Some(InputEvent {
                timestamp_ns: event.timestamp_ns,
                kind,
            })
        })
        .collect()
}

fn map_pointer_to_recorded_monitor(
    x: f64,
    y: f64,
    recording: &RecordingConfig,
    pointer_space: PointerCoordinateSpace,
) -> Option<(f64, f64)> {
    match pointer_space {
        PointerCoordinateSpace::CaptureNormalized | PointerCoordinateSpace::LegacyUnspecified => {
            Some((x, y))
        }
        PointerCoordinateSpace::VirtualDesktopNormalized => {
            if recording.monitor_width == 0
                || recording.monitor_height == 0
                || recording.virtual_width == 0
                || recording.virtual_height == 0
            {
                return Some((x, y));
            }

            let monitor_w = recording.monitor_width as f64;
            let monitor_h = recording.monitor_height as f64;
            let virtual_w = recording.virtual_width as f64;
            let virtual_h = recording.virtual_height as f64;

            let pixel_x = recording.virtual_x as f64 + x * virtual_w;
            let pixel_y = recording.virtual_y as f64 + y * virtual_h;
            Some((
                (pixel_x - recording.monitor_x as f64) / monitor_w,
                (pixel_y - recording.monitor_y as f64) / monitor_h,
            ))
        }
        PointerCoordinateSpace::VirtualDesktopRootOrigin => {
            if recording.monitor_width == 0
                || recording.monitor_height == 0
                || recording.virtual_width == 0
                || recording.virtual_height == 0
            {
                return Some((x, y));
            }

            let monitor_w = recording.monitor_width as f64;
            let monitor_h = recording.monitor_height as f64;
            let virtual_w = recording.virtual_width as f64;
            let virtual_h = recording.virtual_height as f64;

            let pixel_x = x * virtual_w;
            let pixel_y = y * virtual_h;
            Some((
                (pixel_x - recording.monitor_x as f64) / monitor_w,
                (pixel_y - recording.monitor_y as f64) / monitor_h,
            ))
        }
    }
}

fn clamp_timeline_to_visible_bounds(keyframes: &mut [CameraKeyframe], min_viewport: f64) {
    for keyframe in keyframes {
        let width = keyframe.viewport.w.clamp(min_viewport, 1.0);
        let height = keyframe.viewport.h.clamp(min_viewport, 1.0);
        let x = keyframe.viewport.x.clamp(0.0, (1.0 - width).max(0.0));
        let y = keyframe.viewport.y.clamp(0.0, (1.0 - height).max(0.0));
        keyframe.viewport = Viewport::new(x, y, width, height);
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    // Disable automatic HiDPI scaling so dimensions stay predictable.
    // The user can still drag the window; we just don't want the WM
    // doubling our already-compact pixel sizes.
    std::env::set_var("WINIT_X11_SCALE_FACTOR", "1");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GrabMe")
            .with_always_on_top()
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([BUBBLE_WIDTH_IDLE, BUBBLE_HEIGHT])
            .with_resizable(false)
            // X11: mark as Utility so compositors / screen-recorders can
            // filter it out of the capture. PipeWire portal captures also
            // honour _NET_WM_WINDOW_TYPE for some compositors (e.g. KWin).
            .with_window_type(egui::X11WindowType::Utility)
            // Hide from taskbar so the bubble is unobtrusive.
            .with_taskbar(false),
        ..Default::default()
    };

    eframe::run_native(
        "GrabMe",
        options,
        Box::new(|cc| {
            // Force 1.0 pixels-per-point so egui doesn't scale up on HiDPI.
            cc.egui_ctx.set_pixels_per_point(1.0);
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Box::new(OverlayApp::default())
        }),
    )
    .map_err(|e| anyhow::anyhow!("overlay failed: {e}"))
}
