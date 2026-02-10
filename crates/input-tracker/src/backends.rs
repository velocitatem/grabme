//! Input tracking backend implementations.
//!
//! Each backend provides a different way to capture mouse/keyboard input.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;

use grabme_common::error::GrabmeResult;
use grabme_project_model::event::InputEvent;
use grabme_project_model::event::{ButtonState, MouseButton};

use crate::InputBackend;

const LEFT_BUTTON: usize = 0;
const RIGHT_BUTTON: usize = 1;
const MIDDLE_BUTTON: usize = 2;

pub struct EvdevBackend {
    device: std::fs::File,
    pending: VecDeque<InputEvent>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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

        let (width, height) = desktop_size();

        Ok(Self {
            device,
            pending: VecDeque::new(),
            x: 0.5,
            y: 0.5,
            width,
            height,
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

fn desktop_size() -> (f64, f64) {
    match grabme_platform_linux::detect_monitors() {
        Ok(monitors) if !monitors.is_empty() => {
            let width = monitors.iter().map(|m| m.width).max().unwrap_or(1920) as f64;
            let height = monitors.iter().map(|m| m.height).max().unwrap_or(1080) as f64;
            (width, height)
        }
        _ => (1920.0, 1080.0),
    }
}
