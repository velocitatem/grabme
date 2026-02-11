//! Input tracking backend implementations.
//!
//! Each backend provides a different way to capture mouse/keyboard input.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

use grabme_common::error::GrabmeResult;
use grabme_project_model::event::InputEvent;
use grabme_project_model::event::{ButtonState, MouseButton};

use crate::InputBackend;

const LEFT_BUTTON: usize = 0;
const RIGHT_BUTTON: usize = 1;
const MIDDLE_BUTTON: usize = 2;
const POINTER_RESYNC_INTERVAL: Duration = Duration::from_millis(75);

pub struct EvdevBackend {
    device: std::fs::File,
    pending: VecDeque<InputEvent>,
    x: f64,
    y: f64,
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
    last_resync: Instant,
    button_state: [bool; 3],
}

impl EvdevBackend {
    pub fn new() -> GrabmeResult<Self> {
        let device = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/input/mice")
            .map_err(|e| {
                grabme_common::error::GrabmeError::input_tracking(format!(
                    "Failed to open /dev/input/mice: {e}"
                ))
            })?;

        let (origin_x, origin_y, width, height) = desktop_geometry();
        let (x, y) = initial_pointer_position(origin_x, origin_y, width, height);

        Ok(Self {
            device,
            pending: VecDeque::new(),
            x,
            y,
            origin_x,
            origin_y,
            width,
            height,
            last_resync: Instant::now(),
            button_state: [false, false, false],
        })
    }

    pub fn is_supported() -> bool {
        OpenOptions::new()
            .read(true)
            .open("/dev/input/mice")
            .is_ok()
    }

