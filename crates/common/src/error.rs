//! Error types shared across GrabMe crates.

use std::path::PathBuf;

/// Top-level error type for GrabMe operations.
#[derive(Debug, thiserror::Error)]
pub enum GrabmeError {
    #[error("Capture error: {message}")]
    Capture { message: String },

    #[error("Input tracking error: {message}")]
    InputTracking { message: String },

    #[error("Processing error: {message}")]
    Processing { message: String },

    #[error("Render error: {message}")]
    Render { message: String },

    #[error("Project error: {message}")]
    Project { message: String },

    #[error("Audio error: {message}")]
    Audio { message: String },

    #[error("Platform error: {message}")]
    Platform { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("Unsupported operation: {message}")]
    Unsupported { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias using GrabmeError.
pub type GrabmeResult<T> = Result<T, GrabmeError>;

impl GrabmeError {
    pub fn capture(msg: impl Into<String>) -> Self {
        Self::Capture {
            message: msg.into(),
        }
    }

    pub fn input_tracking(msg: impl Into<String>) -> Self {
        Self::InputTracking {
            message: msg.into(),
        }
    }

    pub fn processing(msg: impl Into<String>) -> Self {
        Self::Processing {
            message: msg.into(),
        }
    }

    pub fn render(msg: impl Into<String>) -> Self {
        Self::Render {
            message: msg.into(),
        }
    }

    pub fn platform(msg: impl Into<String>) -> Self {
        Self::Platform {
            message: msg.into(),
        }
    }

    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported {
            message: msg.into(),
        }
    }
}
