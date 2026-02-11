use std::path::Path;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;
use grabme_platform_windows as platform_windows;

use crate::backend::CaptureBackend;
use crate::pipeline::CapturePipeline;
use crate::session::ScreenCaptureConfig;

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CaptureBackend for WindowsBackend {
    async fn init(&mut self) -> GrabmeResult<()> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn detect_monitors(&self) -> GrabmeResult<Vec<MonitorInfo>> {
        platform_windows::detect_monitors()
    }

    async fn prepare_screen_capture(
        &mut self,
        _config: &ScreenCaptureConfig,
    ) -> GrabmeResult<(u32, u32)> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn build_screen_pipeline(
        &self,
        _output_path: &Path,
        _fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn build_mic_pipeline(
        &self,
        _output_path: &Path,
        _sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn build_system_audio_pipeline(
        &self,
        _output_path: &Path,
        _sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn build_webcam_pipeline(
        &self,
        _output_path: &Path,
        _fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        Err(GrabmeError::platform("Windows backend not yet implemented"))
    }

    fn get_display_server(&self) -> grabme_project_model::project::DisplayServer {
        grabme_project_model::project::DisplayServer::Windows
    }

    async fn shutdown(&mut self) -> GrabmeResult<()> {
        Ok(())
    }
}
