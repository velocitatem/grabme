//! Start a recording session.

use std::path::PathBuf;

use grabme_capture_engine::{
    list_monitors, AudioCaptureConfig, CaptureMode, CaptureSession, ScreenCaptureConfig,
    SessionConfig,
};

pub async fn run(
    name: String,
    output: PathBuf,
    fps: u32,
    monitor: usize,
    mic: bool,
    system_audio: bool,
    webcam: bool,
    list_only: bool,
) -> anyhow::Result<()> {
    // Detect monitors first so we can print the list and validate the index.
    let monitors = list_monitors().unwrap_or_default();

    if list_only {
        println!("Available monitors:");
        if monitors.is_empty() {
            println!("  (none detected)");
        }
        for (i, m) in monitors.iter().enumerate() {
            println!(
                "  [{}] {} — {}x{} at ({},{}){}",
                i,
                m.name,
                m.width,
                m.height,
                m.x,
                m.y,
                if m.primary { " [primary]" } else { "" }
            );
        }
        return Ok(());
    }

    // Print monitor list so the user can see which index maps to which screen.
    println!("Available monitors:");
    if monitors.is_empty() {
        println!("  (none detected — will use default)");
    }
    for (i, m) in monitors.iter().enumerate() {
        let selected = if i == monitor {
            " <-- recording this"
        } else {
            ""
        };
        println!(
            "  [{}] {} — {}x{} at ({},{}){}{selected}",
            i,
            m.name,
            m.width,
            m.height,
            m.x,
            m.y,
            if m.primary { " [primary]" } else { "" },
        );
    }

    // Validate monitor index before starting.
    if !monitors.is_empty() && monitor >= monitors.len() {
        anyhow::bail!(
            "Monitor index {} is out of range. Available monitors: 0..{}. \
             Use `grabme record --list-monitors` to see all monitors.",
            monitor,
            monitors.len() - 1
        );
    }

    println!();
    println!("Starting recording session: {name}");
    println!("  Output: {}", output.display());
    println!("  FPS: {fps}");
    println!("  Monitor: {monitor}");
    println!("  Mic: {mic}");
    println!("  System audio: {system_audio}");
    println!("  Webcam: {webcam}");
    println!();

    let config = SessionConfig {
        name,
        output_dir: output,
        screen: ScreenCaptureConfig {
            mode: CaptureMode::FullScreen {
                monitor_index: monitor,
            },
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
