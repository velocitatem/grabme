//! Input tracking backend implementations.
//!
//! Each backend provides a different way to capture mouse/keyboard input.

use grabme_common::error::GrabmeResult;
use grabme_project_model::event::InputEvent;

use crate::InputBackend;

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
    // TODO: Implement real detection logic:
    // 1. Check if running under Wayland or X11
    // 2. Check if evdev access is available
    // 3. Fall back to portal metadata
    tracing::warn!("Using stub input backend — no real input capture");
    Box::new(StubBackend::empty())
}
