//! Start a recording session.

use std::path::PathBuf;

use grabme_capture_engine::{
    AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig, SessionConfig,
};

pub async fn run(
    name: String,
    output: PathBuf,
    fps: u32,
    mic: bool,
    system_audio: bool,
    webcam: bool,
) -> anyhow::Result<()> {
    println!("Starting recording session: {name}");
    println!("  Output: {}", output.display());
    println!("  FPS: {fps}");
    println!("  Mic: {mic}");
    println!("  System audio: {system_audio}");
    println!("  Webcam: {webcam}");
    println!();

    let config = SessionConfig {
        name,
        output_dir: output,
        screen: ScreenCaptureConfig {
            mode: CaptureMode::FullScreen { monitor_index: 0 },
            hide_cursor: true,
        },
        audio: AudioCaptureConfig {
            mic,
            system: system_audio,
            app_isolation: None,
            sample_rate: 48000,
        },
        webcam,
        fps,
        pointer_sample_rate_hz: 60,
    };

    let mut session = CaptureSession::new(config);

    println!("Press Ctrl+C to stop recording...");
    println!();

    session.start().await?;

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    println!();
    let project_path = session.stop().await?;
    println!("Recording saved to: {}", project_path.display());

    Ok(())
}
