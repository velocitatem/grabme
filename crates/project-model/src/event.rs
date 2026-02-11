//! Input event types for the GrabMe event stream.
//!
//! Events are recorded in append-only JSONL format for crash safety.
//! All pointer coordinates are normalized to `[0.0, 1.0]` relative to
//! the capture region dimensions.

use serde::{Deserialize, Serialize};

/// Monotonic timestamp in nanoseconds since recording start.
pub type TimestampNs = u64;

/// Coordinate space used by recorded pointer values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PointerCoordinateSpace {
    /// Coordinates are normalized directly against the captured region.
    CaptureNormalized,
    /// Coordinates are normalized against virtual desktop bounds.
    VirtualDesktopNormalized,
    /// Legacy variant normalized against root-origin virtual desktop.
    VirtualDesktopRootOrigin,
    /// Older recordings did not label coordinate-space explicitly.
    #[default]
    LegacyUnspecified,
}

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

    /// Coordinate-space contract for pointer x/y values.
    #[serde(default)]
    pub pointer_coordinate_space: PointerCoordinateSpace,
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
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(serde_json::from_str)
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
    fn test_parse_events_skips_header_comment() {
        let jsonl =
            "# {\"schema_version\":\"1.0\"}\n{\"t\":0,\"type\":\"pointer\",\"x\":0.5,\"y\":0.3}\n";
        let parsed = parse_events(jsonl).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].timestamp_ns, 0);
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

    #[test]
    fn test_event_header_defaults_pointer_coordinate_space_for_legacy_files() {
        let raw = r#"{
            "schema_version":"1.0",
            "epoch_monotonic_ns":0,
            "epoch_wall":"2026-01-01T00:00:00Z",
            "capture_width":1920,
            "capture_height":1080,
            "scale_factor":1.0,
            "pointer_sample_rate_hz":60
        }"#;

        let parsed: EventStreamHeader = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed.pointer_coordinate_space,
            PointerCoordinateSpace::LegacyUnspecified
        );
    }

    #[test]
    fn test_phase1_ten_minute_fixture_exists_and_is_monotonic() {
        use std::path::PathBuf;

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("sample-project")
            .join("meta")
            .join("events.jsonl");

        let content = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();

        assert!(lines.first().unwrap().starts_with("# "));

        let mut prev_ts = None;
        let mut last_ts = 0u64;
        for line in lines.iter().skip(1) {
            let event: InputEvent = serde_json::from_str(line).unwrap();
            if let Some(prev) = prev_ts {
                assert!(event.timestamp_ns >= prev);
            }
            last_ts = event.timestamp_ns;
            prev_ts = Some(event.timestamp_ns);
        }

        let start = serde_json::from_str::<InputEvent>(lines[1])
            .unwrap()
            .timestamp_ns;
        let duration_secs = (last_ts - start) as f64 / 1_000_000_000.0;
        assert!(duration_secs >= 600.0);
    }
}
