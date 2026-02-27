# Release Process

This document describes how to release new versions of GrabMe.

## Versioning

GrabMe follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR**: Breaking changes to public API
- **MINOR**: New features, backward-compatible
- **PATCH**: Bug fixes, backward-compatible

Current version: `0.1.0` (pre-1.0 development)

During `0.x` releases:
- Minor bumps can include breaking changes
- Patch bumps are for bugfixes only

## Publishable Crates

The following crates are published to crates.io:

1. `grabme-common` - Shared utilities
2. `grabme-platform-core` - Platform contracts
3. `grabme-project-model` - Data model
4. `grabme-processing-core` - Auto-director
5. `grabme-audio-ai` - Audio intelligence
6. `grabme-render-engine` - Export pipeline
7. `grabme-platform-linux` - Linux implementation
8. `grabme-platform-windows` - Windows stub
9. `grabme-platform-macos` - macOS stub
10. `grabme-input-tracker` - Input tracking
11. `grabme-capture-engine` - Capture orchestration
12. `grabme-cli` - CLI binary

**Non-publishable** (internal/experimental):
- `apps/overlay-ui`
- `apps/desktop/src-tauri`
- `tools/e2e-cursor-drift`

## Release Checklist

### Pre-release

1. Update version in `Cargo.toml` workspace section
2. Update CHANGELOG.md (if exists) or add release notes
3. Ensure all tests pass: `cargo test --workspace`
4. Run clippy: `cargo clippy --workspace --all-targets -- -D warnings`
5. Format code: `cargo fmt --all`
6. Update README.md if needed
7. Commit changes: `git commit -am "chore: prepare release v0.x.y"`
8. Create PR and get approval

### Release

1. Merge release PR to main
2. Create and push git tag:
   ```bash
   git tag v0.x.y
   git push origin v0.x.y
   ```
3. GitHub Actions will automatically:
   - Run CI tests
   - Publish crates to crates.io in dependency order
   - Build Linux binary
   - Create GitHub Release with assets

### Post-release

1. Verify crates published successfully on crates.io
2. Verify GitHub Release created with binaries
3. Test installation: `cargo install grabme-cli`
4. Announce release (if applicable)

## GitHub Actions Workflows

### CI (`ci.yml`)

Runs on every push to main and all PRs:
- **Rustfmt**: Code formatting check
- **Clippy**: Lint checks
- **Test**: Run test suite
- **Check**: Compilation check
- **Package**: Verify packaging for all publishable crates

### Release (`release.yml`)

Triggered by version tags (`v*`):
- Publishes all crates to crates.io in dependency order
- Waits 30s between publishes for index update
- Uses `continue-on-error` to skip already-published versions
- Creates GitHub Release with auto-generated notes

### Binary Distribution (`dist.yml`)

Triggered by version tags (`v*`):
- Builds Linux release binaries (target matrix)
- Runs a packaged binary smoke test (`grabme --help`)
- Creates tarball with binary + licenses
- Generates SHA256 checksum files for each archive
- Uploads archives and checksums to GitHub Release

### Installer Script

`scripts/install.sh` provides one-command install from GitHub Releases:

```bash
curl -fsSL https://raw.githubusercontent.com/velocitatem/grabme/main/scripts/install.sh | bash
```

## Required Secrets

Configure in GitHub repository settings:

- `CARGO_REGISTRY_TOKEN`: crates.io API token
  - Generate at https://crates.io/settings/tokens
  - Needs publish permission
  - Store in GitHub repository secrets

## Manual Publishing (Emergency)

If automated publishing fails:

```bash
# Publish in dependency order
cargo publish -p grabme-common
sleep 30
cargo publish -p grabme-platform-core
sleep 30
cargo publish -p grabme-project-model
sleep 30
cargo publish -p grabme-processing-core
sleep 30
cargo publish -p grabme-audio-ai
sleep 30
cargo publish -p grabme-platform-linux
sleep 30
cargo publish -p grabme-platform-windows
sleep 30
cargo publish -p grabme-platform-macos
sleep 30
cargo publish -p grabme-input-tracker
sleep 30
cargo publish -p grabme-capture-engine
sleep 30
cargo publish -p grabme-render-engine
sleep 30
cargo publish -p grabme-cli
```

## System Dependencies

For building and testing on Linux:

```bash
sudo apt-get install -y \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev \
  gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad \
  gstreamer1.0-plugins-ugly \
  gstreamer1.0-libav \
  gstreamer1.0-tools \
  gstreamer1.0-x \
  gstreamer1.0-alsa \
  gstreamer1.0-pulseaudio \
  libx11-dev \
  libxrandr-dev \
  libxext-dev \
  libxfixes-dev \
  libevdev-dev \
  ffmpeg \
  libdbus-1-dev \
  pkg-config
```

## Troubleshooting

### "crate already uploaded"

This is expected if re-running a release. The workflow uses `continue-on-error: true` to skip already-published crates.

### "no matching package found"

Ensure dependencies are published in the correct order. The workflow handles this automatically.

### "failed to verify package"

This can happen if system dependencies are missing. The CI workflow installs all required dependencies.

## Version Bump Strategy

For the next release:

1. **Patch** (0.1.0 → 0.1.1): Bugfixes only
2. **Minor** (0.1.0 → 0.2.0): New features or breaking changes during 0.x
3. **Major** (0.x.y → 1.0.0): Stability declaration, API freeze

Post-1.0:
- Breaking changes require major bump
- New features require minor bump
- Bugfixes require patch bump
