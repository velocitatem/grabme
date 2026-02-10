//! Application configuration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Global application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Directory where projects are stored.
    pub projects_dir: PathBuf,

    /// Default recording settings.
    pub recording: RecordingDefaults,

    /// Logging configuration.
    pub logging: LoggingConfig,
}

/// Default recording parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingDefaults {
    /// Default FPS.
    pub fps: u32,

    /// Default pointer sampling rate (Hz).
    pub pointer_sample_rate_hz: u32,

    /// Default video codec.
    pub video_codec: String,

    /// Default audio sample rate.
    pub audio_sample_rate: u32,

    /// Whether to hide cursor during capture by default.
    pub hide_cursor: bool,
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level filter (e.g., "info", "debug", "grabme=debug,warn").
    pub level: String,

    /// Whether to output structured JSON logs.
    pub json: bool,

    /// Optional log file path.
    pub file: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            projects_dir: dirs_default_projects(),
            recording: RecordingDefaults::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for RecordingDefaults {
    fn default() -> Self {
        Self {
            fps: 60,
            pointer_sample_rate_hz: 60,
            video_codec: "h264".to_string(),
            audio_sample_rate: 48000,
            hide_cursor: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json: false,
            file: None,
        }
    }
}

impl AppConfig {
    /// Load config from the standard location, falling back to defaults.
    pub fn load() -> Self {
        let config_path = config_file_path();
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        tracing::warn!("Failed to parse config at {:?}: {}", config_path, e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read config at {:?}: {}", config_path, e);
                }
            }
        }
        Self::default()
    }

    /// Save config to the standard location.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_path = config_file_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(config_path, json)
    }
}

/// Standard config file location.
fn config_file_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("grabme").join("config.json")
}

/// Default projects directory.
fn dirs_default_projects() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("grabme").join("projects")
}
