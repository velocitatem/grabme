//! GrabMe Input Tracker
//!
//! Records mouse position, clicks, keyboard events, and window focus
//! changes as a synchronized event stream. Uses a pluggable backend
//! architecture to support different input sources:
//!
//! - **Portal:** XDG Desktop Portal (sandboxed, limited)
//! - **Evdev:** Direct device access (requires privileges)
//! - **X11:** XInput2 (legacy)
//!
//! Events are written in append-only JSONL format for crash safety.

pub mod backends;
pub mod writer;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use grabme_common::clock::RecordingClock;
use grabme_common::error::GrabmeResult;
use grabme_project_model::event::{EventStreamHeader, InputEvent};

/// Trait for input tracking backends.
pub trait InputBackend: Send {
    /// Poll for the next input event. Returns `None` if no event is available.
    fn poll(&mut self) -> GrabmeResult<Option<InputEvent>>;

    /// Backend name for logging.
    fn name(&self) -> &str;

    /// Check if the backend is available on this system.
    fn is_available(&self) -> bool;
}

/// The input tracker that coordinates a backend with event writing.
pub struct InputTracker {
    backend: Box<dyn InputBackend>,
    writer: writer::EventWriter,
    #[allow(dead_code)] // Used for future drift detection
    clock: RecordingClock,
    stop_flag: Arc<AtomicBool>,
    events_logged: u64,
}

impl InputTracker {
    /// Create a new input tracker.
    pub fn new(
        backend: Box<dyn InputBackend>,
        output_path: PathBuf,
        clock: RecordingClock,
        capture_width: u32,
        capture_height: u32,
        scale_factor: f64,
        pointer_sample_rate_hz: u32,
    ) -> GrabmeResult<Self> {
        let header = EventStreamHeader {
            schema_version: "1.0".to_string(),
            epoch_monotonic_ns: 0,
            epoch_wall: clock.epoch_wall().to_string(),
            capture_width,
            capture_height,
            scale_factor,
            pointer_sample_rate_hz,
        };

        let writer = writer::EventWriter::new(output_path, header)?;

        Ok(Self {
            backend,
            writer,
            clock,
            stop_flag: Arc::new(AtomicBool::new(false)),
            events_logged: 0,
        })
    }

    /// Run the tracking loop until the stop flag is set.
    pub async fn run(&mut self) -> GrabmeResult<u64> {
        tracing::info!(backend = %self.backend.name(), "Input tracker started");

        while !self.stop_flag.load(Ordering::Relaxed) {
            match self.backend.poll() {
                Ok(Some(event)) => {
                    self.writer.write_event(&event)?;
                    self.events_logged += 1;
                }
                Ok(None) => {
                    // No event available, yield briefly
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Input tracking error");
                }
            }
        }

        self.writer.flush()?;
        tracing::info!(events = self.events_logged, "Input tracker stopped");
        Ok(self.events_logged)
    }

    /// Set the stop flag.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Get the stop flag for external coordination.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    /// Number of events logged so far.
    pub fn events_logged(&self) -> u64 {
        self.events_logged
    }
}
