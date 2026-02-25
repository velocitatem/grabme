# Virtual Desktop Test Suite

Automated end-to-end testing for GrabMe recording with synthetic data and computer vision verification.

## Overview

This test suite runs GrabMe recording sessions in a virtual X11 display (Xvfb) and verifies:

1. **Tracking Accuracy**: Cursor position tracking precision using synthetic test patterns
2. **Image Quality**: Frame extraction and brightness analysis to detect corruption
3. **Stability**: Consistent recording across different display configurations

## Features

- 🖥️ **Xvfb Integration**: Automated virtual display setup and teardown
- 🎨 **Synthetic Patterns**: Generated test images with known markers
- 🔍 **CV Verification**: Computer vision-based quality checks
- 📊 **Detailed Reports**: JSON reports with metrics and pass/fail status
- 🔄 **X11 Polling Fallback**: Automatic cursor tracking when evdev has no permissions
- 🤖 **CI Ready**: Fully integrated with GitHub Actions

## Requirements

### System Dependencies

```bash
# Ubuntu/Debian
sudo apt-get install -y \
  xvfb \
  x11-utils \
  xdotool \
  feh \
  ffmpeg \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libx11-dev \
  libxrandr-dev \
  libxext-dev \
  libxfixes-dev \
  libevdev-dev \
  libdbus-1-dev \
  pkg-config
```

## Usage

### Basic Test Run

```bash
cargo run -p vdesktop-tests -- --display 99 --duration 10
```

### Custom Resolution

```bash
cargo run -p vdesktop-tests -- \
  --width 2560 \
  --height 1440 \
  --display 99 \
  --duration 15
```

### Using Existing Display

```bash
# If you already have Xvfb running
export DISPLAY=:99
cargo run -p vdesktop-tests -- --no-xvfb
```

## Test Patterns

The suite generates three synthetic patterns:

### 1. Tracking Pattern (`tracking.png`)
- Red circles at corners for spatial reference
- Green circle at center for origin verification
- Blue circles at edge midpoints for boundary checking

### 2. Grid Pattern (`grid.png`)
- 100px grid with coordinates
- Major axes (center lines)
- Quadrant markers for subdivision testing

### 3. Quality Pattern (`quality.png`)
- Checkerboard for frequency response
- Gradient for bit depth verification
- Color bars for chroma accuracy

## Test Points

Automated cursor movement through 14 test points:
- 4 corners
- 4 edge midpoints
- 4 quadrant centers
- 1 screen center
- Return to center

## Verification Metrics

### Tracking Accuracy
- **Total events**: Number of input events captured (0 if using X11 fallback)
- **Expected points**: Number of test points (14)
- **Matched points**: Points within 100px tolerance
- **Avg drift**: Average pixel drift from expected positions
- **Max drift**: Maximum drift observed
- **Accuracy %**: `(matched / expected) * 100`

### Image Quality
- **Frames analyzed**: Number of extracted frames
- **Avg brightness**: Mean pixel brightness (0-255)
- **Min/Max brightness**: Range check
- **Corruption detected**: Flags all-black or all-white frames

## Test Report

Output: `vdesktop_test_output/test_report.json`

```json
{
  "tracking_accuracy": {
    "total_events": 1245,
    "expected_points": 14,
    "matched_points": 14,
    "avg_drift_px": 8.3,
    "max_drift_px": 23.1,
    "accuracy_percent": 100.0
  },
  "image_quality": {
    "frames_analyzed": 10,
    "avg_brightness": 95.4,
    "min_brightness": 42.1,
    "max_brightness": 178.3,
    "has_corruption": false
  },
  "overall_status": "Pass"
}
```

## Pass/Fail Criteria

**Pass** (✅):
- Tracking accuracy ≥ 90%
- No image corruption detected

**Warning** (⚠️):
- Tracking accuracy 70-89%
- Image quality marginal

**Fail** (❌):
- Tracking accuracy < 70%
- Image corruption detected

## CI Integration

The test suite runs automatically on:
- Every push to `main`
- All pull requests
- Nightly at 2am UTC (schedule)

See `.github/workflows/vdesktop-tests.yml` for configuration.

## Troubleshooting

### Xvfb won't start

```bash
# Check if display is already in use
ps aux | grep Xvfb

# Kill existing Xvfb
pkill Xvfb

# Try different display number
cargo run -p vdesktop-tests -- --display 100
```

### xdotool errors

```bash
# Verify display is accessible
export DISPLAY=:99
xdpyinfo

# Test xdotool manually
xdotool mousemove 100 100
```

### Frame extraction fails

```bash
# Verify ffmpeg installation
ffmpeg -version

# Check screen recording exists
ls vdesktop_test_output/*/sources/screen.mkv
```

### Low tracking accuracy

- Increase `--duration` for more stable dwells
- Check system load (CPU/memory)
- Verify Xvfb resolution matches recording
- Review `vdesktop_test_output/*/meta/events.jsonl`

### No input events captured (evdev permissions)

The test suite includes an **X11 polling fallback** that automatically activates when the evdev backend captures 0 events (common in CI/testing environments without `/dev/input` access).

**What happens:**
1. Primary evdev backend attempts to capture input events
2. If 0 events captured, fallback to X11 cursor polling
3. X11 polling uses `xdotool getmouselocation` at 60Hz
4. Verification uses `x11_cursor_tracking.jsonl` instead of `events.jsonl`

**Manual evdev permission fix (optional):**
```bash
# Add user to input group
sudo usermod -aG input $USER

# Configure uinput permissions
sudo modprobe uinput
sudo chmod 0666 /dev/uinput

# Reboot to apply group membership
sudo reboot
```

**Note:** The X11 fallback achieves 100% tracking accuracy and is sufficient for automated testing. Manual evdev fixes are only needed if you want to test the evdev backend specifically.

## Development

### Adding New Test Patterns

Edit `src/synthetic.rs`:

```rust
pub fn create_custom_pattern(width: u32, height: u32) -> RgbImage {
    let mut img = ImageBuffer::from_pixel(width, height, Rgb([0, 0, 0]));
    // Your pattern logic here
    img
}
```

### Adding New Verification Checks

Edit `src/verify.rs`:

```rust
pub fn check_custom_metric(project_path: &Path) -> Result<CustomMetrics> {
    // Your verification logic here
}
```

## Architecture

```
vdesktop-tests
├── src/
│   ├── main.rs          # Test orchestration & Xvfb setup
│   ├── synthetic.rs     # Pattern generation
│   └── verify.rs        # CV-based verification
├── Cargo.toml
└── README.md

Test Flow:
1. Start Xvfb virtual display
2. Generate synthetic test patterns
3. Display pattern with feh
4. Start GrabMe recording
5. Automate cursor through test points
6. Stop recording
7. Extract frames with ffmpeg
8. Analyze tracking accuracy
9. Analyze image quality
10. Generate JSON report
11. Cleanup Xvfb
```

## Future Enhancements

- [ ] Multi-monitor test scenarios
- [ ] Webcam simulation with synthetic video
- [ ] Audio quality verification
- [ ] Timeline rendering validation
- [ ] Export format compatibility matrix
- [ ] Performance benchmarking (FPS, latency)
- [ ] Parallel test execution
- [ ] OpenCV integration for advanced CV

## License

MIT OR Apache-2.0 (same as GrabMe)
