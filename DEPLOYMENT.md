# GrabMe Deployment & CI/CD Setup

This document summarizes the stable build and deployment infrastructure for GrabMe.

## Overview

GrabMe now has a complete CI/CD pipeline for:
- Continuous Integration (automated testing)
- Continuous Deployment (crates.io publishing)
- Binary distribution (GitHub Releases)

## What Was Implemented

### 1. Licensing (MIT OR Apache-2.0)

- ✅ `LICENSE-MIT` - MIT License
- ✅ `LICENSE-APACHE` - Apache License 2.0
- ✅ Dual-license model (industry standard for Rust)

### 2. Cargo.toml Manifest Hardening

**Workspace-level** (`Cargo.toml`):
- ✅ Added `homepage`, `documentation`, `readme`, `keywords`, `categories`
- ✅ Converted internal dependencies to `version + path` format for crates.io compatibility

**Per-crate manifests**:
- ✅ All 12 publishable crates inherit workspace metadata (`license.workspace = true`, etc.)
- ✅ Added `readme`, `keywords`, `categories` for discoverability
- ✅ Marked 3 internal/experimental crates with `publish = false`

### 3. GitHub Actions Workflows

#### CI Workflow (`.github/workflows/ci.yml`)

Runs on every push to `main` and all pull requests:

- **Rustfmt**: Code formatting check (`cargo fmt --check`)
- **Clippy**: Lint analysis with warnings as errors
- **Test**: Full workspace test suite
- **Check**: Compilation verification for all targets
- **Package**: Integrity check for all publishable crates

System dependencies installed:
- GStreamer (1.0 + plugins)
- FFmpeg
- X11 development libraries
- libevdev, DBus
- pkg-config

Uses `Swatinem/rust-cache` for faster builds.

#### Release Workflow (`.github/workflows/release.yml`)

Triggered by version tags (`v*`):

1. **Publish to crates.io** in dependency order:
   - grabme-common
   - grabme-platform-core
   - grabme-project-model
   - grabme-processing-core
   - grabme-audio-ai
   - grabme-platform-linux/windows/macos
   - grabme-input-tracker
   - grabme-capture-engine
   - grabme-render-engine
   - grabme-cli

2. **30-second delays** between publishes for crates.io index propagation
3. **continue-on-error** to skip already-published versions
4. **Create GitHub Release** with auto-generated notes

#### Binary Distribution Workflow (`.github/workflows/dist.yml`)

Triggered by version tags (`v*`):

1. Build Linux x86_64 binary (`cargo build --release`)
2. Create tarball with:
   - `grabme` binary
   - `README.md`
   - `LICENSE-MIT`
   - `LICENSE-APACHE`
3. Upload to GitHub Release as asset

### 4. Publishable Crates

**12 crates ready for crates.io:**
1. `grabme-common` - Shared utilities (error, clock, logging)
2. `grabme-platform-core` - Cross-platform contracts
3. `grabme-project-model` - Data model (events, timeline, project)
4. `grabme-processing-core` - Auto-director algorithms
5. `grabme-audio-ai` - Transcription & noise suppression
6. `grabme-render-engine` - Export pipeline (FFmpeg)
7. `grabme-platform-linux` - Linux implementation (Wayland/X11)
8. `grabme-platform-windows` - Windows stub (compile-safe)
9. `grabme-platform-macos` - macOS stub (compile-safe)
10. `grabme-input-tracker` - Input event tracking
11. `grabme-capture-engine` - Capture orchestration
12. `grabme-cli` - Command-line binary

**3 internal crates (not published):**
- `apps/overlay-ui` - Experimental native UI
- `apps/desktop/src-tauri` - Tauri desktop app
- `tools/e2e-cursor-drift` - Test utility

## How to Release

### Quick Steps

1. Update version in `Cargo.toml` (workspace section)
2. Run tests: `cargo test --workspace`
3. Commit: `git commit -am "chore: prepare release v0.x.y"`
4. Tag: `git tag v0.x.y`
5. Push: `git push origin v0.x.y`

GitHub Actions will automatically:
- Run full CI suite
- Publish all crates to crates.io
- Build Linux binary
- Create GitHub Release