    fn ingest_packets(&mut self) -> GrabmeResult<()> {
        loop {
            let mut packet = [0u8; 3];
            match self.device.read(&mut packet) {
                Ok(3) => {
                    self.process_packet(packet);
                }
                Ok(0) => break,
                Ok(_) => break,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    return Err(grabme_common::error::GrabmeError::input_tracking(format!(
                        "Failed reading /dev/input/mice: {err}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn process_packet(&mut self, packet: [u8; 3]) {
        let dx = packet[1] as i8 as f64;
        let dy = packet[2] as i8 as f64;

        self.x = (self.x + dx / self.width).clamp(0.0, 1.0);
        self.y = (self.y - dy / self.height).clamp(0.0, 1.0);

        // /dev/input/mice reports relative counts, not compositor-space pixels.
        // Periodically snap to absolute X11 pointer position to prevent drift.
        if self.last_resync.elapsed() >= POINTER_RESYNC_INTERVAL {
            if let Some((rx, ry)) =
                query_pointer_position(self.origin_x, self.origin_y, self.width, self.height)
            {
                self.x = rx;
                self.y = ry;
            }
            self.last_resync = Instant::now();
        }

        self.pending
            .push_back(InputEvent::pointer(0, self.x, self.y));

        let left = packet[0] & 0b001 != 0;
        let right = packet[0] & 0b010 != 0;
        let middle = packet[0] & 0b100 != 0;

        self.push_button_transition(LEFT_BUTTON, left, MouseButton::Left);
        self.push_button_transition(RIGHT_BUTTON, right, MouseButton::Right);
        self.push_button_transition(MIDDLE_BUTTON, middle, MouseButton::Middle);
    }

    fn push_button_transition(&mut self, idx: usize, now: bool, button: MouseButton) {
        let previous = self.button_state[idx];
        if previous == now {
            return;
        }

        self.button_state[idx] = now;
        let state = if now {
            ButtonState::Down
        } else {
            ButtonState::Up
        };
        self.pending
            .push_back(InputEvent::click(0, button, state, self.x, self.y));
    }
}

impl InputBackend for EvdevBackend {
    fn poll(&mut self) -> GrabmeResult<Option<InputEvent>> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }

        self.ingest_packets()?;
        Ok(self.pending.pop_front())
    }

    fn name(&self) -> &str {
        "evdev"
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Stub backend for testing — generates synthetic events.
pub struct StubBackend {
    events: Vec<InputEvent>,
    index: usize,
}

impl StubBackend {
    /// Create a stub backend with pre-loaded events.
    pub fn new(events: Vec<InputEvent>) -> Self {
        Self { events, index: 0 }
    }

    /// Create an empty stub that never produces events.
    pub fn empty() -> Self {
        Self {
            events: vec![],
            index: 0,
        }
    }
}

impl InputBackend for StubBackend {
    fn poll(&mut self) -> GrabmeResult<Option<InputEvent>> {
        if self.index < self.events.len() {
            let event = self.events[self.index].clone();
            self.index += 1;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    fn name(&self) -> &str {
        "stub"
    }

    fn is_available(&self) -> bool {
        true
    }
}

// Phase 1 will add:
// - EvdevBackend: reads from /dev/input/event* (requires input group)
// - X11Backend: uses XInput2 for X11 sessions
// - PortalBackend: uses XDG Desktop Portal cursor metadata (future)

/// Detect the best available input backend for the current system.
pub fn detect_best_backend() -> Box<dyn InputBackend> {
    if EvdevBackend::is_supported() {
        match EvdevBackend::new() {
            Ok(backend) => {
                tracing::info!("Using evdev backend");
                return Box::new(backend);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize evdev backend, using stub");
            }
        }
    }

    tracing::warn!(
        details = %mice_device_diagnostic(),
        "Using stub input backend — pointer/click events will not be captured"
    );
    Box::new(StubBackend::empty())
}

fn mice_device_diagnostic() -> String {
    let path = "/dev/input/mice";
    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };

    match std::fs::metadata(path) {
        Ok(meta) => {
            let mode = meta.mode() & 0o777;
            let owner = meta.uid();
            let group = meta.gid();
            format!(
                "device={path} mode={mode:o} owner_uid={owner} owner_gid={group} process_uid={uid} process_gid={gid}; likely missing 'input' group membership. Fix: sudo usermod -aG input $USER && log out/in"
            )
        }
        Err(err) => format!(
            "device={path} unavailable ({err}); ensure kernel input device exists and permissions allow read access"
        ),
    }
}

fn desktop_geometry() -> (f64, f64, f64, f64) {
    match grabme_platform_linux::detect_monitors() {
        Ok(monitors) if !monitors.is_empty() => {
            let (origin_x, origin_y, width, height) =
                grabme_platform_linux::virtual_desktop_bounds(&monitors);
            let width = width as f64;
            let height = height as f64;
            (
                origin_x as f64,
                origin_y as f64,
                width.max(1.0),
                height.max(1.0),
            )
        }
        _ => (0.0, 0.0, 1920.0, 1080.0),
    }
}

fn initial_pointer_position(origin_x: f64, origin_y: f64, width: f64, height: f64) -> (f64, f64) {
    query_pointer_position(origin_x, origin_y, width, height).unwrap_or((0.5, 0.5))
}

fn query_pointer_position(
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    // On X11 we can get an initial absolute cursor position via xdotool.
    // This anchors relative /dev/input/mice deltas and improves mapping.
    let output = Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output();

    let Ok(output) = output else {
        return None;
    };
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut x_px = None;
    let mut y_px = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("X=") {
            x_px = rest.trim().parse::<f64>().ok();
        } else if let Some(rest) = line.strip_prefix("Y=") {
            y_px = rest.trim().parse::<f64>().ok();
        }
    }

    let Some(x_px) = x_px else {
        return None;
    };
    let Some(y_px) = y_px else {
        return None;
    };

    let (x, y) = normalize_virtual_point(x_px, y_px, origin_x, origin_y, width, height);
    Some((x, y))
}

fn normalize_virtual_point(
    pixel_x: f64,
    pixel_y: f64,
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let nx = ((pixel_x - origin_x) / width).clamp(0.0, 1.0);
    let ny = ((pixel_y - origin_y) / height).clamp(0.0, 1.0);
    (nx, ny)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_virtual_point_with_zero_origin() {
        let (x, y) = normalize_virtual_point(960.0, 540.0, 0.0, 0.0, 1920.0, 1080.0);
        assert!((x - 0.5).abs() < 1e-9);
        assert!((y - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_normalize_virtual_point_with_negative_origin() {
        let (x, y) = normalize_virtual_point(0.0, 0.0, -1920.0, 0.0, 4480.0, 1440.0);
        assert!((x - (1920.0 / 4480.0)).abs() < 1e-9);
        assert!(y.abs() < 1e-9);
    }
}
