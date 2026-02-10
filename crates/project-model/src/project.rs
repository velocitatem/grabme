//! Project metadata and configuration types.
//!
//! A project is the top-level container that ties together source media,
//! event streams, timeline decisions, and export configuration.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::timeline::Timeline;

/// Top-level project file (`project.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Schema version.
    pub version: String,

    /// Human-readable project name.
    pub name: String,

    /// Unique project identifier (UUID).
    pub id: String,

    /// Creation timestamp (ISO 8601).
    pub created_at: String,

    /// Last modified timestamp (ISO 8601).
    pub modified_at: String,

    /// Recording configuration that was used.
    pub recording: RecordingConfig,

    /// Source media tracks.
    pub tracks: Tracks,

    /// Export configuration.
    pub export: ExportConfig,
}

/// Configuration used during recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    /// Capture resolution (physical pixels).
    pub capture_width: u32,
    pub capture_height: u32,

    /// Recording frame rate.
    pub fps: u32,

    /// Monitor scale factor (e.g., 1.0, 1.25, 2.0).
    pub scale_factor: f64,

    /// Display server used.
    pub display_server: DisplayServer,

    /// Whether the system cursor was hidden during capture.
    pub cursor_hidden: bool,

    /// Audio sample rate.
    pub audio_sample_rate: u32,
}

/// Display server type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayServer {
    Wayland,
    X11,
}

/// References to source media files (relative to project root).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tracks {
    /// Screen capture video.
    pub screen: Option<TrackRef>,

    /// Webcam video.
    pub webcam: Option<TrackRef>,

    /// Microphone audio.
    pub mic: Option<TrackRef>,

    /// System/desktop audio.
    pub system_audio: Option<TrackRef>,

    /// Per-application audio tracks.
    #[serde(default)]
    pub app_audio: Vec<AppAudioTrack>,
}

/// Reference to a media file with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRef {
    /// Relative path from project root to the media file.
    pub path: String,

    /// Duration in seconds.
    pub duration_secs: f64,

    /// Codec used.
    pub codec: String,

    /// Offset in nanoseconds from recording epoch.
    /// Used to synchronize tracks that may have started at different times.
    #[serde(default)]
    pub offset_ns: i64,
}

/// Per-application audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppAudioTrack {
    /// Application name or PID.
    pub app_name: String,

    /// Track reference.
    pub track: TrackRef,
}

/// Export configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Output format.
    pub format: ExportFormat,

    /// Output resolution (width x height in pixels).
    pub width: u32,
    pub height: u32,

    /// Output frame rate.
    pub fps: u32,

    /// Video bitrate in kbps (0 = auto).
    pub video_bitrate_kbps: u32,

    /// Audio bitrate in kbps.
    pub audio_bitrate_kbps: u32,

    /// Aspect ratio mode.
    pub aspect_mode: AspectMode,

    /// Whether to burn subtitles into the video.
    #[serde(default)]
    pub burn_subtitles: bool,
}

/// Output video format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[serde(rename = "mp4-h264")]
    Mp4H264,
    #[serde(rename = "mp4-h265")]
    Mp4H265,
    Gif,
    Webm,
}

/// Aspect ratio / framing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectMode {
    /// Standard 16:9 widescreen.
    Landscape,
    /// 9:16 vertical (social media).
    Portrait,
    /// 1:1 square.
    Square,
    /// Custom aspect ratio.
    Custom,
}

/// The complete in-memory representation of a loaded project.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    /// Filesystem path to the project directory.
    pub root: PathBuf,

    /// Project metadata.
    pub project: Project,

    /// Editing timeline.
    pub timeline: Timeline,
}

impl Project {
    /// Create a new project with defaults.
    pub fn new(name: impl Into<String>, width: u32, height: u32, fps: u32) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: "1.0".to_string(),
            name: name.into(),
            id: uuid_v4(),
            created_at: now.clone(),
            modified_at: now,
            recording: RecordingConfig {
                capture_width: width,
                capture_height: height,
                fps,
                scale_factor: 1.0,
                display_server: DisplayServer::Wayland,
                cursor_hidden: true,
                audio_sample_rate: 48000,
            },
            tracks: Tracks {
                screen: None,
                webcam: None,
                mic: None,
                system_audio: None,
                app_audio: vec![],
            },
            export: ExportConfig {
                format: ExportFormat::Mp4H264,
                width,
                height,
                fps,
                video_bitrate_kbps: 8000,
                audio_bitrate_kbps: 192,
                aspect_mode: AspectMode::Landscape,
                burn_subtitles: false,
            },
        }
    }
}

impl LoadedProject {
    /// Load a project from a directory.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let root = root.as_ref().to_path_buf();

        let project_path = root.join("meta").join("project.json");
        let timeline_path = root.join("meta").join("timeline.json");

        let project_json =
            std::fs::read_to_string(&project_path).map_err(|e| ProjectError::IoError {
                path: project_path.clone(),
                source: e,
            })?;

        let project: Project =
            serde_json::from_str(&project_json).map_err(|e| ProjectError::ParseError {
                path: project_path,
                source: e,
            })?;

