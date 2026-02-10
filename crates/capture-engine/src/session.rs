//! Recording session management.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use grabme_common::clock::RecordingClock;
use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_project_model::{LoadedProject, RecordingConfig, TrackRef};

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
    Region { x: u32, y: u32, width: u32, height: u32 },
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

        // TODO: Initialize GStreamer pipeline for screen capture
        // TODO: Initialize audio capture pipeline
        // TODO: Start input tracker
        // TODO: Start webcam capture if configured

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

        // TODO: Stop all pipelines
        // TODO: Flush event log
        // TODO: Finalize media files

        let elapsed = self
            .clock
            .as_ref()
            .map(|c| c.elapsed_secs())
            .unwrap_or(0.0);

        tracing::info!(duration_secs = elapsed, "Recording stopped");

        // Update project with track references
        if let Some(ref mut project) = self.project {
            project.project.tracks.screen = Some(TrackRef {
                path: "sources/screen.mkv".to_string(),
                duration_secs: elapsed,
                codec: "h264".to_string(),
                offset_ns: 0,
            });

            if self.config.audio.mic {
                project.project.tracks.mic = Some(TrackRef {
                    path: "sources/mic.wav".to_string(),
                    duration_secs: elapsed,
                    codec: "pcm".to_string(),
                    offset_ns: 0,
                });
            }

            if self.config.audio.system {
                project.project.tracks.system_audio = Some(TrackRef {
                    path: "sources/system.wav".to_string(),
                    duration_secs: elapsed,
                    codec: "pcm".to_string(),
                    offset_ns: 0,
                });
            }

            project
                .save()
                .map_err(|e| GrabmeError::capture(format!("Failed to save project: {e}")))?;
        }

        self.state = SessionState::Stopped;

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
        self.state = SessionState::Paused;
        tracing::info!("Recording paused");
        Ok(())
    }

    /// Resume a paused recording.
    pub fn resume(&mut self) -> GrabmeResult<()> {
        if self.state != SessionState::Paused {
            return Err(GrabmeError::capture("Not paused"));
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
