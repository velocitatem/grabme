// tools/vdesktop-tests/src/main.rs
//! Virtual Desktop Test Suite for GrabMe
//!
//! Runs automated recording tests in a virtual X11 display (Xvfb) with:
//! - Synthetic test patterns for tracking verification
//! - Computer vision-based quality validation
//! - Cursor tracking accuracy measurement
//! - Image quality and solidity checks

use anyhow::{Context, Result};
use clap::Parser;
use grabme_capture_engine::{
    AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig, SessionConfig,
};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

mod synthetic;
mod verify;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Output directory for test results
    #[arg(short, long, default_value = "vdesktop_test_output")]
    output_dir: PathBuf,

    /// Virtual display resolution width
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Virtual display resolution height
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// X11 display number for Xvfb
    #[arg(long, default_value_t = 99)]
    display: u32,

    /// Test duration in seconds
    #[arg(long, default_value_t = 10)]
    duration: u64,

    /// Skip Xvfb setup (use existing display)
    #[arg(long)]
    no_xvfb: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,grabme_input_tracker=debug"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    if args.output_dir.exists() {
        std::fs::remove_dir_all(&args.output_dir)?;
    }
    std::fs::create_dir_all(&args.output_dir)?;

    tracing::info!("Starting Virtual Desktop Test Suite");
    tracing::info!("Resolution: {}x{}", args.width, args.height);
    tracing::info!("Display: :{}", args.display);

    // 1. Setup Xvfb virtual display
    let xvfb_handle = if !args.no_xvfb {
        Some(setup_xvfb(&args).await?)
    } else {
        None
    };

    // 2. Generate and display synthetic test patterns
    generate_test_patterns(&args)?;

    // 3. Run recording with cursor automation
    let project_path = run_recording_test(&args).await?;

    // 4. Verify recorded output with CV
    verify_recording(&args, &project_path).await?;

    // 5. Cleanup
    if let Some(mut handle) = xvfb_handle {
        tracing::info!("Stopping Xvfb...");
        handle.kill().await?;
    }

    tracing::info!("Test suite completed successfully!");
    Ok(())
}

async fn setup_xvfb(args: &Args) -> Result<tokio::process::Child> {
    tracing::info!("Starting Xvfb on display :{}...", args.display);

    let child = tokio::process::Command::new("Xvfb")
        .args([
            &format!(":{}", args.display),
            "-screen",
            "0",
            &format!("{}x{}x24", args.width, args.height),
            "-ac",
            "+extension",
            "GLX",
            "+render",
            "-noreset",
        ])
        .spawn()
        .context("Failed to start Xvfb. Install with: sudo apt install xvfb")?;

    // Wait for Xvfb to be ready
    sleep(Duration::from_secs(2)).await;

    // Verify display is working
    std::env::set_var("DISPLAY", format!(":{}", args.display));

    let status = Command::new("xdpyinfo")
        .env("DISPLAY", format!(":{}", args.display))
        .output()
        .context("Failed to verify Xvfb. Install xdpyinfo with: sudo apt install x11-utils")?;

    if !status.status.success() {
        anyhow::bail!("Xvfb display verification failed");
    }

    tracing::info!("Xvfb started successfully");
    Ok(child)
}

fn generate_test_patterns(args: &Args) -> Result<()> {
    tracing::info!("Generating synthetic test patterns...");

    let patterns_dir = args.output_dir.join("patterns");
    std::fs::create_dir_all(&patterns_dir)?;

    // Pattern 1: Tracking markers (colored circles at known positions)
    let tracking_pattern = synthetic::create_tracking_pattern(args.width, args.height);
    tracking_pattern.save(patterns_dir.join("tracking.png"))?;

    // Pattern 2: Grid with coordinates for spatial verification
    let grid_pattern = synthetic::create_grid_pattern(args.width, args.height);
    grid_pattern.save(patterns_dir.join("grid.png"))?;

    // Pattern 3: High-frequency patterns for image quality check
    let quality_pattern = synthetic::create_quality_pattern(args.width, args.height);
    quality_pattern.save(patterns_dir.join("quality.png"))?;

    tracing::info!("Test patterns generated: {}", patterns_dir.display());
    Ok(())
}

