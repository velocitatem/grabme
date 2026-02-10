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

pub mod pipeline;
pub mod session;

pub use session::*;
