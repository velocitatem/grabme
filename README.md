# GrabMe

GrabMe is a cross-platform screen recorder/editor architecture that captures raw
media plus input metadata, then exports polished output through a
timeline-driven render pipeline.

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
  - `grabme-platform-windows` / `grabme-platform-macos` (in progress)

## Install

### Linux (release binary)

```bash
curl -fsSL https://raw.githubusercontent.com/velocitatem/grabme/main/scripts/install.sh | bash
```

Installs `grabme` to `~/.local/bin` by default.

Optional environment variables:

- `GRABME_VERSION=v0.1.0` to pin a version
- `GRABME_INSTALL_DIR=/usr/local/bin` to change install location
- `GRABME_REPO=owner/repo` to install from a fork

### Rust users (all platforms)

```bash
cargo install grabme-cli
```

### Manual download

Download a release archive from:

- `https://github.com/velocitatem/grabme/releases`

Each archive ships with a `.sha256` checksum file.

## Quick start

```bash
cargo run -p grabme-cli -- record --name recording --monitor 0 --webcam
```

List monitor indices first:

```bash
cargo run -p grabme-cli -- record --list-monitors
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

## Wiki

Run a local wiki server:

```bash
cargo install mdbook
mdbook serve docs
```

Primary docs are organized in `docs/src/SUMMARY.md` and rendered from the
existing documentation files.

## First-run checklist

```bash
grabme check
grabme record --list-monitors
```

Then start a recording with an explicit monitor index:

```bash
grabme record --monitor 1
```