async fn run_recording_test(args: &Args) -> Result<PathBuf> {
    tracing::info!("Starting recording test...");

    // Display test pattern using feh or imagemagick
    let pattern_path = args.output_dir.join("patterns/tracking.png");
    let _display_handle = tokio::process::Command::new("feh")
        .args([
            "--fullscreen",
            "--auto-zoom",
            pattern_path.to_str().unwrap(),
        ])
        .env("DISPLAY", format!(":{}", args.display))
        .spawn()
        .context("Failed to display test pattern. Install with: sudo apt install feh")?;

    sleep(Duration::from_secs(2)).await;

    // Configure capture session
    let config = SessionConfig {
        name: "vdesktop_test".to_string(),
        output_dir: args.output_dir.clone(),
        screen: ScreenCaptureConfig {
            mode: CaptureMode::FullScreen { monitor_index: 0 },
            hide_cursor: false,
        },
        audio: AudioCaptureConfig {
            mic: false,
            system: false,
            app_isolation: None,
            sample_rate: 48000,
        },
        fps: 30,
        pointer_sample_rate_hz: 60,
        ..Default::default()
    };

    let mut session = CaptureSession::new(config);

    // Start recording
    tracing::info!("Starting capture...");
    session.start().await?;
    sleep(Duration::from_secs(2)).await;

    // Start X11 polling fallback for tracking verification
    let tracking_log_path = args.output_dir.join("x11_cursor_tracking.jsonl");
    let display_env = format!(":{}", args.display);
    let poll_handle = start_x11_cursor_polling(
        tracking_log_path.clone(),
        display_env.clone(),
        args.width,
        args.height,
    );

    // Automated cursor movement through test points
    let test_points = synthetic::get_test_points(args.width, args.height);
    for (i, (x, y)) in test_points.iter().enumerate() {
        tracing::info!("Moving cursor to test point {}: ({}, {})", i, x, y);

        Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .env("DISPLAY", &display_env)
            .status()
            .context("Failed to execute xdotool. Install with: sudo apt install xdotool")?;

        sleep(Duration::from_millis(500)).await;
    }

    sleep(Duration::from_secs(args.duration - 5)).await;

    // Stop X11 polling
    poll_handle.abort();

    // Stop recording
    tracing::info!("Stopping capture...");
    let project_path = session.stop().await?;
    tracing::info!("Recording saved to: {}", project_path.display());

    Ok(project_path)
}

fn start_x11_cursor_polling(
    log_path: PathBuf,
    display: String,
    width: u32,
    height: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use std::io::Write;
        
        let mut file = match std::fs::File::create(&log_path) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create tracking log: {}", e);
                return;
            }
        };

        tracing::info!("Started X11 cursor polling fallback -> {}", log_path.display());

        let start_time = std::time::Instant::now();
        let poll_interval = Duration::from_millis(16); // ~60Hz

        loop {
            let output = match Command::new("xdotool")
                .args(["getmouselocation", "--shell"])
                .env("DISPLAY", &display)
                .output()
            {
                Ok(o) => o,
                Err(_) => {
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
            };

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut x = None;
                let mut y = None;

                for line in stdout.lines() {
                    if let Some(val) = line.strip_prefix("X=") {
                        x = val.parse::<u32>().ok();
                    } else if let Some(val) = line.strip_prefix("Y=") {
                        y = val.parse::<u32>().ok();
                    }
                }

                if let (Some(x_pos), Some(y_pos)) = (x, y) {
                    let timestamp_ns = start_time.elapsed().as_nanos() as u64;
                    let norm_x = x_pos as f64 / width as f64;
                    let norm_y = y_pos as f64 / height as f64;

                    let entry = serde_json::json!({
                        "timestamp_ns": timestamp_ns,
                        "x": norm_x,
                        "y": norm_y,
                        "x_px": x_pos,
                        "y_px": y_pos,
                    });

                    if let Ok(json) = serde_json::to_string(&entry) {
                        let _ = writeln!(file, "{}", json);
                    }
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    })
}

async fn verify_recording(args: &Args, project_path: &std::path::Path) -> Result<()> {
    tracing::info!("Verifying recording quality...");

    // 1. Verify event tracking accuracy
    let tracking_metrics = verify::check_tracking_accuracy(project_path, args.width, args.height)?;

    // 2. Verify image quality (frame extraction + analysis)
    let image_metrics = verify::check_image_quality(project_path)?;

    // 3. Generate test report
    verify::generate_report(
        args.output_dir.join("test_report.json"),
        tracking_metrics,
        image_metrics,
    )?;

    tracing::info!("Verification complete. See test_report.json for details.");
    Ok(())
}
