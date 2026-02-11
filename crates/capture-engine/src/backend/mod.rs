use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use grabme_common::error::GrabmeResult;
use grabme_platform_core::MonitorInfo;

use crate::pipeline::CapturePipeline;
use crate::session::ScreenCaptureConfig;

/// Abstract interface for platform-specific capture capabilities.
#[async_trait::async_trait]
pub trait CaptureBackend: Send + Sync {
    /// Initialize the backend (e.g. check permissions, connect to display server).
    async fn init(&mut self) -> GrabmeResult<()>;

    /// Detect available monitors.
    fn detect_monitors(&self) -> GrabmeResult<Vec<MonitorInfo>>;

    /// Prepare for screen capture (e.g. Request ScreenCast portal on Linux).
    /// Returns the negotiated width/height.
    async fn prepare_screen_capture(
        &mut self,
        config: &ScreenCaptureConfig,
    ) -> GrabmeResult<(u32, u32)>;

    /// Build the screen capture pipeline.
    fn build_screen_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>>;

    /// Build the microphone capture pipeline.
    fn build_mic_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>>;

    /// Build the system audio capture pipeline.
    fn build_system_audio_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>>;

    /// Build the webcam capture pipeline.
    fn build_webcam_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>>;

    /// Get the stop flag for input tracking if the backend manages it.
    fn get_input_stop_flag(&self) -> Option<Arc<AtomicBool>> {
        None
    }

    /// Get the display server type for metadata.
    fn get_display_server(&self) -> grabme_project_model::project::DisplayServer;

    /// Cleanup resources when session ends.
    async fn shutdown(&mut self) -> GrabmeResult<()>;
}

pub mod linux;
pub mod macos;
pub mod windows;

pub use linux::LinuxBackend;
pub use macos::MacOSBackend;
pub use windows::WindowsBackend;

/// Get the platform-specific backend.
pub fn get_backend() -> Box<dyn CaptureBackend> {
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxBackend::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsBackend::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSBackend::new())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        // Fallback or panic, though this code path should be unreachable on supported platforms
        panic!("Unsupported platform");
    }
}
