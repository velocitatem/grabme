//! Recording session management.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use grabme_common::clock::{DriftMeasurement, RecordingClock};
use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_input_tracker::backends::detect_best_backend;
use grabme_input_tracker::InputTracker;
use grabme_platform_linux::portal::{
    close_session, is_portal_available, request_screencast, CursorMode,
};
use grabme_platform_linux::SourceType;
use grabme_platform_linux::{detect_display_server, DisplayServer};
use grabme_project_model::{LoadedProject, TrackRef};

use crate::pipeline::{
    build_mic_pipeline, build_screen_pipeline, build_system_audio_pipeline, build_x11_mic_pipeline,
    build_x11_screen_pipeline, CapturePipeline,
};

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
    screen_pipeline: Option<Box<dyn CapturePipeline>>,
    mic_pipeline: Option<Box<dyn CapturePipeline>>,
    system_pipeline: Option<Box<dyn CapturePipeline>>,
    portal_session_handle: Option<String>,
    input_stop_flag: Option<Arc<AtomicBool>>,
    input_task: Option<tokio::task::JoinHandle<GrabmeResult<u64>>>,
    stream_offsets_ns: StreamOffsets,
}

#[derive(Debug, Default, Clone, Copy)]
struct StreamOffsets {
    screen_ns: i64,
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
            screen_pipeline: None,
            mic_pipeline: None,
            system_pipeline: None,
            portal_session_handle: None,
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

        // Create project on disk
        let project_dir = self.config.output_dir.join(&self.config.name);
        let project = LoadedProject::create(
            &project_dir,
            &self.config.name,
            self.detect_capture_width(),
            self.detect_capture_height(),
            self.config.fps,
        )
        .map_err(|e| GrabmeError::capture(format!("Failed to create project: {e}")))?;

        // Start the recording clock
        let clock = RecordingClock::start();

        tracing::info!(
            epoch_wall = %clock.epoch_wall(),
            "Recording clock started"
        );

        let sources_dir = project.root.join("sources");
        let screen_path = sources_dir.join("screen.mkv");
        let capture_width = self.detect_capture_width();
        let capture_height = self.detect_capture_height();

        let display_server = detect_display_server();
        tracing::info!(?display_server, "Detected display server");
        let mut screen_pipeline = match display_server {
            DisplayServer::Wayland => {
                if !is_portal_available() {
                    return Err(GrabmeError::platform(
                        "XDG ScreenCast portal is not available for this Wayland session",
                    ));
                }

                let cursor_mode = if self.config.screen.hide_cursor {
                    CursorMode::Hidden
                } else {
                    CursorMode::Embedded
                };

                let portal_session = request_screencast(SourceType::Monitor, cursor_mode).await?;
                self.portal_session_handle = Some(portal_session.session_handle.clone());
                build_screen_pipeline(
                    portal_session.pipewire_node_id,
                    &screen_path,
                    self.config.fps,
                )?
            }
            DisplayServer::X11 => {
                tracing::info!("Using X11 capture path (ximagesrc)");
                build_x11_screen_pipeline(
                    &screen_path,
                    self.config.fps,
                    self.config.screen.hide_cursor,
                )?
            }
            DisplayServer::Unknown => {
                return Err(GrabmeError::platform(
                    "Unsupported display server (neither Wayland nor X11)",
                ));
            }
        };
        tracing::info!("Starting screen pipeline");
        screen_pipeline.start()?;
        tracing::info!("Screen pipeline started");
        self.stream_offsets_ns.screen_ns = clock.elapsed_ns() as i64;
        self.screen_pipeline = Some(screen_pipeline);

        if self.config.audio.mic {
            let mic_path = sources_dir.join("mic.wav");
            let mut mic_pipeline = if display_server == DisplayServer::X11 {
                build_x11_mic_pipeline(&mic_path, self.config.audio.sample_rate)?
            } else {
                build_mic_pipeline(&mic_path, self.config.audio.sample_rate)?
            };
            tracing::info!("Starting mic pipeline");
            mic_pipeline.start()?;
            tracing::info!("Mic pipeline started");
            self.stream_offsets_ns.mic_ns = clock.elapsed_ns() as i64;
            self.mic_pipeline = Some(mic_pipeline);
        }

        if self.config.audio.system && display_server == DisplayServer::X11 {
            tracing::warn!(
                "System audio capture via PipeWire monitor is currently disabled on X11; use --no-system-audio"
            );
        }

        if self.config.audio.system && display_server != DisplayServer::X11 {
            let system_path = sources_dir.join("system.wav");
            let mut system_pipeline =
                build_system_audio_pipeline(&system_path, self.config.audio.sample_rate)?;
            tracing::info!("Starting system audio pipeline");
            system_pipeline.start()?;
            tracing::info!("System audio pipeline started");
            self.stream_offsets_ns.system_ns = clock.elapsed_ns() as i64;
            self.system_pipeline = Some(system_pipeline);
        }

        let events_path = project.root.join("meta").join("events.jsonl");
        let mut tracker = InputTracker::new(
            detect_best_backend(),
            events_path,
            clock.clone(),
            capture_width,
            capture_height,
            1.0,
            self.config.pointer_sample_rate_hz,
        )?;
        self.stream_offsets_ns.events_ns = clock.elapsed_ns() as i64;
        let stop_flag = tracker.stop_flag();
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

        if let Some(handle) = self.portal_session_handle.take() {
            let _ = close_session(&handle).await;
        }

        let elapsed = self.clock.as_ref().map(|c| c.elapsed_secs()).unwrap_or(0.0);

        tracing::info!(duration_secs = elapsed, "Recording stopped");

        // Update project with track references
        if let Some(ref mut project) = self.project {
            project.project.tracks.screen = Some(TrackRef {
                path: "sources/screen.mkv".to_string(),
                duration_secs: elapsed,
                codec: "h264".to_string(),
                offset_ns: self.stream_offsets_ns.screen_ns,
            });

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

    fn detect_capture_width(&self) -> u32 {
        match &self.config.screen.mode {
            CaptureMode::Region { width, .. } => *width,
            _ => 1920, // TODO: detect from platform
        }
    }

    fn detect_capture_height(&self) -> u32 {
        match &self.config.screen.mode {
            CaptureMode::Region { height, .. } => *height,
            _ => 1080, // TODO: detect from platform
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
