# GrabMe Native Overlay UI

GrabMe includes a native Rust floating overlay (`grabme-overlay`) focused on recording orchestration, then post-record automation.

## Run

```bash
cargo run -p grabme-overlay-ui --bin grabme-overlay
```

## What it includes

- Always-on-top floating overlay window (no Tauri runtime)
- Record/Stop capture controls directly in the overlay
- Project naming and output directory controls
- Recording options (FPS, mic, system audio, webcam)
- Live recording timer and saved project path confirmation
- Post-record stage with `Auto-Direct` and `Render` actions
- Built-in render progress updates and output path display

Editing controls remain a later roadmap phase; this overlay now handles capture + immediate post-process orchestration.
