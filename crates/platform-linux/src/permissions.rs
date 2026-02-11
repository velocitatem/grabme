//! Permission detection and guidance for Linux.
//!
//! GrabMe needs various system permissions depending on
//! the input tracking backend and capture method used.

/// A system capability that GrabMe may need.
#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub available: bool,
    pub required: bool,
    pub fix_instructions: Option<String>,
}

/// Check all capabilities and report status.
pub fn check_capabilities() -> Vec<Capability> {
    vec![
        check_portal_access(),
        check_pipewire_access(),
        check_webcam_access(),
        check_input_device_access(),
        check_audio_access(),
    ]
}

/// Check if XDG Desktop Portal is accessible.
fn check_portal_access() -> Capability {
    let available = std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok();

    Capability {
        name: "Screen Capture Portal".to_string(),
        description: "XDG Desktop Portal for screen recording consent".to_string(),
        available,
        required: true,
        fix_instructions: if !available {
            Some(
                "Ensure you are running a graphical desktop session (GNOME, KDE, etc.)".to_string(),
            )
        } else {
            None
        },
    }
}

/// Check PipeWire availability.
fn check_pipewire_access() -> Capability {
    let available = std::path::Path::new("/run/user").exists(); // Simplified check

    Capability {
        name: "PipeWire".to_string(),
        description: "PipeWire multimedia server for audio/video streams".to_string(),
        available,
        required: true,
        fix_instructions: if !available {
            Some("Install PipeWire: sudo apt install pipewire pipewire-pulse".to_string())
        } else {
            None
        },
    }
}

/// Check if the user can access input devices (for evdev backend).
fn check_input_device_access() -> Capability {
    let input_dir = std::path::Path::new("/dev/input");
    let available = input_dir.exists();

    // Check if user is in the 'input' group
    let in_input_group = std::process::Command::new("groups")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("input"))
        .unwrap_or(false);

    Capability {
        name: "Input Device Access".to_string(),
        description: "Direct input device access for mouse tracking (evdev)".to_string(),
        available: available && in_input_group,
        required: false, // fallback to focused-window tracking
        fix_instructions: if !in_input_group {
            Some(
                "Add user to input group: sudo usermod -aG input $USER (logout required)"
                    .to_string(),
            )
        } else {
            None
        },
    }
}

/// Check audio capture capability.
fn check_audio_access() -> Capability {
    Capability {
        name: "Audio Capture".to_string(),
        description: "PulseAudio/PipeWire audio capture".to_string(),
        available: true, // Usually available on desktop Linux
        required: false,
        fix_instructions: None,
    }
}

/// Check if a webcam device is available.
fn check_webcam_access() -> Capability {
    let has_webcam = (0..16)
        .map(|idx| format!("/dev/video{idx}"))
        .any(|path| std::path::Path::new(&path).exists());

    Capability {
        name: "Webcam Device".to_string(),
        description: "Video4Linux webcam source for optional picture-in-picture capture"
            .to_string(),
        available: has_webcam,
        required: false,
        fix_instructions: if has_webcam {
            None
        } else {
            Some(
                "Connect a webcam and verify /dev/video* exists (v4l2-ctl --list-devices)"
                    .to_string(),
            )
        },
    }
}

/// Print a user-friendly capability report.
pub fn print_capability_report(capabilities: &[Capability]) {
    println!("GrabMe System Capabilities:");
    println!("{}", "-".repeat(60));

    for cap in capabilities {
        let status = if cap.available {
            "[OK]"
        } else if cap.required {
            "[MISSING - REQUIRED]"
        } else {
            "[MISSING - OPTIONAL]"
        };

        println!("  {} {}: {}", status, cap.name, cap.description);

        if let Some(ref fix) = cap.fix_instructions {
            println!("    Fix: {fix}");
        }
    }
}
