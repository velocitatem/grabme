//! Local transcription using Whisper.
//!
//! Runs speech-to-text inference locally (no cloud APIs).
//! Uses whisper.cpp bindings for efficient CPU inference.

use grabme_common::error::{GrabmeError, GrabmeResult};
use serde::{Deserialize, Serialize};

/// Whisper model size selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModel {
    /// Fastest, least accurate (~39 MB).
    Tiny,
    /// Good balance of speed and accuracy (~142 MB).
    Base,
    /// Better accuracy, slower (~466 MB).
    Small,
    /// High accuracy (~1.5 GB).
    Medium,
    /// Best accuracy, slowest (~2.9 GB).
    Large,
}

impl WhisperModel {
    /// Approximate model file size in bytes.
    pub fn size_bytes(&self) -> u64 {
        match self {
            WhisperModel::Tiny => 39_000_000,
            WhisperModel::Base => 142_000_000,
            WhisperModel::Small => 466_000_000,
            WhisperModel::Medium => 1_500_000_000,
            WhisperModel::Large => 2_900_000_000,
        }
    }

    /// Model filename.
    pub fn filename(&self) -> &str {
        match self {
            WhisperModel::Tiny => "ggml-tiny.bin",
            WhisperModel::Base => "ggml-base.bin",
            WhisperModel::Small => "ggml-small.bin",
            WhisperModel::Medium => "ggml-medium.bin",
            WhisperModel::Large => "ggml-large.bin",
        }
    }
}

/// Configuration for transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Model to use.
    pub model: WhisperModel,

    /// Language hint (ISO 639-1 code, e.g., "en").
    pub language: Option<String>,

    /// Whether to translate to English.
    pub translate: bool,

    /// Number of CPU threads for inference.
    pub threads: u32,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            model: WhisperModel::Base,
            language: Some("en".to_string()),
            translate: false,
            threads: 4,
        }
    }
}

/// A single transcribed segment with timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Start time in seconds.
    pub start_secs: f64,
    /// End time in seconds.
    pub end_secs: f64,
    /// Transcribed text.
    pub text: String,
    /// Confidence score [0.0, 1.0] (if available).
    pub confidence: Option<f64>,
}

/// Result of a transcription job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Detected language.
    pub language: String,
    /// Transcribed segments.
    pub segments: Vec<TranscriptionSegment>,
    /// Total duration processed.
    pub duration_secs: f64,
    /// Processing time in seconds.
    pub processing_time_secs: f64,
}

/// Transcribe an audio file.
///
/// This is the main entry point for transcription.
pub fn transcribe(
    audio_path: &std::path::Path,
    config: &TranscriptionConfig,
) -> GrabmeResult<TranscriptionResult> {
    tracing::info!(
        path = %audio_path.display(),
        model = ?config.model,
        "Starting transcription"
    );

    if !audio_path.exists() {
        return Err(GrabmeError::FileNotFound {
            path: audio_path.to_path_buf(),
        });
    }

    // TODO: Phase 5 implementation:
    // 1. Load whisper model from models/ directory
    // 2. Resample audio to 16kHz mono (required by Whisper)
    // 3. Run inference
    // 4. Parse segments and timestamps

    Err(GrabmeError::unsupported(
        "Transcription will be implemented in Phase 5 (whisper.cpp integration)",
    ))
}
