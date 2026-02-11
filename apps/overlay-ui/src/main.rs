use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Color32};
use grabme_capture_engine::{
    AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig, SessionConfig,
    SessionState,
};
use grabme_processing_core::auto_zoom::{AutoZoomAnalyzer, AutoZoomConfig};
use grabme_project_model::event::parse_events;
use grabme_project_model::project::{AspectMode, ExportConfig, ExportFormat, LoadedProject};
use grabme_project_model::timeline::{CameraKeyframe, EasingFunction, KeyframeSource};
use grabme_project_model::viewport::Viewport;
use grabme_render_engine::export::{export_project, ExportJob, ExportProgress};

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GrabMe Overlay")
            .with_always_on_top()
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([460.0, 270.0]),
        ..Default::default()
    };

    eframe::run_native(
        "GrabMe Overlay",
        options,
        Box::new(|_cc| Box::new(OverlayRecorderApp::default())),
    )
    .map_err(|e| anyhow::anyhow!("overlay launch failed: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkflowStage {
    Ready,
    Recording,
    PostRecord,
    Rendering,
}

#[derive(Debug)]
enum RenderMessage {
    Progress {
        percent: f64,
        frames_rendered: u64,
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

struct OverlayRecorderApp {
    runtime: tokio::runtime::Runtime,
    session: Option<CaptureSession>,
    stage: WorkflowStage,
    project_name: String,
    output_dir: String,
    fps: u32,
    monitor_index: usize,
    mic: bool,
    system_audio: bool,
    webcam: bool,
    status: String,
    active_project_path: Option<PathBuf>,
    last_export_path: Option<PathBuf>,
    render_receiver: Option<Receiver<RenderMessage>>,
    render_percent: f64,
    render_eta_secs: f64,
    render_frames_rendered: u64,
    render_total_frames: u64,
}

impl Default for OverlayRecorderApp {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime should initialize");

        Self {
            runtime,
            session: None,
            stage: WorkflowStage::Ready,
            project_name: "recording".to_string(),
            output_dir: ".".to_string(),
            fps: 60,
            monitor_index: 0,
            mic: true,
            system_audio: true,
            webcam: false,
            status: "Ready".to_string(),
            active_project_path: None,
            last_export_path: None,
            render_receiver: None,
            render_percent: 0.0,
            render_eta_secs: 0.0,
            render_frames_rendered: 0,
            render_total_frames: 0,
        }
    }
}

impl OverlayRecorderApp {
    fn build_session_config(&self) -> SessionConfig {
        SessionConfig {
            name: self.project_name.trim().to_string(),
            output_dir: PathBuf::from(self.output_dir.trim()),
            screen: ScreenCaptureConfig {
                mode: CaptureMode::FullScreen {
                    monitor_index: self.monitor_index,
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

    fn start_recording(&mut self) {
        if self.project_name.trim().is_empty() {
            self.status = "Project name is required".to_string();
            return;
        }

        let mut session = CaptureSession::new(self.build_session_config());
        match self.runtime.block_on(session.start()) {
            Ok(()) => {
                self.status = "Recording started".to_string();
                self.last_export_path = None;
                self.active_project_path = None;
                self.stage = WorkflowStage::Recording;
                self.session = Some(session);
            }
            Err(err) => {
                self.status = format!("Failed to start: {err}");
            }
        }
    }

    fn stop_recording(&mut self) {
        let Some(mut session) = self.session.take() else {
            self.status = "Not recording".to_string();
            return;
        };

        match self.runtime.block_on(session.stop()) {
            Ok(path) => {
                self.status = "Recording stopped. Ready for Auto-Direct or Render".to_string();
                self.active_project_path = Some(path);
                self.stage = WorkflowStage::PostRecord;
            }
            Err(err) => {
                self.status = format!("Failed to stop: {err}");
                self.stage = WorkflowStage::Ready;
            }
        }
    }

    fn run_auto_direct(&mut self) {
        let Some(project_path) = self.active_project_path.as_ref() else {
            self.status = "No project to auto-direct".to_string();
            return;
        };

        match auto_direct_project(project_path) {
            Ok(keyframe_count) => {
                self.status =
                    format!("Auto-Director complete: {keyframe_count} keyframes generated");
            }
            Err(err) => {
                self.status = format!("Auto-Director failed: {err}");
            }
        }
    }

    fn start_render(&mut self) {
        let Some(project_path) = self.active_project_path.as_ref() else {
            self.status = "No project to render".to_string();
            return;
        };

        let project_path = project_path.clone();
        let (tx, rx) = mpsc::channel::<RenderMessage>();
        self.render_receiver = Some(rx);
        self.render_percent = 0.0;
        self.render_eta_secs = 0.0;
        self.render_frames_rendered = 0;
        self.render_total_frames = 0;
        self.stage = WorkflowStage::Rendering;
        self.status = "Render started...".to_string();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    let _ = tx.send(RenderMessage::Failed {
                        error: format!("Failed to create runtime: {err}"),
                    });
                    return;
                }
            };

            let result = runtime.block_on(async {
                let loaded = LoadedProject::load(&project_path)
                    .map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

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
                    percent,
                    frames_rendered,
                    total_frames,
                    eta_secs,
                }) => {
                    self.render_percent = percent;
                    self.render_frames_rendered = frames_rendered;
                    self.render_total_frames = total_frames;
                    self.render_eta_secs = eta_secs;
                    self.status = format!(
                        "Rendering {:.1}% ({} / {} frames, ETA {:.0}s)",
                        percent * 100.0,
                        frames_rendered,
                        total_frames,
                        eta_secs
                    );
                }
                Ok(RenderMessage::Complete { output }) => {
                    self.last_export_path = Some(output.clone());
                    self.stage = WorkflowStage::PostRecord;
                    self.status = format!("Render complete: {}", output.display());
                    self.render_receiver = None;
                    break;
                }
                Ok(RenderMessage::Failed { error }) => {
                    self.stage = WorkflowStage::PostRecord;
                    self.status = error;
                    self.render_receiver = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stage = WorkflowStage::PostRecord;
                    self.status = "Render worker disconnected".to_string();
                    self.render_receiver = None;
                    break;
                }
            }
        }
    }

    fn is_recording(&self) -> bool {
        self.session
            .as_ref()
            .map(|session| session.state() == SessionState::Recording)
            .unwrap_or(false)
    }
}

impl eframe::App for OverlayRecorderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        self.poll_render_messages();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(18, 20, 24, 220))
                    .rounding(egui::Rounding::same(12.0))
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(70))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("GrabMe Recorder");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("x").clicked() {
                            if self.is_recording() {
                                self.stop_recording();
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Project");
                    ui.text_edit_singleline(&mut self.project_name);
                });
                ui.horizontal(|ui| {
                    ui.label("Output");
                    ui.text_edit_singleline(&mut self.output_dir);
                });
                ui.horizontal(|ui| {
                    ui.label("FPS");
                    ui.add(egui::Slider::new(&mut self.fps, 24..=120));
                });
                ui.horizontal(|ui| {
                    ui.label("Monitor");
                    ui.add(egui::Slider::new(&mut self.monitor_index, 0..=7));
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.mic, "Mic");
                    ui.checkbox(&mut self.system_audio, "System Audio");
                    ui.checkbox(&mut self.webcam, "Webcam");
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| match self.stage {
                    WorkflowStage::Ready => {
                        let button = egui::Button::new("Record")
                            .fill(Color32::from_rgb(200, 52, 52))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(230, 120, 120)));
                        if ui.add(button).clicked() {
                            self.start_recording();
                        }
                    }
                    WorkflowStage::Recording => {
                        let button = egui::Button::new("Stop")
                            .fill(Color32::from_rgb(60, 72, 88))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(110, 130, 160)));
                        if ui.add(button).clicked() {
                            self.stop_recording();
                        }
                        if let Some(session) = self.session.as_ref() {
                            ui.colored_label(
                                Color32::from_rgb(255, 120, 120),
                                format!("REC {:.1}s", session.elapsed_secs()),
                            );
                        }
                    }
                    WorkflowStage::PostRecord => {
                        if ui.button("Auto-Direct").clicked() {
                            self.run_auto_direct();
                        }
                        if ui.button("Render").clicked() {
                            self.start_render();
                        }
                        if ui.button("New Recording").clicked() {
                            self.stage = WorkflowStage::Ready;
                            self.active_project_path = None;
                            self.last_export_path = None;
                            self.status = "Ready".to_string();
                        }
                    }
                    WorkflowStage::Rendering => {
                        ui.add(
                            egui::ProgressBar::new(self.render_percent as f32)
                                .desired_width(160.0)
                                .text(format!("{:.1}%", self.render_percent * 100.0)),
                        );
                        if ui.button("Render In Progress...").clicked() {}
                    }
                });

                ui.label(format!("Status: {}", self.status));
                if let Some(path) = self.active_project_path.as_ref() {
                    ui.label(format!("Project: {}", path.display()));
                }
                if let Some(path) = self.last_export_path.as_ref() {
                    ui.label(format!("Export: {}", path.display()));
                }
            });
    }
}

