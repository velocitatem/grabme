# Automated Testing Infrastructure

GrabMe now has comprehensive automated testing with virtual desktop integration and CV-based verification.

## Test Suite Components

### 1. Virtual Desktop Tests (`tools/vdesktop-tests`)

**Purpose**: End-to-end recording verification with synthetic data

**Features**:
- Xvfb virtual display automation
- Synthetic test pattern generation
- Computer vision-based quality verification
- Cursor tracking accuracy measurement
- Image corruption detection
- JSON test reports

**Test Patterns**:
1. **Tracking Pattern**: Colored markers at known positions (corners, center, edges)
2. **Grid Pattern**: Coordinate grid with quadrant markers
3. **Quality Pattern**: Checkerboard, gradients, and color bars

**Verification Metrics**:
- Tracking accuracy (expected vs recorded cursor positions)
- Image quality (brightness analysis, corruption detection)
- Overall pass/fail status

**Usage**:
```bash
cargo run -p vdesktop-tests -- --display 99 --duration 10
```

### 2. Cursor Drift Tests (`tools/e2e-cursor-drift`)

**Purpose**: Measure cursor tracking precision over time

**Features**:
- Automated corner-to-corner cursor movement
- Event stream analysis
- Drift measurement in pixels
- Pass/fail criteria (30px tolerance)

**Usage**:
```bash
cargo run -p e2e-cursor-drift -- --width 1280 --height 720
```

## CI/CD Integration

### Workflows

**1. Main CI Pipeline** (`.github/workflows/ci.yml`)
- Runs on every push/PR
- Format, lint, test, package checks
- All workspace crates

**2. Virtual Desktop Tests** (`.github/workflows/vdesktop-tests.yml`)
- Runs on push/PR + nightly schedule
- Full Xvfb integration
- 15-second recording test
- Uploads test artifacts

**3. Release Pipeline** (`.github/workflows/release.yml`)
- Triggered by version tags
- Publishes to crates.io
- Creates GitHub releases

**4. Binary Distribution** (`.github/workflows/dist.yml`)
- Builds Linux x86_64 binary
- Creates release tarballs

### Test Artifacts

All test runs upload:
- `test_report.json` - Metrics and pass/fail status
- `patterns/` - Generated test patterns
- `meta/events.jsonl` - Event streams

Retention: 7 days

## Test Architecture

```
┌─────────────────────────────────────────┐
│           Xvfb Virtual Display           │
│              :99  1920x1080              │
│  ┌────────────────────────────────────┐ │
│  │     Synthetic Test Pattern         │ │
│  │  (tracking.png displayed by feh)   │ │
│  └────────────────────────────────────┘ │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│       GrabMe Capture Session             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐│
│  │ Screen   │ │ Input    │ │ Events   ││
│  │ Recorder │ │ Tracker  │ │ Logger   ││
│  └─────┬────┘ └─────┬────┘ └────┬─────┘│
│        │            │            │      │
│        ▼            ▼            ▼      │
│   screen.mkv   pointer     events.jsonl│
└────────┬────────────┬────────────┬──────┘
         │            │            │
         ▼            ▼            ▼
┌─────────────────────────────────────────┐
│        CV Verification Module            │
│  ┌──────────────┐ ┌──────────────────┐  │
│  │ Frame        │ │ Tracking         │  │
│  │ Extraction   │ │ Accuracy         │  │
│  │ (FFmpeg)     │ │ Analysis         │  │
│  └──────┬───────┘ └────────┬─────────┘  │
│         │                  │            │
│         ▼                  ▼            │
│   Image Quality      Position Matching  │
│   Metrics            Drift Calculation  │
└────────┬────────────────────┬───────────┘
         │                    │
         └────────┬───────────┘
                  ▼
         ┌────────────────────┐
         │   test_report.json  │
         │                    │
         │ - Tracking: 100%   │
         │ - Drift: 8.3px     │
         │ - Quality: Pass    │
         │ - Status: ✅ Pass  │
         └────────────────────┘
```

## Pass/Fail Criteria

### Virtual Desktop Tests

**Pass** (✅):
- Tracking accuracy ≥ 90%
- No frame corruption
- All 14 test points matched within 50px

**Warning** (⚠️):
- Tracking accuracy 70-89%
- Marginal image quality

**Fail** (❌):
- Tracking accuracy < 70%
- Frame corruption detected
- Missing/invalid test report

### Cursor Drift Tests

**Pass** (✅):
- ≥3 corners matched within 30px
- Average drift < 25px
- Max drift < 50px

