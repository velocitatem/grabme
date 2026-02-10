//! GrabMe Project Model
//!
//! Defines the core data contracts for GrabMe projects:
//! - **Events:** Timestamped input events (pointer, click, key, window focus)
//! - **Timeline:** Editing decisions (zoom keyframes, camera regions, effects)
//! - **Project:** Top-level metadata, tracks, and export configuration
//!
//! All coordinates are normalized to `[0.0, 1.0]` range relative to the
//! capture region to survive DPI/scaling changes across sessions.

pub mod event;
pub mod project;
pub mod timeline;
pub mod viewport;

pub use event::*;
pub use project::*;
pub use timeline::*;
pub use viewport::*;
