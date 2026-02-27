use std::path::Path;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::MonitorInfo;
use grabme_platform_windows as platform_windows;

use crate::backend::CaptureBackend;
use crate::pipeline::{
    build_windows_mic_pipeline, build_windows_screen_pipeline, build_windows_system_audio_pipeline,
    build_windows_webcam_pipeline, CapturePipeline,
};
use crate::session::{CaptureMode, ScreenCaptureConfig};

pub struct WindowsBackend {
    selected_monitor_index: Option<usize>,
    hide_cursor: bool,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            selected_monitor_index: None,
            hide_cursor: false,
        }
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
        tracing::info!("Initialized Windows capture backend");
        Ok(())
    }

    fn detect_monitors(&self) -> GrabmeResult<Vec<MonitorInfo>> {
        platform_windows::detect_monitors()
    }

    async fn prepare_screen_capture(
        &mut self,
        config: &ScreenCaptureConfig,
    ) -> GrabmeResult<(u32, u32)> {
        self.hide_cursor = config.hide_cursor;
        let monitor_index = match config.mode {
            CaptureMode::FullScreen { monitor_index } => monitor_index,
            CaptureMode::Window { .. } => {
                return Err(GrabmeError::unsupported(
                    "Window capture is not implemented on Windows yet",
                ));
            }
            CaptureMode::Region { .. } => {
                return Err(GrabmeError::unsupported(
                    "Region capture is not implemented on Windows yet",
                ));
            }
        };

        let monitors = self.detect_monitors()?;
        if monitor_index >= monitors.len() {
            return Err(GrabmeError::capture(format!(
                "Invalid monitor index {monitor_index}. Available monitors: {}",
                monitor_list_for_error(&monitors)
            )));
        }

        let monitor = &monitors[monitor_index];
        self.selected_monitor_index = Some(monitor_index);
        Ok((monitor.width, monitor.height))
    }

    fn build_screen_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        let monitor_index = self.selected_monitor_index.ok_or_else(|| {
            GrabmeError::capture(
                "Monitor selection missing. Did you call prepare_screen_capture on Windows backend?",
            )
        })?;

        build_windows_screen_pipeline(output_path, fps, monitor_index, self.hide_cursor)
    }

    fn build_mic_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        build_windows_mic_pipeline(output_path, sample_rate)
    }

    fn build_system_audio_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        build_windows_system_audio_pipeline(output_path, sample_rate)
    }

    fn build_webcam_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        build_windows_webcam_pipeline(output_path, fps)
    }

    fn get_display_server(&self) -> grabme_project_model::project::DisplayServer {
        grabme_project_model::project::DisplayServer::Windows
    }

    async fn shutdown(&mut self) -> GrabmeResult<()> {
        Ok(())
    }
}

fn monitor_list_for_error(monitors: &[MonitorInfo]) -> String {
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
