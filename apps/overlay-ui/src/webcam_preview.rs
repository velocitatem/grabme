use std::process::{Child, Command, Stdio};

/// Lightweight live webcam preview controller.
///
/// Current implementation shells out to `ffplay` for a low-latency preview
/// window. This keeps recording pipelines non-destructive and isolated.
pub struct WebcamPreview {
    child: Option<Child>,
}

impl WebcamPreview {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }

        if !command_exists("ffplay") {
            return Err("ffplay not found in PATH".to_string());
        }

        let device = detect_default_webcam_device()
            .ok_or_else(|| "No webcam device found under /dev/video*".to_string())?;

        let child = Command::new("ffplay")
            .args([
                "-loglevel",
                "error",
                "-nostats",
                "-fflags",
                "nobuffer",
                "-flags",
                "low_delay",
                "-f",
                "video4linux2",
                "-video_size",
                "320x180",
                "-window_title",
                "GrabMe Webcam Preview",
                "-i",
                &device,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("Failed to start webcam preview: {err}"))?;

        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.child = None;
                    false
                }
            },
            None => false,
        }
    }
}

impl Drop for WebcamPreview {
    fn drop(&mut self) {
        self.stop();
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

fn command_exists(binary: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {binary} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
