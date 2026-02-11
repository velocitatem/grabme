//! GrabMe CLI — Command-line interface for recording, analysis, and export.
//!
//! Usage:
//!   grabme record [OPTIONS]    Start a new recording
//!   grabme validate <PATH>     Validate a project bundle
//!   grabme analyze <PATH>      Run Auto-Director on a project
//!   grabme export <PATH>       Export a project to video
//!   grabme info <PATH>         Show project information
//!   grabme check               Check system capabilities

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "grabme",
    about = "Professional screen recording with intelligent editing",
    version,
    author
)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new recording session
    Record {
        /// Project name
        #[arg(short, long, default_value = "recording")]
        name: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,

        /// Target FPS
        #[arg(long, default_value = "60")]
        fps: u32,

        /// Zero-based monitor index to record
        #[arg(long, default_value = "0")]
        monitor: usize,

        /// Disable microphone capture
        #[arg(long)]
        no_mic: bool,

        /// Disable system audio capture
        #[arg(long)]
        no_system_audio: bool,

        /// Enable webcam capture
        #[arg(long)]
        webcam: bool,
    },

    /// Validate a project bundle
    Validate {
        /// Path to the project directory
        path: PathBuf,
    },

    /// Run Auto-Director analysis on a project
    Analyze {
        /// Path to the project directory
        path: PathBuf,

        /// Chunk duration for analysis (seconds)
        #[arg(long, default_value = "2.0")]
        chunk_secs: f64,

        /// Enable vertical/social mode (9:16)
        #[arg(long)]
        vertical: bool,

        /// Hover zoom viewport size (lower = tighter zoom)
        #[arg(long, default_value = "0.4")]
        hover_zoom: f64,

        /// Scan zoom viewport size (higher = wider framing)
        #[arg(long, default_value = "0.85")]
        scan_zoom: f64,

        /// Dwell radius threshold (normalized)
        #[arg(long, default_value = "0.15")]
        dwell_radius: f64,

        /// Dwell velocity threshold (normalized units/sec)
        #[arg(long, default_value = "0.18")]
        dwell_velocity: f64,

        /// Smoothing window for generated camera keyframes
        #[arg(long, default_value = "3")]
        smooth_window: usize,

        /// Cursor smoothing algorithm for export: ema|bezier|kalman|none
        #[arg(long, default_value = "ema")]
        cursor_smoothing: String,

        /// Cursor smoothing strength [0.0, 1.0]
        #[arg(long, default_value = "0.3")]
        cursor_smoothing_factor: f64,

        /// Number of monitors packed in the capture region
        #[arg(long, default_value = "1")]
        monitor_count: usize,

        /// Zero-based focused monitor index
        #[arg(long, default_value = "0")]
        focused_monitor: usize,
    },

    /// Export a project to video
    Export {
        /// Path to the project directory
        path: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format
        #[arg(long, default_value = "mp4-h264")]
        format: String,

        /// Output width
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Output height
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// Show project information
    Info {
        /// Path to the project directory
        path: PathBuf,
    },

    /// Check system capabilities
    Check,

    /// Create a new empty project
    Init {
        /// Project name
        name: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,

        /// Capture width
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Capture height
        #[arg(long, default_value = "1080")]
        height: u32,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    grabme_common::logging::init_logging(&grabme_common::config::LoggingConfig {
        level: log_level.to_string(),
        json: false,
        file: None,
    });

    match cli.command {
        Commands::Record {
            name,
            output,
            fps,
            monitor,
            no_mic,
            no_system_audio,
            webcam,
        } => {
            commands::record::run(
                name,
                output,
                fps,
                monitor,
                !no_mic,
                !no_system_audio,
                webcam,
            )
            .await
        }
        Commands::Validate { path } => commands::validate::run(path),
        Commands::Analyze {
            path,
            chunk_secs,
            vertical,
            hover_zoom,
            scan_zoom,
            dwell_radius,
            dwell_velocity,
            smooth_window,
            cursor_smoothing,
            cursor_smoothing_factor,
            monitor_count,
            focused_monitor,
        } => commands::analyze::run(
            path,
            chunk_secs,
            vertical,
            hover_zoom,
            scan_zoom,
            dwell_radius,
            dwell_velocity,
            smooth_window,
            cursor_smoothing,
            cursor_smoothing_factor,
            monitor_count,
            focused_monitor,
        ),
        Commands::Export {
            path,
            output,
            format,
            width,
            height,
        } => commands::export::run(path, output, format, width, height).await,
        Commands::Info { path } => commands::info::run(path),
        Commands::Check => commands::check::run(),
        Commands::Init {
            name,
            output,
            width,
            height,
        } => commands::init::run(name, output, width, height),
    }
}
