//! GrabMe Linux Platform Integration
//!
//! Platform-specific implementations for Linux:
//! - **XDG Desktop Portal:** Screen capture negotiation via DBus
//! - **PipeWire:** Audio/video stream management
//! - **Display Detection:** Monitor enumeration and DPI handling
//! - **Permissions:** Capability detection and user guidance

pub mod display;
pub mod permissions;
pub mod portal;

pub use display::*;