**Fail** (❌):
- <3 corners matched
- Excessive drift

## Running Tests Locally

### Prerequisites

```bash
sudo apt-get install -y \
  xvfb x11-utils xdotool feh ffmpeg \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libx11-dev libxrandr-dev libevdev-dev \
  libdbus-1-dev pkg-config
```

### Full Test Suite

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Lint
cargo clippy --workspace --all-targets -- -D warnings

# 3. Unit tests
cargo test --workspace

# 4. Virtual desktop test
cargo run -p vdesktop-tests -- --display 99 --duration 10

# 5. Cursor drift test  
cargo run -p e2e-cursor-drift -- --output-dir drift_test
```

### CI Simulation

```bash
# Run exactly what CI runs
./.github/scripts/run-ci-locally.sh  # (if created)
```

## Test Reports

### Example Success Report

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

### Example Failure Report

```json
{
  "tracking_accuracy": {
    "total_events": 423,
    "expected_points": 14,
    "matched_points": 6,
    "avg_drift_px": 87.2,
    "max_drift_px": 156.8,
    "accuracy_percent": 42.9
  },
  "image_quality": {
    "frames_analyzed": 8,
    "avg_brightness": 2.1,
    "min_brightness": 0.0,
    "max_brightness": 4.3,
    "has_corruption": true
  },
  "overall_status": "Fail"
}
```

## Debugging Failed Tests

### Low Tracking Accuracy

1. Check event stream:
   ```bash
   jq . vdesktop_test_output/*/meta/events.jsonl | less
   ```

2. Verify cursor movement:
   ```bash
   export DISPLAY=:99
   xdotool mousemove 100 100
   ```

3. Increase test duration:
   ```bash
   cargo run -p vdesktop-tests -- --duration 30
   ```

### Image Corruption

1. Extract and inspect frames:
   ```bash
   ffmpeg -i screen.mkv -vf "select='not(mod(n\,30))'" frame_%03d.png
   ```

2. Check GStreamer pipelines:
   ```bash
   GST_DEBUG=3 cargo run -p vdesktop-tests
   ```

3. Verify Xvfb rendering:
   ```bash
   export DISPLAY=:99
   xwd -root | xwdtopnm | pnmtopng > screenshot.png
   ```

### Missing Dependencies

```bash
# Check all required tools
command -v Xvfb xdotool feh ffmpeg xdpyinfo
```

## Performance Benchmarks

Expected performance on Ubuntu 22.04 (4-core, 8GB RAM):

- Xvfb startup: <2s
- Pattern generation: <0.5s
- 10s recording: ~10.5s walltime
- Frame extraction: ~1s
- CV analysis: <1s
- Total test time: ~15s

## Coverage

### What's Tested

✅ Screen capture (X11 and Wayland via Xvfb)
✅ Input event tracking (cursor positions)
✅ Event stream serialization
✅ Project file structure
✅ Basic image quality
✅ Cursor tracking accuracy

### What's Not Tested (Yet)

⏸️ Webcam capture
⏸️ Audio recording (mic/system)
⏸️ Timeline rendering
⏸️ Export formats (MP4/H265/WebM/GIF)
⏸️ Multi-monitor setups
⏸️ Auto-director algorithms
⏸️ Performance under load

## Future Enhancements

### Planned

- [ ] Multi-monitor test scenarios
- [ ] Audio quality verification (mic/system)
- [ ] Timeline rendering validation
- [ ] Export format compatibility matrix
- [ ] Performance stress testing
- [ ] Parallel test execution
- [ ] OpenCV advanced CV features

### Under Consideration

- [ ] Webcam simulation with synthetic video
- [ ] Real-time performance monitoring
- [ ] Chaos testing (CPU/mem pressure)
- [ ] Cross-platform test parity (Windows/macOS)
- [ ] Visual regression testing
- [ ] Automated bisection for regressions

## Contributing

To add new tests:

1. Create test in `tools/vdesktop-tests/src/`
2. Update `synthetic.rs` for new patterns
3. Update `verify.rs` for new metrics
4. Update CI workflow if needed
5. Document in this file

## References

- [Testing Strategy](testing-strategy.md)
- [Virtual Desktop Tests README](../tools/vdesktop-tests/README.md)
- [CI Workflows](../.github/workflows/)
- [Architecture](architecture.md)

---

**Status**: ✅ Automated testing infrastructure complete
**Coverage**: Core recording + tracking validation
**CI**: Fully integrated with GitHub Actions
