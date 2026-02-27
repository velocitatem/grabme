//! GrabMe Capture Engine
//!
//! Orchestrates screen, webcam, and audio capture into a project bundle.
//! The capture engine runs real-time recording sessions, writing raw media
//! files and coordinating the input tracker for event logging.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │              CaptureSession                  │
//! │  ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
//! │  │ Screen   │ │ Audio    │ │ InputTracker │ │
//! │  │ Capturer │ │ Capturer │ │              │ │
//! │  └─────┬────┘ └─────┬────┘ └──────┬───────┘ │
//! │        │            │             │          │
//! │        ▼            ▼             ▼          │
//! │  ┌─────────────────────────────────────────┐ │
//! │  │         Project Bundle (Disk)           │ │
//! │  │  screen.mkv  mic.wav  events.jsonl      │ │
//! │  └─────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────┘
//! ```

pub mod backend;
pub mod pipeline;
pub mod session;

pub use session::*;

/// Detect and return all connected monitors using the platform backend.
/// Returns monitors in enumeration order — the index in this list corresponds
/// to the `--monitor N` argument passed to `grabme record`.
pub fn list_monitors() -> grabme_common::error::GrabmeResult<Vec<grabme_platform_core::MonitorInfo>> {
    backend::get_backend().detect_monitors()
}
