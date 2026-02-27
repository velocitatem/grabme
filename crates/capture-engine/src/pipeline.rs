//! GStreamer pipeline construction for capture.
//!
//! This module will contain the actual GStreamer pipeline setup.
//! For now it defines the trait interface that the session uses.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

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

        // Wait for the pipeline to actually reach Playing state.
        // GStreamer state changes are async; without this wait the pipeline
        // may not have opened the capture source yet when we return.
        let wait_result = self.pipeline.state(gst::ClockTime::from_seconds(10));
        match wait_result {
            (Ok(_), gst::State::Playing, _) => {}
            (Ok(_), state, _) => {
                tracing::warn!(
                    pipeline = %self.name,
                    ?state,
                    "Pipeline did not reach Playing state within timeout"
                );
            }
            (Err(e), _, _) => {
                return Err(grabme_common::error::GrabmeError::capture(format!(
                    "{} pipeline failed to reach Playing state: {e:?}",
                    self.name
                )));
            }
        }

        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> GrabmeResult<()> {
        // Send EOS downstream first so encoders/muxers can flush and finalize
        // their output. Without this, the tail of the recording (last few
        // seconds worth of buffered frames) may be truncated or corrupted.
        let eos_sent = self.pipeline.send_event(gst::event::Eos::new());
        if !eos_sent {
            tracing::warn!(pipeline = %self.name, "Failed to send EOS event; output may be truncated");
        } else {
            // Wait for EOS to propagate through the entire pipeline.
            // We poll the bus with a timeout so we don't block forever if
            // something goes wrong.
            let bus = self.pipeline.bus();
            if let Some(bus) = bus {
                let deadline = Duration::from_secs(10);
                let start = std::time::Instant::now();
                loop {
                    let timeout_ns = {
                        let elapsed = start.elapsed();
                        if elapsed >= deadline {
                            break;
                        }
                        let remaining = deadline - elapsed;
                        gst::ClockTime::from_nseconds(remaining.as_nanos() as u64)
                    };
                    match bus.timed_pop(timeout_ns) {
                        Some(msg) => match msg.view() {
                            gst::MessageView::Eos(_) => {
                                tracing::debug!(pipeline = %self.name, "EOS received; pipeline drained");
                                break;
                            }
                            gst::MessageView::Error(e) => {
                                tracing::warn!(
                                    pipeline = %self.name,
                                    error = %e.error(),
                                    "Pipeline error during EOS drain"
                                );
                                break;
                            }
                            _ => {}
                        },
                        None => {
                            tracing::warn!(pipeline = %self.name, "EOS drain timed out after 10s");
                            break;
                        }
                    }
                }
            }
        }

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
    // `keyframe-interval` = fps * 2 seconds: one keyframe every 2 seconds allows
    // reasonable seeking while keeping file size low.
    let keyint = fps.saturating_mul(2).max(2);
    // queue elements decouple the capture source from the encoder so that
    // encoder stalls don't cause dropped frames at the source.
    let launch = format!(
        "pipewiresrc path={pipewire_node_id} do-timestamp=true ! queue max-size-buffers=200 leaky=downstream ! videoconvert ! videorate ! video/x-raw,framerate={fps}/1 ! queue max-size-buffers=8 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max={keyint} ! h264parse ! queue max-size-buffers=8 ! matroskamux ! filesink location=\"{path}\""
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
    let region = x11_capture_region_fragment(capture_region)?;
    let keyint = fps.saturating_mul(2).max(2);
    // `remote=true` allows ximagesrc to work correctly with certain remote X11
    // setups. `use-damage=false` ensures full frame delivery (no incremental
    // damage updates that can miss regions). queue leaky=downstream prevents
    // buffer build-up when the encoder is momentarily slow.
    let launch = format!(
        "ximagesrc use-damage=false remote=true show-pointer={show_pointer}{region} ! queue max-size-buffers=200 leaky=downstream ! videoconvert ! videorate ! video/x-raw,framerate={fps}/1 ! queue max-size-buffers=8 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max={keyint} ! h264parse ! queue max-size-buffers=8 ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "screen-x11",
        &launch,
    )?))
}

