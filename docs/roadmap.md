# GrabMe Development Roadmap

## Phase 0: Foundations (Week 1-2) [CURRENT]
- [x] Workspace scaffold with crate boundaries
- [x] Project model schemas (events, timeline, viewport, project)
- [x] Common utilities (clock, config, logging, errors)
- [x] CLI tool with subcommands
- [x] Test suite (49 tests passing)
- [ ] Git repository initialization
- [ ] CI pipeline setup

## Phase 1: Recorder POC (Week 3-5)
- [x] XDG Desktop Portal integration (DBus → PipeWire node)
- [x] GStreamer pipeline for PipeWire screen capture
- [x] Screen capture with `cursor_mode=hidden`
- [x] Audio capture (mic + system via PipeWire monitor)
- [x] evdev input backend for mouse tracking
- [x] Clock synchronization validation (drift <100ms)
- [x] 10-minute test recording fixture

## Phase 2: Playback + Synthetic Cursor (Week 6-8)
- [x] Tauri v2 app scaffold
- [x] React frontend with video player
- [x] Event-driven cursor overlay (`<canvas>` or `<div>`)
- [x] Cursor smoothing controls in UI
- [x] EMA / Bezier / Kalman algorithm selection
- [x] SVG cursor assets and high-res switching
- [x] Click animation rendering

## Phase 3: Auto-Director (Week 9-12)
- [x] Redesign the UI into a fully native Rust floating overlay (no Tauri runtime)
- [x] Heatmap visualization
- [x] Auto-zoom keyframe generation with tunable params
- [x] Camera motion preview (CSS transform simulation)
- [x] Manual keyframe editor (drag/drop on timeline)
- [x] Vertical (9:16) mode with cursor-centered framing
- [x] Golden tests for Auto-Director with fixture data
- [x] For edge cases of multiple monitors always focus on one monitor (the one in focus)

## Phase 4: Export Engine (Week 13-16)
- [x] GStreamer/FFmpeg render pipeline
- [x] Crop/scale from zoom keyframes
- [x] Cursor sprite overlay at smoothed coordinates
- [ ] Webcam compositing (TODO: add webcam)
- [x] H.264 MP4 output
- [x] H.265 MP4 output
- [x] Export progress reporting
- [x] Visual verification against preview

## Phase 5: Audio Intelligence (Week 17-20)
- [ ] whisper.cpp / whisper.rs integration
- [ ] Audio resampling to 16kHz
- [ ] SRT/VTT subtitle generation
- [ ] RNNoise noise suppression
- [ ] Noise gate implementation
- [ ] Subtitle burn-in during export
- [ ] Model size selection (tiny/base/small/medium)

## Phase 6: Polish (Week 21-24)
- [ ] Social vertical mode with one-click export
- [ ] Webcam occlusion detection and repositioning
- [ ] Clipboard export (render → xclip/wl-copy)
- [ ] GIF export with palette optimization
- [ ] Per-app audio isolation (PipeWire node filtering)
- [ ] Fractional scaling / HiDPI testing
- [ ] Flatpak/Snap packaging
- [ ] User documentation
