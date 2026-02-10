//! GrabMe Render Engine
//!
//! Offline rendering pipeline that composites raw source media
//! with editing decisions (zoom, cursor, webcam, subtitles)
//! into final exported video files.
//!
//! # Pipeline Architecture
//!
//! ```text
//! source.mkv ──┐
//!              ├── Crop/Scale (zoom keyframes)
//! timeline ────┘         │
//!                        ├── Cursor Overlay
//! events.jsonl ──────────┘         │
//!                                  ├── Webcam Composite
//! webcam.mkv ──────────────────────┘         │
//!                                            ├── Subtitle Burn
//! subtitles.srt ─────────────────────────────┘         │
//!                                                      ▼
//!                                               Encode (H.264)
//!                                                      │
//!                                                      ▼
//!                                                  output.mp4
//! ```

pub mod compositor;
pub mod export;

pub use export::*;
