//! GrabMe Processing Core — The Auto-Director
//!
//! Analyzes input event streams to generate automated editing decisions:
//! - **Auto-Zoom:** Detect activity regions and generate camera keyframes
//! - **Cursor Smoothing:** Apply motion smoothing algorithms to pointer data
//! - **Vertical Framing:** Generate 9:16 viewport that follows cursor
//!
//! This crate is pure computation — no I/O, no platform dependencies.
//! All inputs are data; all outputs are data.

pub mod auto_zoom;
pub mod cursor_smooth;
pub mod vertical;

pub use auto_zoom::AutoZoomAnalyzer;
pub use cursor_smooth::CursorSmoother;
