//! GStreamer pipeline construction for capture.
//!
//! This module will contain the actual GStreamer pipeline setup.
//! For now it defines the trait interface that the session uses.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use grabme_common::error::{GrabmeError, GrabmeResult};
use gst::prelude::*;
use gstreamer as gst;

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

pub struct GstCapturePipeline {
    name: String,
    pipeline: gst::Pipeline,
    running: Arc<AtomicBool>,
    stats: PipelineStats,
}

impl GstCapturePipeline {
    pub fn from_launch(name: impl Into<String>, launch: &str) -> GrabmeResult<Self> {
        init_gstreamer()?;

        let element = gst::parse::launch(launch).map_err(|e| {
            grabme_common::error::GrabmeError::capture(format!("Failed to build pipeline: {e}"))
        })?;

        let pipeline = element.dynamic_cast::<gst::Pipeline>().map_err(|_| {
            grabme_common::error::GrabmeError::capture("Launch string did not produce a pipeline")
        })?;

        Ok(Self {
            name: name.into(),
            pipeline,
            running: Arc::new(AtomicBool::new(false)),
            stats: PipelineStats::default(),
        })
    }
}

impl CapturePipeline for GstCapturePipeline {
    fn start(&mut self) -> GrabmeResult<()> {
        self.pipeline.set_state(gst::State::Playing).map_err(|e| {
            grabme_common::error::GrabmeError::capture(format!(
                "Failed to start {} pipeline: {e:?}",
                self.name
            ))
        })?;
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> GrabmeResult<()> {
        self.pipeline.set_state(gst::State::Null).map_err(|e| {
            grabme_common::error::GrabmeError::capture(format!(
                "Failed to stop {} pipeline: {e:?}",
                self.name
            ))
        })?;
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn pause(&mut self) -> GrabmeResult<()> {
        self.pipeline.set_state(gst::State::Paused).map_err(|e| {
            grabme_common::error::GrabmeError::capture(format!(
                "Failed to pause {} pipeline: {e:?}",
                self.name
            ))
        })?;
        Ok(())
    }

    fn resume(&mut self) -> GrabmeResult<()> {
        self.pipeline.set_state(gst::State::Playing).map_err(|e| {
            grabme_common::error::GrabmeError::capture(format!(
                "Failed to resume {} pipeline: {e:?}",
                self.name
            ))
        })?;
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn stats(&self) -> PipelineStats {
        self.stats.clone()
    }
}

pub fn build_screen_pipeline(
    pipewire_node_id: u32,
    output_path: &Path,
    fps: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "pipewiresrc path={pipewire_node_id} do-timestamp=true ! videoconvert ! videorate ! video/x-raw,framerate={fps}/1 ! x264enc tune=zerolatency speed-preset=veryfast ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "screen", &launch,
    )?))
}

pub fn build_x11_screen_pipeline(
    output_path: &Path,
    fps: u32,
    hide_cursor: bool,
    capture_region: Option<(i32, i32, u32, u32)>,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let show_pointer = if hide_cursor { "false" } else { "true" };
    let region = capture_region
        .map(|(x, y, width, height)| {
            format!(
                " startx={x} starty={y} endx={} endy={}",
                x + width as i32,
                y + height as i32
            )
        })
        .unwrap_or_default();
    let launch = format!(
        "ximagesrc use-damage=false show-pointer={show_pointer}{region} ! videoconvert ! videorate ! video/x-raw,framerate={fps}/1 ! x264enc tune=zerolatency speed-preset=veryfast ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "screen-x11",
        &launch,
    )?))
}

pub fn build_mic_pipeline(
    output_path: &Path,
    sample_rate: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "pipewiresrc do-timestamp=true ! audioconvert ! audioresample ! audio/x-raw,rate={sample_rate} ! wavenc ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch("mic", &launch)?))
}

pub fn build_x11_mic_pipeline(
    output_path: &Path,
    sample_rate: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "pulsesrc do-timestamp=true ! audioconvert ! audioresample ! audio/x-raw,rate={sample_rate} ! wavenc ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "mic-x11", &launch,
    )?))
}

pub fn build_system_audio_pipeline(
    output_path: &Path,
    sample_rate: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "pipewiresrc do-timestamp=true stream-properties=props,media.class=Audio/Source ! audioconvert ! audioresample ! audio/x-raw,rate={sample_rate} ! wavenc ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "system", &launch,
    )?))
}

pub fn build_webcam_pipeline(
    output_path: &Path,
    fps: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let device = detect_default_webcam_device().ok_or_else(|| {
        GrabmeError::capture(
            "No webcam device found (expected /dev/video0 or another /dev/video* node)",
        )
    })?;
    let path = escape_path(output_path);
    let webcam_fps = fps.clamp(1, 30);
    let keyint = (webcam_fps.saturating_mul(2)).max(2);
    let launch = format!(
        "v4l2src device=\"{device}\" do-timestamp=true ! videoconvert ! videoscale ! videorate ! video/x-raw,framerate={webcam_fps}/1 ! x264enc tune=zerolatency speed-preset=veryfast bitrate=2500 key-int-max={keyint} ! h264parse ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "webcam", &launch,
    )?))
}

fn init_gstreamer() -> GrabmeResult<()> {
    static GST_INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let init_res = GST_INIT.get_or_init(|| gst::init().map_err(|e| e.to_string()));
    match init_res {
        Ok(()) => Ok(()),
        Err(e) => Err(grabme_common::error::GrabmeError::capture(format!(
            "Failed to initialize GStreamer: {e}"
        ))),
    }
}

fn detect_default_webcam_device() -> Option<String> {
    for idx in 0..16 {
        let candidate = format!("/dev/video{idx}");
        if std::path::Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }

    let entries = std::fs::read_dir("/dev").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("video") {
            let path = entry.path();
            if path.exists() {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }

    None
}

fn escape_path(path: &Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}
