//! Input event types for the GrabMe event stream.
//!
//! Events are recorded in append-only JSONL format for crash safety.
//! All pointer coordinates are normalized to `[0.0, 1.0]` relative to
//! the capture region dimensions.

use serde::{Deserialize, Serialize};

/// Monotonic timestamp in nanoseconds since recording start.
pub type TimestampNs = u64;

/// A single recorded input event with timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputEvent {
    /// Monotonic nanoseconds since recording start.
    #[serde(rename = "t")]
    pub timestamp_ns: TimestampNs,

    /// The event payload.
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Discriminated union of event types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    /// Mouse/touchpad pointer position update.
    Pointer {
        /// Normalized X coordinate [0.0, 1.0].
        x: f64,
        /// Normalized Y coordinate [0.0, 1.0].
        y: f64,
    },

    /// Mouse button click.
    Click {
        /// Which button was pressed.
        button: MouseButton,
        /// Press or release.
        state: ButtonState,
        /// Pointer position at click time.
        x: f64,
        y: f64,
    },

    /// Keyboard key event.
    Key {
        /// Key code (e.g., "KeyA", "Enter", "ShiftLeft").
        code: String,
        /// Press or release.
        state: ButtonState,
    },

    /// Scroll wheel event.
    Scroll {
        /// Horizontal scroll delta (normalized).
        dx: f64,
        /// Vertical scroll delta (normalized).
        dy: f64,
        /// Pointer position at scroll time.
        x: f64,
        y: f64,
    },

    /// Window focus change.
    WindowFocus {
        /// Window title or identifier that gained focus.
        window_title: String,
        /// Application name / WM_CLASS.
        app_id: Option<String>,
    },
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// Button/key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonState {
    Down,
    Up,
}

/// Stream of events with recording metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamHeader {
    /// Schema version for forward compatibility.
    pub schema_version: String,

    /// Monotonic clock epoch: system monotonic time (ns) at recording start.
    pub epoch_monotonic_ns: u64,

    /// Wall-clock time at recording start (ISO 8601).
    pub epoch_wall: String,

    /// Capture region dimensions in physical pixels.
    pub capture_width: u32,
    pub capture_height: u32,

    /// Monitor scale factor at recording time.
    pub scale_factor: f64,

    /// Nominal sampling rate for pointer events (Hz).
    pub pointer_sample_rate_hz: u32,
}

impl InputEvent {
    /// Create a pointer event.
    pub fn pointer(timestamp_ns: TimestampNs, x: f64, y: f64) -> Self {
        Self {
            timestamp_ns,
            kind: EventKind::Pointer { x, y },
        }
    }

    /// Create a click event.
    pub fn click(
        timestamp_ns: TimestampNs,
        button: MouseButton,
        state: ButtonState,
        x: f64,
        y: f64,
    ) -> Self {
        Self {
            timestamp_ns,
            kind: EventKind::Click {
                button,
                state,
                x,
                y,
            },
        }
    }

    /// Create a key event.
    pub fn key(timestamp_ns: TimestampNs, code: impl Into<String>, state: ButtonState) -> Self {
        Self {
            timestamp_ns,
            kind: EventKind::Key {
                code: code.into(),
                state,
            },
        }
    }

    /// Timestamp as fractional seconds since recording start.
    pub fn timestamp_secs(&self) -> f64 {
        self.timestamp_ns as f64 / 1_000_000_000.0
    }

    /// Extract pointer position if this event contains one.
    pub fn pointer_position(&self) -> Option<(f64, f64)> {
        match &self.kind {
            EventKind::Pointer { x, y } => Some((*x, *y)),
            EventKind::Click { x, y, .. } => Some((*x, *y)),
            EventKind::Scroll { x, y, .. } => Some((*x, *y)),
            _ => None,
        }
    }
}

/// Parse events from JSONL content (one JSON object per line).
pub fn parse_events(jsonl: &str) -> Result<Vec<InputEvent>, serde_json::Error> {
    jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line))
        .collect()
}

/// Serialize events to JSONL format.
pub fn serialize_events(events: &[InputEvent]) -> Result<String, serde_json::Error> {
    let mut output = String::new();
    for event in events {
        output.push_str(&serde_json::to_string(event)?);
        output.push('\n');
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pointer_event_roundtrip() {
        let event = InputEvent::pointer(1_000_000_000, 0.5, 0.3);
        let json = serde_json::to_string(&event).unwrap();
        let parsed: InputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_click_event_roundtrip() {
        let event = InputEvent::click(
            2_000_000_000,
            MouseButton::Left,
            ButtonState::Down,
            0.1,
            0.9,
        );
        let json = serde_json::to_string(&event).unwrap();
        let parsed: InputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_key_event_roundtrip() {
        let event = InputEvent::key(3_000_000_000, "KeyA", ButtonState::Down);
        let json = serde_json::to_string(&event).unwrap();
        let parsed: InputEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_jsonl_roundtrip() {
        let events = vec![
            InputEvent::pointer(0, 0.0, 0.0),
            InputEvent::click(100_000_000, MouseButton::Left, ButtonState::Down, 0.5, 0.5),
            InputEvent::pointer(200_000_000, 0.6, 0.4),
        ];
        let jsonl = serialize_events(&events).unwrap();
        let parsed = parse_events(&jsonl).unwrap();
        assert_eq!(events, parsed);
    }

    #[test]
    fn test_pointer_position_extraction() {
        let ptr = InputEvent::pointer(0, 0.3, 0.7);
        assert_eq!(ptr.pointer_position(), Some((0.3, 0.7)));

        let click = InputEvent::click(0, MouseButton::Left, ButtonState::Down, 0.1, 0.2);
        assert_eq!(click.pointer_position(), Some((0.1, 0.2)));

        let key = InputEvent::key(0, "KeyA", ButtonState::Down);
        assert_eq!(key.pointer_position(), None);
    }

    #[test]
    fn test_timestamp_secs() {
        let event = InputEvent::pointer(1_500_000_000, 0.0, 0.0);
        assert!((event.timestamp_secs() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_json_format_matches_spec() {
        // Verify the JSON matches the format defined in PRD
        let event = InputEvent::pointer(1234567890123, 0.5, 0.3);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"t\":1234567890123"));
        assert!(json.contains("\"type\":\"pointer\""));
        assert!(json.contains("\"x\":0.5"));
        assert!(json.contains("\"y\":0.3"));
    }
}
