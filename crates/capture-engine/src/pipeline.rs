//! GStreamer pipeline construction for capture.
//!
//! This module will contain the actual GStreamer pipeline setup.
//! For now it defines the trait interface that the session uses.

use grabme_common::error::GrabmeResult;

/// Trait for a media capture pipeline.
///
/// Implementations will wrap GStreamer or FFmpeg pipelines
/// for screen, audio, and webcam capture.
pub trait CapturePipeline: Send {
    /// Start the pipeline.
    fn start(&mut self) -> GrabmeResult<()>;

    /// Stop the pipeline and finalize output.
    fn stop(&mut self) -> GrabmeResult<()>;

    /// Pause the pipeline.
    fn pause(&mut self) -> GrabmeResult<()>;

    /// Resume the pipeline.
    fn resume(&mut self) -> GrabmeResult<()>;

    /// Check if the pipeline is currently running.
    fn is_running(&self) -> bool;

    /// Get pipeline statistics.
    fn stats(&self) -> PipelineStats;
}

/// Runtime statistics from a capture pipeline.
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Frames captured.
    pub frames_captured: u64,

    /// Frames dropped due to processing delays.
    pub frames_dropped: u64,

    /// Bytes written to disk.
    pub bytes_written: u64,

    /// Current encoding latency in milliseconds.
    pub encoding_latency_ms: f64,
}

impl PipelineStats {
    /// Drop rate as a percentage.
    pub fn drop_rate(&self) -> f64 {
        let total = self.frames_captured + self.frames_dropped;
        if total == 0 {
            return 0.0;
        }
        self.frames_dropped as f64 / total as f64 * 100.0
    }
}

// Placeholder: GStreamer screen capture pipeline
// Will be implemented in Phase 1 with actual gstreamer-rs bindings

// Placeholder: PipeWire audio capture pipeline
// Will be implemented in Phase 1

// Placeholder: V4L2 webcam capture pipeline
// Will be implemented in Phase 2
