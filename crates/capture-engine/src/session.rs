//! Recording session management.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use grabme_common::clock::{DriftMeasurement, RecordingClock};
use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_input_tracker::backends::detect_best_backend;
use grabme_input_tracker::InputTracker;
use grabme_platform_core::{virtual_desktop_bounds, MonitorInfo};
use grabme_project_model::event::PointerCoordinateSpace;
use grabme_project_model::project::RecordedMonitor;
use grabme_project_model::{LoadedProject, TrackRef};

use crate::backend::{get_backend, CaptureBackend};
use crate::pipeline::CapturePipeline;

/// Configuration for starting a new recording session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Project name.
    pub name: String,

    /// Directory to create the project in.
    pub output_dir: PathBuf,

    /// Screen capture settings.
    pub screen: ScreenCaptureConfig,

    /// Audio capture settings.
    pub audio: AudioCaptureConfig,

    /// Whether to capture webcam.
    pub webcam: bool,

    /// Target FPS for screen capture.
    pub fps: u32,

    /// Pointer sampling rate in Hz.
    pub pointer_sample_rate_hz: u32,
}

/// Screen capture configuration.
#[derive(Debug, Clone)]
pub struct ScreenCaptureConfig {
    /// Capture mode.
    pub mode: CaptureMode,

    /// Whether to hide the system cursor from the capture.
    pub hide_cursor: bool,
}

/// What region of the screen to capture.
#[derive(Debug, Clone)]
pub enum CaptureMode {
    /// Entire screen / monitor.
    FullScreen { monitor_index: usize },
    /// A specific window.
    Window { window_id: String },
    /// A rectangular region.
    Region {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

/// Audio capture configuration.
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    /// Capture microphone audio.
    pub mic: bool,

    /// Capture system/desktop audio.
    pub system: bool,

    /// Per-app audio isolation (app name or PID).
    pub app_isolation: Option<String>,

    /// Sample rate.
    pub sample_rate: u32,
}

/// State of a recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created but not started.
    Idle,
    /// Recording in progress.
    Recording,
    /// Recording paused.
    Paused,
    /// Recording stopped, project finalized.
    Stopped,
    /// An error occurred.
    Error,
}

/// A recording session that coordinates all capture streams.
pub struct CaptureSession {
    config: SessionConfig,
    state: SessionState,
    clock: Option<RecordingClock>,
    project: Option<LoadedProject>,
    stop_flag: Arc<AtomicBool>,
    backend: Box<dyn CaptureBackend>,
    screen_pipeline: Option<Box<dyn CapturePipeline>>,
    webcam_pipeline: Option<Box<dyn CapturePipeline>>,
    mic_pipeline: Option<Box<dyn CapturePipeline>>,
    system_pipeline: Option<Box<dyn CapturePipeline>>,
    input_stop_flag: Option<Arc<AtomicBool>>,
    input_task: Option<tokio::task::JoinHandle<GrabmeResult<u64>>>,
    stream_offsets_ns: StreamOffsets,
}

#[derive(Debug, Default, Clone, Copy)]
struct StreamOffsets {
    screen_ns: i64,
    webcam_ns: i64,
    mic_ns: i64,
    system_ns: i64,
    events_ns: i64,
}

