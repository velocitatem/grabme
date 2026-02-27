use std::path::Path;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_platform_core::{virtual_desktop_bounds, MonitorInfo};
use grabme_platform_linux::portal::{
    close_session, is_portal_available, request_screencast, CursorMode,
};
use grabme_platform_linux::{detect_display_server, detect_monitors, DisplayServer, SourceType};

use crate::backend::CaptureBackend;
use crate::pipeline::{
    build_mic_pipeline, build_screen_pipeline, build_system_audio_pipeline, build_webcam_pipeline,
    build_x11_mic_pipeline, build_x11_screen_pipeline, CapturePipeline,
};
use crate::session::{CaptureMode, ScreenCaptureConfig};

pub struct LinuxBackend {
    display_server: DisplayServer,
    pipewire_node_id: Option<u32>,
    portal_session_handle: Option<String>,
    // Store cursor config for X11 pipeline
    cursor_hidden: bool,
    // Store region for X11 pipeline
    capture_region: Option<(i32, i32, u32, u32)>,
}

impl LinuxBackend {
    pub fn new() -> Self {
        Self {
            display_server: DisplayServer::Unknown,
            pipewire_node_id: None,
            portal_session_handle: None,
            cursor_hidden: true,
            capture_region: None,
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CaptureBackend for LinuxBackend {
    async fn init(&mut self) -> GrabmeResult<()> {
        self.display_server = detect_display_server();
        tracing::info!(?self.display_server, "Detected display server on Linux backend");
        Ok(())
    }

    fn detect_monitors(&self) -> GrabmeResult<Vec<MonitorInfo>> {
        detect_monitors()
    }

    async fn prepare_screen_capture(
        &mut self,
        config: &ScreenCaptureConfig,
    ) -> GrabmeResult<(u32, u32)> {
        self.cursor_hidden = config.hide_cursor;

        let monitor_index = match config.mode {
            CaptureMode::FullScreen { monitor_index } => monitor_index,
            _ => 0,
        };
        let monitors = self.detect_monitors().unwrap_or_default();
        if !monitors.is_empty() && monitor_index >= monitors.len() {
            return Err(GrabmeError::capture(format!(
                "Invalid monitor index {monitor_index}. Available monitors: {}",
                monitor_list_for_error(&monitors)
            )));
        }

        match self.display_server {
            DisplayServer::Wayland => {
                if !is_portal_available() {
                    return Err(GrabmeError::platform(
                        "XDG ScreenCast portal is not available for this Wayland session",
                    ));
                }

                let cursor_mode = if config.hide_cursor {
                    CursorMode::Hidden
                } else {
                    CursorMode::Embedded
                };

                // Note: For now we only support monitor capture via portal fully.
                // Window capture support would need to be passed here.
                let portal_session =
                    request_screencast(SourceType::Monitor, cursor_mode, monitor_index).await?;

                self.pipewire_node_id = Some(portal_session.pipewire_node_id);
                self.portal_session_handle = Some(portal_session.session_handle);

                Ok((portal_session.width, portal_session.height))
            }
            DisplayServer::X11 => {
                if monitors.is_empty() {
                    return Err(GrabmeError::capture("No monitors detected for X11 capture"));
                }
                // X11 reliability mode: always capture the full virtual desktop.
                // This prevents monitor-index/region drift and enables cursor-driven
                // monitor following in post processing.
                let (vx, vy, vw, vh) = virtual_desktop_bounds(&monitors);
                self.capture_region = Some((vx, vy, vw, vh));

                tracing::info!(
                    virtual_x = vx,
                    virtual_y = vy,
                    virtual_width = vw,
                    virtual_height = vh,
                    selected_monitor_index = monitor_index,
                    selected_monitor = ?monitors.get(monitor_index).map(|m| &m.name),
                    "X11 capture configured for full virtual desktop"
                );

                Ok((vw, vh))
            }
            _ => Err(GrabmeError::platform(
                "Unsupported display server for Linux backend (expected Wayland or X11)",
            )),
        }
    }

    fn build_screen_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        match self.display_server {
            DisplayServer::Wayland => {
                let node_id = self.pipewire_node_id.ok_or_else(|| {
                    GrabmeError::capture(
                        "PipeWire node ID not available. Did you call prepare_screen_capture?",
                    )
                })?;
                build_screen_pipeline(node_id, output_path, fps)
            }
            DisplayServer::X11 => {
                build_x11_screen_pipeline(output_path, fps, self.cursor_hidden, self.capture_region)
            }
            _ => Err(GrabmeError::platform("Unknown display server")),
        }
    }

    fn build_mic_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        if self.display_server == DisplayServer::X11 {
            build_x11_mic_pipeline(output_path, sample_rate)
        } else {
            build_mic_pipeline(output_path, sample_rate)
        }
    }

    fn build_system_audio_pipeline(
        &self,
        output_path: &Path,
        sample_rate: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        if self.display_server == DisplayServer::X11 {
            tracing::warn!(
                "System audio capture is not available on X11; continuing without system audio"
            );
            return Err(GrabmeError::unsupported(
                "System audio capture is not supported on X11 yet. Re-run with --no-system-audio",
            ));
        }

        build_system_audio_pipeline(output_path, sample_rate)
    }

    fn build_webcam_pipeline(
        &self,
        output_path: &Path,
        fps: u32,
    ) -> GrabmeResult<Box<dyn CapturePipeline>> {
        build_webcam_pipeline(output_path, fps)
    }

    fn get_display_server(&self) -> grabme_project_model::project::DisplayServer {
        match self.display_server {
            DisplayServer::Wayland => grabme_project_model::project::DisplayServer::Wayland,
            DisplayServer::X11 => grabme_project_model::project::DisplayServer::X11,
            _ => grabme_project_model::project::DisplayServer::Wayland,
        }
    }

    async fn shutdown(&mut self) -> GrabmeResult<()> {
        if let Some(handle) = self.portal_session_handle.take() {
            let _ = close_session(&handle).await;
        }
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
