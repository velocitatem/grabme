# GrabMe

GrabMe is a Linux-first screen recorder/editor that captures raw media plus
input metadata, then exports polished output through a timeline-driven render
pipeline.

## What is implemented

- Deterministic monitor targeting with strict monitor-index validation.
- X11 capture-region math fixed for non-zero monitor origins.
- Track sync improvements with start-group pipeline startup and duration-based
  offset correction.
- Export-time alignment for webcam, mic, and system audio (positive and
  negative offsets).
- Dual-source audio mixing (`mic + system`) via FFmpeg `amix`.
- Timeline rendering restored as default; full-screen override is now opt-in via
  `GRABME_FORCE_FULL_SCREEN_RENDER=1`.
- Monitor pre-crop fallback for accidental full-virtual-desktop recordings.
- Canvas style controls in export config (background, radius, shadow, padding).
- Optional cursor motion-trail effect (disabled by default).
- Per-export sync diagnostics artifact (`*.sync-report.json`).
- Platform abstraction split into:
  - `grabme-platform-core` (shared contracts)
  - `grabme-platform-linux` (active implementation)
  - `grabme-platform-windows` / `grabme-platform-macos` (compile-safe scaffolds)

## Quick start

```bash
cargo run -p grabme-cli -- record --name recording --monitor 0 --webcam
```

Stop with `Ctrl+C`, then export:

```bash
cargo run -p grabme-cli -- export ./recording --format mp4-h264 --width 1920 --height 1080
```

## Validation

```bash
cargo check --workspace
cargo test -p grabme-project-model -p grabme-input-tracker -p grabme-capture-engine -p grabme-render-engine
```

## Documentation

- `docs/architecture.md`
- `docs/linux-capture.md`
- `docs/export-pipeline.md`
- `docs/data-contracts.md`
- `docs/roadmap.md`
