use std::path::Path;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;
use grabme_platform_macos as platform_macos;

use crate::backend::CaptureBackend;
use crate::pipeline::CapturePipeline;
use crate::session::ScreenCaptureConfig;

/// Compile-safe macOS backend skeleton.
///
/// TODO(platform/macos): implement ScreenCaptureKit + Quartz input integration.
pub struct MacOSBackend;

impl MacOSBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CaptureBackend for MacOSBackend {
    async fn init(&mut self) -> GrabmeResult<()> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn detect_monitors(&self) -> GrabmeResult<Vec<MonitorInfo>> {
        platform_macos::detect_monitors()
    }

    async fn prepare_screen_capture(
        &mut self,
        _config: &ScreenCaptureConfig,
    ) -> GrabmeResult<(u32, u32)> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn build_screen_pipeline(
        &self,
        _output_path: &Path,
        _fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn build_mic_pipeline(
        &self,
        _output_path: &Path,
        _sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn build_system_audio_pipeline(
        &self,
        _output_path: &Path,
        _sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn build_webcam_pipeline(
        &self,
        _output_path: &Path,
        _fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("macOS backend not yet implemented"))
    }

    fn get_display_server(&self) -> grabme_project_model::project::DisplayServer {
        grabme_project_model::project::DisplayServer::MacOS
    }

    async fn shutdown(&mut self) -> GrabmeResult<()> {
        Ok(())
    }
}
