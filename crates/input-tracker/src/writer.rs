//! Append-only event writer for crash-safe event logging.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use grabme_common::error::{GrabmeError, GrabmeResult};
use grabme_project_model::event::{EventStreamHeader, InputEvent};

/// Writes events to a JSONL file in append-only mode.
pub struct EventWriter {
    writer: BufWriter<File>,
    path: PathBuf,
    events_written: u64,
}

impl EventWriter {
    /// Create a new event writer, writing the header as the first line.
    pub fn new(path: PathBuf, header: EventStreamHeader) -> GrabmeResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        let mut writer = BufWriter::new(file);

        // Write header as a comment line (prefixed with #)
        let header_json = serde_json::to_string(&header)?;
        writeln!(writer, "# {header_json}")
            .map_err(|e| GrabmeError::capture(format!("Failed to write header: {e}")))?;

        Ok(Self {
            writer,
            path,
            events_written: 0,
        })
    }

    /// Write a single event as a JSONL line.
    pub fn write_event(&mut self, event: &InputEvent) -> GrabmeResult<()> {
        let json = serde_json::to_string(event)?;
        writeln!(self.writer, "{json}")
            .map_err(|e| GrabmeError::capture(format!("Failed to write event: {e}")))?;
        self.events_written += 1;

        // Flush every 1000 events for crash safety
        if self.events_written % 1000 == 0 {
            self.flush()?;
        }

        Ok(())
    }

    /// Flush buffered writes to disk.
    pub fn flush(&mut self) -> GrabmeResult<()> {
        self.writer
            .flush()
            .map_err(|e| GrabmeError::capture(format!("Failed to flush events: {e}")))?;
        Ok(())
    }

    /// Number of events written.
    pub fn events_written(&self) -> u64 {
        self.events_written
    }

    /// Path to the output file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grabme_project_model::event::{
        ButtonState, InputEvent, MouseButton, PointerCoordinateSpace,
    };

    #[test]
    fn test_event_writer_roundtrip() {
        let dir = std::env::temp_dir().join("grabme_test_writer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("events.jsonl");
        let header = EventStreamHeader {
            schema_version: "1.0".to_string(),
            epoch_monotonic_ns: 0,
            epoch_wall: "2026-01-01T00:00:00Z".to_string(),
            capture_width: 1920,
            capture_height: 1080,
            scale_factor: 1.0,
            pointer_sample_rate_hz: 60,
            pointer_coordinate_space: PointerCoordinateSpace::CaptureNormalized,
        };

        {
            let mut writer = EventWriter::new(path.clone(), header).unwrap();
            writer
                .write_event(&InputEvent::pointer(0, 0.5, 0.5))
                .unwrap();
            writer
                .write_event(&InputEvent::click(
                    100_000_000,
                    MouseButton::Left,
                    ButtonState::Down,
                    0.5,
                    0.5,
                ))
                .unwrap();
            writer
                .write_event(&InputEvent::pointer(200_000_000, 0.6, 0.4))
                .unwrap();
            assert_eq!(writer.events_written(), 3);
        }

        // Read back and verify
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // 1 header + 3 events
        assert!(lines[0].starts_with("# "));

        // Parse non-header lines
        let events: Vec<InputEvent> = lines[1..]
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }
}
