//! GrabMe Common Utilities
//!
//! Shared infrastructure for all GrabMe crates:
//! - Error types and result aliases
//! - Clock and timing utilities for stream synchronization
//! - Tracing/logging initialization
//! - Configuration loading

pub mod clock;
pub mod config;
pub mod error;
pub mod logging;

pub use clock::*;
pub use config::*;
pub use error::*;