fn x11_capture_region_fragment(
    capture_region: Option<(i32, i32, u32, u32)>,
) -> GrabmeResult<String> {
    let Some((x, y, width, height)) = capture_region else {
        return Ok(String::new());
    };

    if width == 0 || height == 0 {
        return Err(GrabmeError::capture(format!(
            "Invalid X11 capture region {width}x{height} at ({x},{y})"
        )));
    }

    let width_i32 = i32::try_from(width)
        .map_err(|_| GrabmeError::capture(format!("X11 capture width too large: {width}")))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| GrabmeError::capture(format!("X11 capture height too large: {height}")))?;

    let endx = x
        .checked_add(width_i32 - 1)
        .ok_or_else(|| GrabmeError::capture("X11 capture region x-range overflow"))?;
    let endy = y
        .checked_add(height_i32 - 1)
        .ok_or_else(|| GrabmeError::capture("X11 capture region y-range overflow"))?;

    Ok(format!(" startx={x} starty={y} endx={endx} endy={endy}"))
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

pub fn build_windows_screen_pipeline(
    output_path: &Path,
    fps: u32,
    monitor_index: usize,
    hide_cursor: bool,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let show_cursor = if hide_cursor { "false" } else { "true" };
    let keyint = fps.saturating_mul(2).max(2);
    let launch = format!(
        "d3d11screencapturesrc monitor-index={monitor_index} show-cursor={show_cursor} ! queue max-size-buffers=200 leaky=downstream ! videoconvert ! videorate ! video/x-raw,framerate={fps}/1 ! queue max-size-buffers=8 ! x264enc tune=zerolatency speed-preset=veryfast key-int-max={keyint} ! h264parse ! queue max-size-buffers=8 ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "screen-windows",
        &launch,
    )?))
}

pub fn build_windows_mic_pipeline(
    output_path: &Path,
    sample_rate: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "wasapisrc low-latency=true do-timestamp=true ! audioconvert ! audioresample ! audio/x-raw,rate={sample_rate} ! wavenc ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "mic-windows",
        &launch,
    )?))
}

