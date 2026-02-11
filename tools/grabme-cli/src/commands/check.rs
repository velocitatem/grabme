//! Check system capabilities.

use grabme_platform_linux::{detect_display_server, DisplayServer};

pub fn run() -> anyhow::Result<()> {
    println!("GrabMe System Check");
    println!("{}", "=".repeat(50));

    // Display server
    let ds = detect_display_server();
    match ds {
        DisplayServer::Wayland => println!("[OK] Display server: Wayland"),
        DisplayServer::X11 => println!("[OK] Display server: X11"),
        _ => println!("[WARN] Display server: Unknown"),
    }

    // Check monitors
    let monitors = grabme_platform_linux::detect_monitors()?;
    println!("[OK] Monitors detected: {}", monitors.len());
    for m in &monitors {
        println!(
            "     {} {}x{} @ {}Hz (scale: {}x) {}",
            m.name,
            m.width,
            m.height,
            m.refresh_rate_hz,
            m.scale_factor,
            if m.primary { "(primary)" } else { "" }
        );
    }

    // Check permissions
    let capabilities = grabme_platform_linux::permissions::check_capabilities();
    println!();
    grabme_platform_linux::permissions::print_capability_report(&capabilities);

    let all_required_ok = capabilities
        .iter()
        .filter(|c| c.required)
        .all(|c| c.available);

    println!();
    if all_required_ok {
        println!("All required capabilities are available. GrabMe is ready.");
    } else {
        println!("Some required capabilities are missing. See above for fixes.");
    }

    Ok(())
}
