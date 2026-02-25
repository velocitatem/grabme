use anyhow::{Context, Result};
use clap::Parser;
use grabme_capture_engine::{
    AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig, SessionConfig,
};
use grabme_project_model::event::InputEvent;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for the recording
    #[arg(short, long, default_value = "drift_test_output")]
    output_dir: PathBuf,

    /// Width of the X11 screen
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Height of the X11 screen
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// Duration to wait at each corner (seconds)
    #[arg(long, default_value_t = 1.0)]
    dwell_time: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configure logging: Info by default, override with RUST_LOG
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,grabme_input_tracker=debug,grabme_capture_engine=info")
    });

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    // Ensure output directory exists
    if args.output_dir.exists() {
        std::fs::remove_dir_all(&args.output_dir)?;
    }
    std::fs::create_dir_all(&args.output_dir)?;

    tracing::info!("Starting E2E Cursor Drift Test");
    tracing::info!("Screen: {}x{}", args.width, args.height);
    tracing::info!("Output: {:?}", args.output_dir);

    // 1. Configure Session
    let config = SessionConfig {
        name: "drift_test".to_string(),
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

    // 2. Start Recording
    tracing::info!("Starting capture session...");
    session.start().await.context("Failed to start session")?;

    // Allow some time for startup and stabilization
    sleep(Duration::from_secs(2)).await;

    // 3. Simulate Mouse Movement
    tracing::info!("Simulating mouse movement...");

    let corners = vec![
        (100, 100),
        (args.width - 100, 100),
        (args.width - 100, args.height - 100),
        (100, args.height - 100),
        (100, 100),
    ];

    let dwell = Duration::from_secs_f64(args.dwell_time);

    // Move to start first
    let _ = Command::new("xdotool")
        .args([
            "mousemove",
            &corners[0].0.to_string(),
            &corners[0].1.to_string(),
        ])
        .status();
    sleep(Duration::from_millis(500)).await;

    for (i, (x, y)) in corners.iter().enumerate() {
        tracing::info!("Moving to corner {}: ({}, {})", i, x, y);

        let status = Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .status()
            .context("Failed to execute xdotool")?;

        if !status.success() {
            tracing::warn!("Warning: xdotool exited with error");
        }

        sleep(dwell).await;
    }

    // 4. Stop Recording
    tracing::info!("Stopping capture session...");
    let project_path = session.stop().await.context("Failed to stop session")?;
    tracing::info!("Project saved to: {:?}", project_path);

    // 5. Analyze Results
    analyze_results(
        &project_path,
        &corners,
        args.width,
        args.height,
        args.dwell_time,
    )
    .await?;

    Ok(())
}

async fn analyze_results(
    project_path: &std::path::Path,
    expected_corners: &[(u32, u32)],
    width: u32,
    height: u32,
    dwell_time: f64,
) -> Result<()> {
    let events_path = project_path.join("meta").join("events.jsonl");
    tracing::info!("Analyzing events from: {:?}", events_path);

    if !events_path.exists() {
        anyhow::bail!("Events file not found: {:?}", events_path);
    }
    let metadata = std::fs::metadata(&events_path)?;
    if metadata.len() == 0 {
        anyhow::bail!("Events file is empty! (size=0)");
    }

    let content = std::fs::read_to_string(events_path).context("Failed to read events.jsonl")?;

    // Parse events, skipping comments
    let events: Vec<InputEvent> = content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| serde_json::from_str::<InputEvent>(l).ok())
        .collect();

    if events.is_empty() {
        anyhow::bail!("No valid events recorded in file!");
    }

    tracing::info!("Recorded {} events.", events.len());

    let mut stable_points = Vec::new();
    let mut current_stable_start = 0;
    let mut current_pos = (0.0, 0.0);
    // Stability threshold in normalized coordinates (0.005 is roughly 6px on 1280 screen)
    let stability_threshold = 0.005;
    let min_stable_duration_ns = (dwell_time * 0.4 * 1_000_000_000.0) as u64;

    let dist =
        |(x1, y1): (f64, f64), (x2, y2): (f64, f64)| ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt();

    if let Some(first) = events.first().and_then(|e| e.pointer_position()) {
        current_pos = first;
    }

    for i in 1..events.len() {
        let e = &events[i];
        if let Some(pos) = e.pointer_position() {
            if dist(pos, current_pos) > stability_threshold {
                // Movement detected
                // The duration of the PREVIOUS stable segment ends at this event's timestamp
                let duration = e.timestamp_ns - events[current_stable_start].timestamp_ns;
                if duration > min_stable_duration_ns {
                    // It was stable for a while
                    stable_points.push((current_pos, duration));
                }
                current_stable_start = i;
                current_pos = pos;
            }
        }
    }

    // Handle the final segment.
    // We assume the last recorded position is a stable point since we dwell at the end.
    stable_points.push((current_pos, min_stable_duration_ns + 1));

    tracing::info!("Detected {} stable positions:", stable_points.len());

    let mut match_count = 0;
    let mut max_drift_px: f64 = 0.0;
    let mut total_drift_px = 0.0;

    for (i, (expected_x, expected_y)) in expected_corners.iter().enumerate() {
        let norm_expected_x = *expected_x as f64 / width as f64;
        let norm_expected_y = *expected_y as f64 / height as f64;

        let mut best_match = None;
        let mut min_d = f64::MAX;

        for (recorded_pos, _) in &stable_points {
            let d = dist((norm_expected_x, norm_expected_y), *recorded_pos);
            if d < min_d {
                min_d = d;
                best_match = Some(*recorded_pos);
            }
        }

        if let Some((rec_x, rec_y)) = best_match {
            let pixel_drift = dist(
                (rec_x * width as f64, rec_y * height as f64),
                (*expected_x as f64, *expected_y as f64),
            );

            tracing::info!(
                "Corner {}: Expected ({}, {}), Recorded ({:.1}, {:.1}), Drift: {:.2}px",
                i,
                expected_x,
                expected_y,
                rec_x * width as f64,
                rec_y * height as f64,
                pixel_drift
            );

            if pixel_drift < 30.0 {
                // Tolerance 30px
                match_count += 1;
            }
            max_drift_px = max_drift_px.max(pixel_drift);
            total_drift_px += pixel_drift;
        }
    }

    println!("---------------------------------------------------");
    println!("Summary:");
    println!(
        "Corners Matched: {}/{}",
        match_count,
        expected_corners.len()
    );
    println!("Max Drift: {:.2} px", max_drift_px);
    println!(
        "Avg Drift: {:.2} px",
        total_drift_px / expected_corners.len() as f64
    );

    if match_count >= expected_corners.len() {
        println!("SUCCESS: Tracking is accurate.");
    } else {
        println!("FAILURE: Significant drift or missed corners detected.");
        if match_count < 3 {
            anyhow::bail!("Too few corners matched. Setup might be broken.");
        }
    }

    Ok(())
}