pub fn build_windows_system_audio_pipeline(
    output_path: &Path,
    sample_rate: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let launch = format!(
        "wasapisrc loopback=true low-latency=true do-timestamp=true ! audioconvert ! audioresample ! audio/x-raw,rate={sample_rate} ! wavenc ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "system-windows",
        &launch,
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

pub fn build_windows_webcam_pipeline(
    output_path: &Path,
    fps: u32,
) -> GrabmeResult<Box<dyn CapturePipeline>> {
    let path = escape_path(output_path);
    let webcam_fps = fps.clamp(1, 30);
    let keyint = (webcam_fps.saturating_mul(2)).max(2);
    let launch = format!(
        "ksvideosrc device-index=0 do-stats=true ! videoconvert ! videoscale ! videorate ! video/x-raw,framerate={webcam_fps}/1 ! x264enc tune=zerolatency speed-preset=veryfast bitrate=2500 key-int-max={keyint} ! h264parse ! matroskamux ! filesink location=\"{path}\""
    );
    Ok(Box::new(GstCapturePipeline::from_launch(
        "webcam-windows",
        &launch,
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

/// Detect the best V4L2 webcam device.
///
/// Strategy:
/// 1. Enumerate `/dev/video0`–`/dev/video15`
/// 2. For each candidate that exists, check `/sys/class/video4linux/videoN/`
///    to determine device capabilities. We want devices that support
///    `V4L2_CAP_VIDEO_CAPTURE` (capability bit 0x00000001) but are NOT
///    output-only or metadata devices.
/// 3. Prefer devices whose sysfs name contains common webcam keywords
///    over capture cards or TV tuners.
/// 4. Fall back to the first video device that responds to `v4l2-ctl --info`
///    without errors, or the first existing `/dev/videoN` node.
fn detect_default_webcam_device() -> Option<String> {
    // Collect all candidate /dev/videoN nodes
    let mut candidates: Vec<(String, u32)> = Vec::new(); // (path, priority)

    for idx in 0..16u32 {
        let dev_path = format!("/dev/video{idx}");
        if !std::path::Path::new(&dev_path).exists() {
            continue;
        }

        let priority = webcam_device_priority(idx, &dev_path);
        candidates.push((dev_path, priority));
    }

    if candidates.is_empty() {
        return None;
    }

    // Sort by priority descending (higher = better webcam candidate)
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    let best = &candidates[0];
    if best.1 == 0 {
        // All devices have zero priority (no sysfs info, no v4l2-ctl) —
        // fall back to the first device that exists
        return Some(candidates[0].0.clone());
    }

    tracing::info!(
        device = %best.0,
        priority = best.1,
        "Selected webcam device"
    );
    Some(best.0.clone())
}

/// Score a V4L2 device as a webcam candidate (higher = more likely a webcam).
/// Returns 0 if the device is definitely not a webcam, or a positive score.
fn webcam_device_priority(idx: u32, dev_path: &str) -> u32 {
    let sysfs_dir = format!("/sys/class/video4linux/video{idx}");

    // Read device name from sysfs
    let sysfs_name_path = format!("{sysfs_dir}/name");
    let device_name = std::fs::read_to_string(&sysfs_name_path)
        .unwrap_or_default()
        .to_lowercase();

    // Read capabilities from sysfs if available
    // /sys/class/video4linux/videoN/device/streaming_intf gives streaming class
    // The most reliable check is reading the V4L2_CAP_DEVICE_CAPS via ioctl,
    // but we can't easily do that without unsafe. Instead use sysfs heuristics.

    // Positive indicators (webcam-like device names)
    let webcam_keywords = [
        "webcam",
        "camera",
        "cam",
        "facetime",
        "logitech",
        "microsoft",
        "creative",
        "razer",
        "elgato",
        "obs",
        "virtual",
        "v4l2loopback",
    ];
    // Negative indicators (capture cards, tuners, encoders — not webcams)
    let non_webcam_keywords = [
        "tuner",
        "tv",
        "dvb",
        "hdmi",
        "capture",
        "encoder",
        "decoder",
        "hauppauge",
        "blackmagic",
        "magewell",
    ];

    let has_non_webcam = non_webcam_keywords
        .iter()
        .any(|kw| device_name.contains(kw));
    if has_non_webcam {
        tracing::debug!(device = dev_path, name = %device_name, "Skipping non-webcam V4L2 device");
        return 0;
    }

    let has_webcam_keyword = webcam_keywords.iter().any(|kw| device_name.contains(kw));

    // Check if device supports video capture via v4l2-ctl (if available).
    // `v4l2-ctl --device=N --info` outputs "Device Caps" which includes
    // "Video Capture" for real capture devices.
    let v4l2_supports_capture = probe_v4l2_capture_capability(dev_path);

    match (has_webcam_keyword, v4l2_supports_capture) {
        (true, Some(true)) => 100, // Named webcam + confirmed capture
        (true, _) => 80,           // Named webcam (no v4l2-ctl available)
        (false, Some(true)) => 50, // Confirmed capture, generic name
        (false, Some(false)) => 0, // Confirmed non-capture
        (false, None) => 10,       // Unknown — low priority fallback
    }
}

/// Use `v4l2-ctl` to check if a device reports Video Capture capability.
/// Returns `Some(true)` if it does, `Some(false)` if it doesn't,
/// `None` if v4l2-ctl is not available.
fn probe_v4l2_capture_capability(dev_path: &str) -> Option<bool> {
    let output = std::process::Command::new("v4l2-ctl")
        .args(["--device", dev_path, "--info"])
        .output()
        .ok()?;

    if !output.status.success() {
        return Some(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    // Look for "video capture" in capabilities output
    if stdout.contains("video capture") {
        Some(true)
    } else {
        Some(false)
    }
}

fn escape_path(path: &Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::x11_capture_region_fragment;

    #[test]
    fn x11_region_fragment_uses_inclusive_end_coordinates() {
        let fragment = x11_capture_region_fragment(Some((2560, 0, 2560, 1440))).unwrap();
        assert_eq!(
            fragment,
            " startx=2560 starty=0 endx=5119 endy=1439".to_string()
        );
    }

    #[test]
    fn x11_region_fragment_rejects_zero_size() {
        let err = x11_capture_region_fragment(Some((0, 0, 0, 1080))).unwrap_err();
        assert!(err.to_string().contains("Invalid X11 capture region"));
    }
}