impl CaptureSession {
    /// Create a new capture session with the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: SessionState::Idle,
            clock: None,
            project: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            backend: get_backend(),
            screen_pipeline: None,
            webcam_pipeline: None,
            mic_pipeline: None,
            system_pipeline: None,
            input_stop_flag: None,
            input_task: None,
            stream_offsets_ns: StreamOffsets::default(),
        }
    }

    /// Current session state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Start recording.
    ///
    /// This initializes the project on disk, starts all capture pipelines,
    /// and begins logging input events.
    pub async fn start(&mut self) -> GrabmeResult<()> {
        if self.state != SessionState::Idle {
            return Err(GrabmeError::capture("Session already started"));
        }

        tracing::info!(name = %self.config.name, "Starting capture session");

        // Initialize backend (detect display server, permissions)
        self.backend.init().await?;

        let monitors = self.backend.detect_monitors().unwrap_or_default();
        let selected_monitor = self.resolve_selected_monitor(&monitors)?;

        // Prepare screen capture (negotiate portals, etc.)
        let (capture_width, capture_height) = self
            .backend
            .prepare_screen_capture(&self.config.screen)
            .await?;

        // Create project on disk
        let project_dir = self.config.output_dir.join(&self.config.name);
        let mut project = LoadedProject::create(
            &project_dir,
            &self.config.name,
            capture_width,
            capture_height,
            self.config.fps,
        )
        .map_err(|e| GrabmeError::capture(format!("Failed to create project: {e}")))?;

        project.project.recording.monitor_index = self.selected_monitor_index();
        project.project.recording.pointer_coordinate_space =
            PointerCoordinateSpace::VirtualDesktopNormalized;

        if let Some(monitor) = selected_monitor.as_ref() {
            project.project.recording.monitor_name = monitor.name.clone();
            project.project.recording.monitor_x = monitor.x;
            project.project.recording.monitor_y = monitor.y;
            project.project.recording.monitor_width = monitor.width;
            project.project.recording.monitor_height = monitor.height;
        } else {
            project.project.recording.monitor_name.clear();
            project.project.recording.monitor_width = capture_width;
            project.project.recording.monitor_height = capture_height;
        }

        if !monitors.is_empty() {
            let (vx, vy, vw, vh) = virtual_desktop_bounds(&monitors);
            project.project.recording.virtual_x = vx;
            project.project.recording.virtual_y = vy;
            project.project.recording.virtual_width = vw;
            project.project.recording.virtual_height = vh;
            project.project.recording.monitors = monitors
                .iter()
                .map(|m| RecordedMonitor {
                    name: m.name.clone(),
                    x: m.x,
                    y: m.y,
                    width: m.width,
                    height: m.height,
                    primary: m.primary,
                })
                .collect();
        } else {
            project.project.recording.virtual_x = 0;
            project.project.recording.virtual_y = 0;
            project.project.recording.virtual_width = capture_width;
            project.project.recording.virtual_height = capture_height;
            project.project.recording.monitors.clear();
        }

        project.project.recording.display_server = self.backend.get_display_server();

        // Persist a placeholder screen track immediately so abrupt termination
        // (for example, SIGINT before graceful stop) still leaves discoverable
        // metadata for recovery/export paths.
        project.project.tracks.screen = Some(TrackRef {
            path: "sources/screen.mkv".to_string(),
            duration_secs: 0.0,
            codec: "h264".to_string(),
            offset_ns: 0,
        });
        project
            .save()
            .map_err(|e| GrabmeError::capture(format!("Failed to save project metadata: {e}")))?;

        // Start the recording clock
        let clock = RecordingClock::start();

        tracing::info!(
            epoch_wall = %clock.epoch_wall(),
            "Recording clock started"
        );

        let sources_dir = project.root.join("sources");

        // Build all pipelines first so startup is near-simultaneous.
        let screen_path = sources_dir.join("screen.mkv");
        let mut screen_pipeline = self
            .backend
            .build_screen_pipeline(&screen_path, self.config.fps)?;

        let mut webcam_pipeline = if self.config.webcam {
            let webcam_path = sources_dir.join("webcam.mkv");
            Some(
                self.backend
                    .build_webcam_pipeline(&webcam_path, self.config.fps)?,
            )
        } else {
            None
        };

        let mut mic_pipeline = if self.config.audio.mic {
            let mic_path = sources_dir.join("mic.wav");
            Some(
                self.backend
                    .build_mic_pipeline(&mic_path, self.config.audio.sample_rate)?,
            )
        } else {
            None
        };

        let mut system_pipeline = if self.config.audio.system {
            let system_path = sources_dir.join("system.wav");
            match self
                .backend
                .build_system_audio_pipeline(&system_path, self.config.audio.sample_rate)
            {
                Ok(pipeline) => Some(pipeline),
                Err(e) => {
                    tracing::warn!("Failed to build system audio pipeline: {}", e);
                    None
                }
            }
        } else {
            None
        };

        tracing::info!(
            screen = true,
            webcam = webcam_pipeline.is_some(),
            mic = mic_pipeline.is_some(),
            system = system_pipeline.is_some(),
            "Starting capture pipelines"
        );

        screen_pipeline.start()?;
        self.stream_offsets_ns.screen_ns = clock.elapsed_ns() as i64;
        self.screen_pipeline = Some(screen_pipeline);

        if let Some(mut webcam_pipeline) = webcam_pipeline.take() {
            webcam_pipeline.start()?;
            self.stream_offsets_ns.webcam_ns = clock.elapsed_ns() as i64;
            self.webcam_pipeline = Some(webcam_pipeline);
        }

        if let Some(mut mic_pipeline) = mic_pipeline.take() {
            mic_pipeline.start()?;
            self.stream_offsets_ns.mic_ns = clock.elapsed_ns() as i64;
            self.mic_pipeline = Some(mic_pipeline);
        }

        if let Some(mut system_pipeline) = system_pipeline.take() {
            system_pipeline.start()?;
            self.stream_offsets_ns.system_ns = clock.elapsed_ns() as i64;
            self.system_pipeline = Some(system_pipeline);
        }

        let events_path = project.root.join("meta").join("events.jsonl");
        // NOTE: detect_best_backend() is from input-tracker, which also needs abstraction potentially,
        // but for now we assume it works or we should add `get_input_backend` to our CaptureBackend trait?
        // The prompt said "Abstract the Capture Layer... Raw Input API... for high-precision mouse events".
        // Current input tracker detects backend.

        let mut tracker = InputTracker::new(
            detect_best_backend(),
            events_path,
            clock.clone(),
            capture_width,
            capture_height,
            selected_monitor
                .as_ref()
                .map(|m| m.scale_factor)
                .unwrap_or(1.0),
            self.config.pointer_sample_rate_hz,
        )?;
        self.stream_offsets_ns.events_ns = clock.elapsed_ns() as i64;

        let stop_flag = if let Some(flag) = self.backend.get_input_stop_flag() {
            flag
        } else {
            tracker.stop_flag()
        };

        self.input_stop_flag = Some(stop_flag);
        self.input_task = Some(tokio::spawn(async move { tracker.run().await }));
        tracing::info!("Input tracker task started");

        self.clock = Some(clock);
        self.project = Some(project);
        self.state = SessionState::Recording;
        self.stop_flag.store(false, Ordering::SeqCst);

        tracing::info!("Capture session started successfully");
        Ok(())
    }

    /// Stop recording and finalize the project.
    pub async fn stop(&mut self) -> GrabmeResult<PathBuf> {
        if self.state != SessionState::Recording && self.state != SessionState::Paused {
            return Err(GrabmeError::capture("Session not recording"));
        }

        tracing::info!("Stopping capture session");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(ref stop) = self.input_stop_flag {
            stop.store(true, Ordering::SeqCst);
        }

        if let Some(mut pipeline) = self.screen_pipeline.take() {
            pipeline.stop()?;
        }
        if let Some(mut pipeline) = self.webcam_pipeline.take() {
            pipeline.stop()?;
        }
        if let Some(mut pipeline) = self.mic_pipeline.take() {
            pipeline.stop()?;
        }
        if let Some(mut pipeline) = self.system_pipeline.take() {
            pipeline.stop()?;
        }

        if let Some(handle) = self.input_task.take() {
            match handle.await {
                Ok(Ok(events)) => tracing::info!(events, "Input tracker flushed"),
                Ok(Err(e)) => tracing::warn!(error = %e, "Input tracker exited with error"),
                Err(e) => tracing::warn!(error = %e, "Input tracker join failed"),
            }
        }

        // Cleanup backend resources (e.g. close portal session)
        self.backend.shutdown().await?;

        let elapsed = self.clock.as_ref().map(|c| c.elapsed_secs()).unwrap_or(0.0);
        let project_root = self.project.as_ref().map(|project| project.root.clone());

        if let Some(root) = project_root.as_ref() {
            let sources = root.join("sources");
            let screen_duration_ns = probe_media_duration_ns(&sources.join("screen.mkv"));
            if let Some(screen_duration_ns) = screen_duration_ns {
                if self.stream_offsets_ns.webcam_ns != 0 {
                    let webcam_duration_ns = probe_media_duration_ns(&sources.join("webcam.mkv"));
                    self.stream_offsets_ns.webcam_ns = corrected_track_offset_ns(
                        self.stream_offsets_ns.screen_ns,
                        self.stream_offsets_ns.webcam_ns,
                        screen_duration_ns,
                        webcam_duration_ns,
                    );
                }

                if self.stream_offsets_ns.mic_ns != 0 {
                    let mic_duration_ns = probe_media_duration_ns(&sources.join("mic.wav"));
                    self.stream_offsets_ns.mic_ns = corrected_track_offset_ns(
                        self.stream_offsets_ns.screen_ns,
                        self.stream_offsets_ns.mic_ns,
                        screen_duration_ns,
                        mic_duration_ns,
                    );
                }

                if self.stream_offsets_ns.system_ns != 0 {
                    let system_duration_ns = probe_media_duration_ns(&sources.join("system.wav"));
                    self.stream_offsets_ns.system_ns = corrected_track_offset_ns(
                        self.stream_offsets_ns.screen_ns,
                        self.stream_offsets_ns.system_ns,
                        screen_duration_ns,
                        system_duration_ns,
                    );
                }
            }
        }

        tracing::info!(duration_secs = elapsed, "Recording stopped");

        // Update project with track references
        if let Some(ref mut project) = self.project {
            project.project.recording.cursor_hidden = self.config.screen.hide_cursor;
            project.project.recording.monitor_index = match self.config.screen.mode {
                CaptureMode::FullScreen { monitor_index } => monitor_index,
                _ => 0,
            };

            let screen_path = project.root.join("sources").join("screen.mkv");
            if let Some((captured_w, captured_h)) = probe_video_dimensions(&screen_path) {
                let expected_w = project.project.recording.monitor_width;
                let expected_h = project.project.recording.monitor_height;
                if expected_w > 0
                    && expected_h > 0
                    && (captured_w != expected_w || captured_h != expected_h)
                {
                    tracing::warn!(
                        expected_w,
                        expected_h,
                        captured_w,
                        captured_h,
                        path = %screen_path.display(),
                        "Captured source dimensions differ from selected monitor metadata"
                    );
                }
            }

            project.project.tracks.screen = Some(TrackRef {
                path: "sources/screen.mkv".to_string(),
                duration_secs: elapsed,
                codec: "h264".to_string(),
                offset_ns: self.stream_offsets_ns.screen_ns,
            });

            if self.config.webcam && self.stream_offsets_ns.webcam_ns != 0 {
                project.project.tracks.webcam = Some(TrackRef {
                    path: "sources/webcam.mkv".to_string(),
                    duration_secs: elapsed,
                    codec: "h264".to_string(),
                    offset_ns: self.stream_offsets_ns.webcam_ns,
                });
            }

            if self.config.audio.mic {
                project.project.tracks.mic = Some(TrackRef {
                    path: "sources/mic.wav".to_string(),
                    duration_secs: elapsed,
                    codec: "pcm".to_string(),
                    offset_ns: self.stream_offsets_ns.mic_ns,
                });
            }

            if self.config.audio.system && self.stream_offsets_ns.system_ns != 0 {
                project.project.tracks.system_audio = Some(TrackRef {
                    path: "sources/system.wav".to_string(),
                    duration_secs: elapsed,
                    codec: "pcm".to_string(),
                    offset_ns: self.stream_offsets_ns.system_ns,
                });
            }

            project
                .save()
                .map_err(|e| GrabmeError::capture(format!("Failed to save project: {e}")))?;
        }

        self.state = SessionState::Stopped;

        self.log_clock_drift_check();

        Ok(self
            .project
            .as_ref()
            .map(|p| p.root.clone())
            .unwrap_or_default())
    }

    /// Pause recording (keeps pipelines alive but stops writing).
    pub fn pause(&mut self) -> GrabmeResult<()> {
        if self.state != SessionState::Recording {
            return Err(GrabmeError::capture("Not recording"));
        }
        if let Some(ref mut pipeline) = self.screen_pipeline {
            pipeline.pause()?;
        }
        if let Some(ref mut pipeline) = self.webcam_pipeline {
            pipeline.pause()?;
        }
        if let Some(ref mut pipeline) = self.mic_pipeline {
            pipeline.pause()?;
        }
        if let Some(ref mut pipeline) = self.system_pipeline {
            pipeline.pause()?;
        }
        self.state = SessionState::Paused;
        tracing::info!("Recording paused");
        Ok(())
    }

    /// Resume a paused recording.
    pub fn resume(&mut self) -> GrabmeResult<()> {
        if self.state != SessionState::Paused {
            return Err(GrabmeError::capture("Not paused"));
        }
        if let Some(ref mut pipeline) = self.screen_pipeline {
            pipeline.resume()?;
        }
        if let Some(ref mut pipeline) = self.webcam_pipeline {
            pipeline.resume()?;
        }
        if let Some(ref mut pipeline) = self.mic_pipeline {
            pipeline.resume()?;
        }
        if let Some(ref mut pipeline) = self.system_pipeline {
            pipeline.resume()?;
        }
        self.state = SessionState::Recording;
        tracing::info!("Recording resumed");
        Ok(())
    }

    /// Get a clone of the stop flag for use in worker threads.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    /// Recording duration so far.
    pub fn elapsed_secs(&self) -> f64 {
        self.clock.as_ref().map(|c| c.elapsed_secs()).unwrap_or(0.0)
    }

    // Internal helpers

    fn selected_monitor_index(&self) -> usize {
        match self.config.screen.mode {
            CaptureMode::FullScreen { monitor_index } => monitor_index,
            _ => 0,
        }
    }

    fn resolve_selected_monitor(
        &self,
        monitors: &[MonitorInfo],
    ) -> GrabmeResult<Option<MonitorInfo>> {
        match self.config.screen.mode {
            CaptureMode::FullScreen { monitor_index } => {
                if monitors.is_empty() {
                    return Err(GrabmeError::capture(
                        "No monitors detected; cannot start fullscreen capture",
                    ));
                }

                let selected = monitors.get(monitor_index).cloned().ok_or_else(|| {
                    GrabmeError::capture(format!(
                        "Invalid monitor index {monitor_index}. Available monitors: {}",
                        format_monitor_list(monitors)
                    ))
                })?;
                Ok(Some(selected))
            }
            _ => Ok(monitors.first().cloned()),
        }
    }

    fn log_clock_drift_check(&self) {
        let threshold_ns: i64 = 100_000_000;
        let reference = self.stream_offsets_ns.screen_ns;
        if reference == 0 {
            return;
        }

        for (label, offset) in [
            ("events", self.stream_offsets_ns.events_ns),
            ("webcam", self.stream_offsets_ns.webcam_ns),
            ("mic", self.stream_offsets_ns.mic_ns),
            ("system", self.stream_offsets_ns.system_ns),
        ] {
            if offset == 0 {
                continue;
            }
            let measurement = DriftMeasurement {
                reference_ns: reference as u64,
                measured_ns: offset as u64,
            };
            let drift_ns = measurement.drift_ns().abs();
            let drift_ms = measurement.drift_ms().abs();
            if drift_ns > threshold_ns {
                tracing::warn!(stream = label, drift_ms, "Clock drift exceeds 100ms");
            } else {
                tracing::info!(stream = label, drift_ms, "Clock drift within threshold");
            }
        }
    }
}