fn auto_direct_project(project_path: &Path) -> anyhow::Result<usize> {
    let mut loaded = LoadedProject::load(project_path)
        .map_err(|e| anyhow::anyhow!("Failed to load project: {e}"))?;

    let events_path = project_path.join("meta").join("events.jsonl");
    let events_raw = std::fs::read_to_string(&events_path)
        .map_err(|e| anyhow::anyhow!("Failed to read events at {}: {e}", events_path.display()))?;
    let events = parse_events(&events_raw)
        .map_err(|e| anyhow::anyhow!("Failed to parse events for auto-direct: {e}"))?;

    if events.is_empty() {
        loaded.timeline.keyframes = vec![CameraKeyframe {
            time_secs: 0.0,
            viewport: Viewport::FULL,
            easing: EasingFunction::EaseInOut,
            source: KeyframeSource::Auto,
        }];
    } else {
        let config = AutoZoomConfig {
            monitor_count: 1,
            focused_monitor_index: loaded.project.recording.monitor_index,
            ..Default::default()
        };
        let analyzer = AutoZoomAnalyzer::new(config);
        let timeline = analyzer.analyze(&events);
        loaded.timeline.keyframes = timeline.keyframes;
    }

    loaded
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save auto-directed timeline: {e}"))?;

    Ok(loaded.timeline.keyframes.len())
}
