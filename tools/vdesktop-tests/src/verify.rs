//! Recording verification using computer vision

use anyhow::{Context, Result};
use grabme_project_model::event::InputEvent;
use image::{ImageBuffer, Rgb};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestReport {
    pub tracking_accuracy: TrackingMetrics,
    pub image_quality: ImageQualityMetrics,
    pub overall_status: TestStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingMetrics {
    pub total_events: usize,
    pub expected_points: usize,
    pub matched_points: usize,
    pub avg_drift_px: f64,
    pub max_drift_px: f64,
    pub accuracy_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageQualityMetrics {
    pub frames_analyzed: usize,
    pub avg_brightness: f64,
    pub min_brightness: f64,
    pub max_brightness: f64,
    pub has_corruption: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TestStatus {
    Pass,
    Fail,
    Warning,
}

pub fn check_tracking_accuracy(
    project_path: &Path,
    width: u32,
    height: u32,
) -> Result<TrackingMetrics> {
    tracing::info!("Analyzing tracking accuracy...");

    let events_path = project_path.join("meta").join("events.jsonl");
    let content = std::fs::read_to_string(&events_path).context("Failed to read events.jsonl")?;

    let events: Vec<InputEvent> = content
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    tracing::info!("Parsed {} events", events.len());

    // Get expected test points
    let expected_points = crate::synthetic::get_test_points(width, height);

    // Find stable positions from events (dwells)
    let stable_positions = find_stable_positions(&events, width, height);

    // Match stable positions to expected points
    let (matched, avg_drift, max_drift) =
        match_positions(&stable_positions, &expected_points, width, height);

    let accuracy_percent = (matched as f64 / expected_points.len() as f64) * 100.0;

    let metrics = TrackingMetrics {
        total_events: events.len(),
        expected_points: expected_points.len(),
        matched_points: matched,
        avg_drift_px: avg_drift,
        max_drift_px: max_drift,
        accuracy_percent,
    };

    tracing::info!(
        "Tracking accuracy: {:.1}% ({}/{} points matched, avg drift: {:.1}px)",
        metrics.accuracy_percent,
        metrics.matched_points,
        metrics.expected_points,
        metrics.avg_drift_px
    );

    Ok(metrics)
}

fn find_stable_positions(events: &[InputEvent], width: u32, height: u32) -> Vec<(f64, f64)> {
    let mut stable_positions = Vec::new();
    let stability_threshold = 0.01; // 1% of screen
    let min_stable_duration_ms = 300;

    let mut current_pos: Option<(f64, f64)> = None;
    let mut stable_start_ts = 0u64;

    for event in events {
        if let Some((x, y)) = event.pointer_position() {
            if let Some((prev_x, prev_y)) = current_pos {
                let dist = ((x - prev_x).powi(2) + (y - prev_y).powi(2)).sqrt();

                if dist < stability_threshold {
                    // Still stable, check duration
                    let duration_ms = (event.timestamp_ns - stable_start_ts) / 1_000_000;
                    if duration_ms >= min_stable_duration_ms
                        && !stable_positions.contains(&(prev_x, prev_y))
                    {
                        stable_positions.push((prev_x, prev_y));
                    }
                } else {
                    // Movement detected, reset
                    current_pos = Some((x, y));
                    stable_start_ts = event.timestamp_ns;
                }
            } else {
                current_pos = Some((x, y));
                stable_start_ts = event.timestamp_ns;
            }
        }
    }

    stable_positions
}

fn match_positions(
    recorded: &[(f64, f64)],
    expected: &[(u32, u32)],
    width: u32,
    height: u32,
) -> (usize, f64, f64) {
    let mut matched = 0;
    let mut total_drift = 0.0;
    let mut max_drift = 0.0;

    for (exp_x, exp_y) in expected {
        let norm_exp_x = *exp_x as f64 / width as f64;
        let norm_exp_y = *exp_y as f64 / height as f64;

        // Find closest recorded position
        let mut min_dist = f64::MAX;
        let mut best_match = None;

        for (rec_x, rec_y) in recorded {
            let dist = ((rec_x - norm_exp_x).powi(2) + (rec_y - norm_exp_y).powi(2)).sqrt();
            if dist < min_dist {
                min_dist = dist;
                best_match = Some((*rec_x, *rec_y));
            }
        }

        if let Some((rec_x, rec_y)) = best_match {
            // Convert to pixels
            let pixel_drift = (((rec_x - norm_exp_x) * width as f64).powi(2)
                + ((rec_y - norm_exp_y) * height as f64).powi(2))
            .sqrt();

            if pixel_drift < 50.0 {
                // Tolerance: 50px
                matched += 1;
            }

            total_drift += pixel_drift;
            max_drift = max_drift.max(pixel_drift);
        }
    }

    let avg_drift = if matched > 0 {
        total_drift / matched as f64
    } else {
        0.0
    };

    (matched, avg_drift, max_drift)
}

pub fn check_image_quality(project_path: &Path) -> Result<ImageQualityMetrics> {
    tracing::info!("Analyzing image quality...");

    // Check if screen source exists
    let screen_path = project_path.join("sources").join("screen.mkv");
    if !screen_path.exists() {
        anyhow::bail!("Screen recording not found at: {}", screen_path.display());
    }

    // Extract a few frames using ffmpeg
    let frames_dir = project_path.join("extracted_frames");
    std::fs::create_dir_all(&frames_dir)?;

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-i",
            screen_path.to_str().unwrap(),
            "-vf",
            "select='not(mod(n\\,30))'",
            "-frames:v",
            "10",
            "-vsync",
            "vfr",
            "-y",
            &format!("{}/frame_%03d.png", frames_dir.display()),
        ])
        .output()
        .context("Failed to extract frames. Ensure ffmpeg is installed.")?;

    if !status.status.success() {
        anyhow::bail!(
            "Frame extraction failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    // Analyze extracted frames
    let mut brightness_values = Vec::new();
    let mut has_corruption = false;

    for entry in std::fs::read_dir(&frames_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) == Some("png") {
            if let Ok(img) = image::open(entry.path()) {
                let rgb_img = img.to_rgb8();
                let brightness = calculate_brightness(&rgb_img);
                brightness_values.push(brightness);

                // Check for corruption (all black or all white)
                if brightness < 5.0 || brightness > 250.0 {
                    has_corruption = true;
                    tracing::warn!(
                        "Potential corruption in {}: brightness {}",
                        entry.path().display(),
                        brightness
                    );
                }
            }
        }
    }

    let metrics = ImageQualityMetrics {
        frames_analyzed: brightness_values.len(),
        avg_brightness: brightness_values.iter().sum::<f64>() / brightness_values.len() as f64,
        min_brightness: brightness_values
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min),
        max_brightness: brightness_values
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max),
        has_corruption,
    };

    tracing::info!(
        "Image quality: {} frames analyzed, avg brightness: {:.1}, corruption: {}",
        metrics.frames_analyzed,
        metrics.avg_brightness,
        metrics.has_corruption
    );

    Ok(metrics)
}

