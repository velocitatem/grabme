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
- [ ] XDG Desktop Portal integration (DBus → PipeWire node)
- [ ] GStreamer pipeline for PipeWire screen capture
- [ ] Screen capture with `cursor_mode=hidden`
- [ ] Audio capture (mic + system via PipeWire monitor)
- [ ] evdev input backend for mouse tracking
- [ ] Clock synchronization validation (drift <100ms)
- [ ] 10-minute test recording fixture

## Phase 2: Playback + Synthetic Cursor (Week 6-8)
- [ ] Tauri v2 app scaffold
- [ ] React frontend with video player
- [ ] Event-driven cursor overlay (`<canvas>` or `<div>`)
- [ ] Cursor smoothing controls in UI
- [ ] EMA / Bezier / Kalman algorithm selection
- [ ] SVG cursor assets and high-res switching
- [ ] Click animation rendering

## Phase 3: Auto-Director (Week 9-12)
- [ ] Heatmap visualization in UI
- [ ] Auto-zoom keyframe generation with tunable params
- [ ] Camera motion preview (CSS transform simulation)
- [ ] Manual keyframe editor (drag/drop on timeline)
- [ ] Vertical (9:16) mode with cursor-centered framing
- [ ] Golden tests for Auto-Director with fixture data

## Phase 4: Export Engine (Week 13-16)
- [ ] GStreamer/FFmpeg render pipeline
- [ ] Crop/scale from zoom keyframes
- [ ] Cursor sprite overlay at smoothed coordinates
- [ ] Webcam compositing
- [ ] H.264 MP4 output
- [ ] H.265 MP4 output
- [ ] Export progress reporting
- [ ] Visual verification against preview

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