See `RELEASING.md` for detailed process.

## Required Secrets

Configure in GitHub repository settings (Settings → Secrets → Actions):

- `CARGO_REGISTRY_TOKEN`: crates.io API token
  - Generate at: https://crates.io/settings/tokens
  - Permission: `publish-update`

## CI Status Badges

Add to README.md:

```markdown
[![CI](https://github.com/grabme/grabme/actions/workflows/ci.yml/badge.svg)](https://github.com/grabme/grabme/actions/workflows/ci.yml)
[![Release](https://github.com/grabme/grabme/actions/workflows/release.yml/badge.svg)](https://github.com/grabme/grabme/actions/workflows/release.yml)
```

## System Dependencies

For local development and CI:

```bash
# Ubuntu/Debian
sudo apt-get install -y \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev \
  gstreamer1.0-plugins-{base,good,bad,ugly} \
  gstreamer1.0-{libav,tools,x,alsa,pulseaudio} \
  libx11-dev libxrandr-dev libxext-dev libxfixes-dev \
  libevdev-dev libdbus-1-dev \
  ffmpeg pkg-config
```

## Package Verification

Test package integrity locally:

```bash
cargo package -p grabme-common --allow-dirty --no-verify
cargo package -p grabme-cli --allow-dirty --no-verify
```

## Crates.io Metadata

Each published crate includes:
- **License**: MIT OR Apache-2.0
- **Repository**: https://github.com/grabme/grabme
- **Homepage**: https://github.com/grabme/grabme
- **Documentation**: https://docs.rs/grabme (auto-generated)
- **Keywords**: screen-recorder, video-editor, linux, screencast
- **Categories**: multimedia::video, command-line-utilities

## Deployment Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Developer pushes v* tag                                 │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│  GitHub Actions Workflows (3 parallel jobs)              │
├─────────────────────────────────────────────────────────┤
│  1. Publish Crates (.github/workflows/release.yml)       │
│     - Publish to crates.io in dependency order           │
│     - Wait for index propagation between crates          │
│     - Create GitHub Release with notes                   │
├─────────────────────────────────────────────────────────┤
│  2. Build Binary (.github/workflows/dist.yml)            │
│     - Build Linux x86_64 binary                          │
│     - Create tarball with licenses                       │
│     - Upload to GitHub Release                           │
├─────────────────────────────────────────────────────────┤
│  3. CI Validation (.github/workflows/ci.yml)             │
│     - Format check, Clippy, Tests, Package integrity     │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│  Release Artifacts                                       │
├─────────────────────────────────────────────────────────┤
│  - 12 crates on crates.io                                │
│  - grabme-VERSION-x86_64-unknown-linux-gnu.tar.gz        │
│  - GitHub Release with auto-generated changelog         │
└─────────────────────────────────────────────────────────┘
```

## Platform Support

**Tier 1 (fully supported):**
- Linux (Wayland + X11 via GStreamer/PipeWire)

**Tier 2 (compile-safe stubs):**
- Windows (platform-windows crate)
- macOS (platform-macos crate)

Future implementations will replace stubs with native backends (Windows Graphics Capture, ScreenCaptureKit).

## Next Steps

1. **Create crates.io account** if you don't have one
2. **Generate API token** at https://crates.io/settings/tokens
3. **Add token to GitHub Secrets** as `CARGO_REGISTRY_TOKEN`
4. **Create first release**: `git tag v0.1.0 && git push origin v0.1.0`
5. **Monitor workflows** at https://github.com/grabme/grabme/actions

## Testing CI Locally

```bash
# Format check
cargo fmt --all -- --check

# Clippy
cargo clippy --workspace --all-targets -- -D warnings

# Tests
cargo test --workspace

# Package integrity
cargo package -p grabme-common --allow-dirty --no-verify
# ... repeat for other crates
```

## Contact & Support

- Issues: https://github.com/grabme/grabme/issues
- Discussions: https://github.com/grabme/grabme/discussions
- Security: See SECURITY.md (if created)

---

**Status**: ✅ Production-ready deployment infrastructure  
**Last Updated**: 2024  
**Maintainer**: GrabMe Contributors