fn format_monitor_list(monitors: &[MonitorInfo]) -> String {
    monitors
        .iter()
        .enumerate()
        .map(|(idx, monitor)| {
            format!(
                "{idx}:{}({}x{}@{},{}{})",
                monitor.name,
                monitor.width,
                monitor.height,
                monitor.x,
                monitor.y,
                if monitor.primary { ",primary" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn corrected_track_offset_ns(
    screen_offset_ns: i64,
    measured_track_offset_ns: i64,
    screen_duration_ns: i64,
    track_duration_ns: Option<i64>,
) -> i64 {
    let measured_delta_ns = measured_track_offset_ns - screen_offset_ns;
    let inferred_delta_ns = track_duration_ns
        .filter(|duration| *duration > 0)
        .map(|duration| screen_duration_ns - duration);

    let corrected_delta_ns = match inferred_delta_ns {
        Some(inferred) => {
            // Blend measured and inferred start deltas for better stability.
            ((measured_delta_ns as f64 * 0.5) + (inferred as f64 * 0.5)).round() as i64
        }
        None => measured_delta_ns,
    };

    screen_offset_ns + corrected_delta_ns
}

fn probe_media_duration_ns(path: &std::path::Path) -> Option<i64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8(output.stdout).ok()?;
    let secs = raw.lines().next()?.trim().parse::<f64>().ok()?;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }

    Some((secs * 1_000_000_000.0).round() as i64)
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

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            name: "recording".to_string(),
            output_dir: PathBuf::from("."),
            screen: ScreenCaptureConfig {
                mode: CaptureMode::FullScreen { monitor_index: 0 },
                hide_cursor: true,
            },
            audio: AudioCaptureConfig {
                mic: true,
                system: true,
                app_isolation: None,
                sample_rate: 48000,
            },
            webcam: false,
            fps: 60,
            pointer_sample_rate_hz: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(name: &str, x: i32) -> MonitorInfo {
        MonitorInfo {
            name: name.to_string(),
            width: 1920,
            height: 1080,
            x,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_hz: 60,
            primary: x == 0,
        }
    }

    #[test]
    fn resolve_selected_monitor_rejects_invalid_index() {
        let mut config = SessionConfig::default();
        config.screen.mode = CaptureMode::FullScreen { monitor_index: 2 };
        let session = CaptureSession::new(config);
        let monitors = vec![monitor("HDMI-A-1", 0), monitor("DP-1", 1920)];

        let err = session.resolve_selected_monitor(&monitors).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Invalid monitor index 2"));
        assert!(msg.contains("HDMI-A-1"));
        assert!(msg.contains("DP-1"));
    }

    #[test]
    fn corrected_track_offset_blends_measured_and_duration_inferred_deltas() {
        // screen starts at 10ms, track starts at 70ms (measured +60ms)
        // screen duration 10.0s, track duration 9.9s (inferred +100ms)
        let corrected =
            corrected_track_offset_ns(10_000_000, 70_000_000, 10_000_000_000, Some(9_900_000_000));

        // blended delta should be +80ms, absolute offset = 90ms.
        assert_eq!(corrected, 90_000_000);
    }
}