        let timeline = if timeline_path.exists() {
            let timeline_json =
                std::fs::read_to_string(&timeline_path).map_err(|e| ProjectError::IoError {
                    path: timeline_path.clone(),
                    source: e,
                })?;
            serde_json::from_str(&timeline_json).map_err(|e| ProjectError::ParseError {
                path: timeline_path,
                source: e,
            })?
        } else {
            Timeline::new()
        };

        Ok(Self {
            root,
            project,
            timeline,
        })
    }

    /// Save project and timeline to disk.
    pub fn save(&self) -> Result<(), ProjectError> {
        let meta_dir = self.root.join("meta");
        std::fs::create_dir_all(&meta_dir).map_err(|e| ProjectError::IoError {
            path: meta_dir.clone(),
            source: e,
        })?;

        let project_path = meta_dir.join("project.json");
        let project_json =
            serde_json::to_string_pretty(&self.project).map_err(|e| ProjectError::ParseError {
                path: project_path.clone(),
                source: e,
            })?;
        std::fs::write(&project_path, project_json).map_err(|e| ProjectError::IoError {
            path: project_path,
            source: e,
        })?;

        let timeline_path = meta_dir.join("timeline.json");
        let timeline_json =
            serde_json::to_string_pretty(&self.timeline).map_err(|e| ProjectError::ParseError {
                path: timeline_path.clone(),
                source: e,
            })?;
        std::fs::write(&timeline_path, timeline_json).map_err(|e| ProjectError::IoError {
            path: timeline_path,
            source: e,
        })?;

        Ok(())
    }

    /// Create a new project on disk with the standard directory structure.
    pub fn create(
        root: impl AsRef<Path>,
        name: impl Into<String>,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self, ProjectError> {
        let root = root.as_ref().to_path_buf();

        // Create directory structure
        for subdir in &["sources", "meta", "cache", "exports"] {
            std::fs::create_dir_all(root.join(subdir)).map_err(|e| ProjectError::IoError {
                path: root.join(subdir),
                source: e,
            })?;
        }

        let loaded = Self {
            root,
            project: Project::new(name, width, height, fps),
            timeline: Timeline::new(),
        };
        loaded.save()?;
        Ok(loaded)
    }

    /// Validate that all referenced source files exist.
    pub fn validate_sources(&self) -> Vec<String> {
        let mut errors = vec![];

        let check_track = |track: &Option<TrackRef>, label: &str, errors: &mut Vec<String>| {
            if let Some(t) = track {
                let path = self.root.join(&t.path);
                if !path.exists() {
                    errors.push(format!("{label} source missing: {}", t.path));
                }
            }
        };

        check_track(&self.project.tracks.screen, "Screen", &mut errors);
        check_track(&self.project.tracks.webcam, "Webcam", &mut errors);
        check_track(&self.project.tracks.mic, "Mic", &mut errors);
        check_track(
            &self.project.tracks.system_audio,
            "System audio",
            &mut errors,
        );

        // Check events file
        let events_path = self.root.join("meta").join("events.jsonl");
        if !events_path.exists() {
            errors.push("Events file missing: meta/events.jsonl".to_string());
        }

        errors
    }
}

/// Errors that can occur when working with projects.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("I/O error at {path}: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Parse error in {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("Invalid project: {message}")]
    ValidationError { message: String },
}

/// Generate a simple UUID v4 without external dependency.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (seed & 0xFFFFFFFF) as u32,
        ((seed >> 32) & 0xFFFF) as u16,
        ((seed >> 48) & 0x0FFF) as u16,
        (((seed >> 60) & 0x3F) | 0x80) as u16 | (((seed >> 66) & 0x3FF) as u16) << 6,
        (seed >> 76) & 0xFFFFFFFFFFFF,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let project = Project::new("Test Recording", 1920, 1080, 60);
        assert_eq!(project.name, "Test Recording");
        assert_eq!(project.recording.capture_width, 1920);
        assert_eq!(project.export.fps, 60);
    }

    #[test]
    fn test_project_serialization() {
        let project = Project::new("Test", 1920, 1080, 30);
        let json = serde_json::to_string_pretty(&project).unwrap();
        let parsed: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Test");
        assert_eq!(parsed.version, "1.0");
    }

    #[test]
    fn test_loaded_project_create_and_load() {
        let dir = std::env::temp_dir().join("grabme_test_project");
        let _ = std::fs::remove_dir_all(&dir);

        let created = LoadedProject::create(&dir, "Integration Test", 1920, 1080, 60).unwrap();
        assert_eq!(created.project.name, "Integration Test");

        let loaded = LoadedProject::load(&dir).unwrap();
        assert_eq!(loaded.project.name, "Integration Test");
        assert_eq!(loaded.timeline.version, "1.0");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_validate_sources_reports_missing() {
        let dir = std::env::temp_dir().join("grabme_test_validate");
        let _ = std::fs::remove_dir_all(&dir);

        let mut loaded = LoadedProject::create(&dir, "Validate Test", 1920, 1080, 60).unwrap();
        loaded.project.tracks.screen = Some(TrackRef {
            path: "sources/screen.mkv".to_string(),
            duration_secs: 60.0,
            codec: "h264".to_string(),
            offset_ns: 0,
        });

        let errors = loaded.validate_sources();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.contains("Screen source missing")));

        std::fs::remove_dir_all(&dir).ok();
    }
}
