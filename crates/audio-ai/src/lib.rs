//! GrabMe Audio Intelligence
//!
//! Local-first audio processing:
//! - **Transcription:** Whisper-based speech-to-text for subtitle generation
//! - **Noise Suppression:** RNNoise-based noise gate and cleanup
//! - **Subtitle Generation:** SRT/VTT output from transcription results

pub mod noise;
pub mod subtitles;
pub mod transcription;

pub use subtitles::*;
pub use transcription::*;