fn calculate_brightness(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> f64 {
    let mut total = 0u64;
    for pixel in img.pixels() {
        let r = pixel[0] as u64;
        let g = pixel[1] as u64;
        let b = pixel[2] as u64;
        total += (r + g + b) / 3;
    }
    total as f64 / img.pixels().count() as f64
}

pub fn generate_report(report_path: std::path::PathBuf, project_path: &Path) -> Result<()> {
    tracing::info!("Generating test report...");

    // Get metrics from other checks
    let tracking_accuracy = check_tracking_accuracy(project_path, 1920, 1080)?;
    let image_quality = check_image_quality(project_path)?;

    // Determine overall status
    let overall_status =
        if tracking_accuracy.accuracy_percent >= 90.0 && !image_quality.has_corruption {
            TestStatus::Pass
        } else if tracking_accuracy.accuracy_percent >= 70.0 {
            TestStatus::Warning
        } else {
            TestStatus::Fail
        };

    let report = TestReport {
        tracking_accuracy,
        image_quality,
        overall_status,
    };

    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&report_path, json)?;

    tracing::info!("Test report written to: {}", report_path.display());
    tracing::info!("Overall status: {:?}", report.overall_status);

    if report.overall_status != TestStatus::Pass {
        anyhow::bail!("Test suite failed with status: {:?}", report.overall_status);
    }

    Ok(())
}
