//! Non-Linux input backend implementations.

use grabme_common::error::GrabmeResult;
use grabme_project_model::event::{InputEvent, PointerCoordinateSpace};

use crate::InputBackend;

pub struct StubBackend {
    events: Vec<InputEvent>,
    index: usize,
}

impl StubBackend {
    pub fn new(events: Vec<InputEvent>) -> Self {
        Self { events, index: 0 }
    }

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

    fn pointer_coordinate_space(&self) -> PointerCoordinateSpace {
        PointerCoordinateSpace::CaptureNormalized
    }
}

pub fn detect_best_backend() -> Box<dyn InputBackend> {
    tracing::warn!(
        "Input capture backends for this platform are not implemented yet; using stub backend"
    );
    Box::new(StubBackend::empty())
}
