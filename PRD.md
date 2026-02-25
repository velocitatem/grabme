# Product Requirements Document: GrabMe

**Version:** 1.0  
**Date:** February 10, 2026  
**Status:** Active Development  

---

## Executive Summary

**GrabMe** is a cross-platform (Linux-first) screen recording solution that automates post-production tasks to create professional-grade tutorials and demos with minimal manual editing. Unlike traditional screen recorders that burn everything into pixels, GrabMe operates as a **non-linear editing system** that records metadata alongside raw video, enabling intelligent automated editing decisions.

---

## Vision & Goals

### Primary Vision
Transform raw screen recordings into polished, professional content through intelligent automation, eliminating 80%+ of manual editing work.

### Success Metrics
- **Quality:** Exported videos acceptable without manual edits in 70%+ of use cases
- **Performance:** <1% dropped frames during capture on supported hardware
- **Efficiency:** Export throughput ≥0.5x realtime for H.264
- **Adoption:** Linux users prefer GrabMe over OBS/SimpleScreenRecorder for tutorials

### Target Audience
- Software developers creating tutorials
- Educators producing instructional content
- Product managers recording demos
- Content creators needing professional output without editing expertise

---

## Core Value Propositions

1. **No Cursor Jitter:** Synthetic cursor rendering with motion smoothing
2. **Intelligent Framing:** Auto-zoom follows user activity without manual keyframing
3. **One-Click Social:** Auto-crop 16:9 to 9:16 with cursor-centered framing
4. **Local-First AI:** Subtitle generation and audio cleanup without cloud dependencies
5. **Non-Destructive Workflow:** Edit decisions separate from source media

---

## Product Architecture

### Three-Engine System

#### 1. Capture Engine
**Responsibility:** High-fidelity recording of raw streams + event metadata

**Inputs:**
- Screen video (Wayland/X11, cursor hidden)
- Webcam video (separate track)
- Audio (mic + system + per-app isolation)
- Input events (mouse position at 60+ Hz, clicks, keys)

**Outputs:**
- Project bundle with raw media + synchronized event stream

**Critical Requirements:**
- Cursor must NOT be burned into video feed
- All streams synchronized to monotonic clock
- Event stream survives process crashes (append-only)

#### 2. Processing Core
**Responsibility:** Analyze events to generate automated editing decisions

**Algorithms:**
- **Heatmap-to-Viewport:** Chunk timeline → calculate centroids → detect hover vs. scan → generate zoom keyframes
- **Velocity-Based Smoothing:** Apply Kalman/EMA filters to camera motion
- **Cursor Path Smoothing:** Bézier interpolation to eliminate jitter
- **Occlusion Detection:** Webcam repositioning when cursor approaches

**Outputs:**
- Timeline JSON with keyframes, camera regions, effects

#### 3. Rendering Engine
**Responsibility:** Offline composition into final video

**Pipeline:**
- Apply zoom/crop transformations
- Composite synthetic cursor at smoothed coordinates
- Overlay webcam with dynamic positioning
- Burn subtitles (optional)
- Encode to H.264/H.265/GIF

---

## Feature Specifications

### P0 Features (MVP)

#### F-001: Cursor-Free Screen Capture
- **User Story:** As a content creator, I want my cursor to appear smooth in the final video, not as captured pixels
- **Implementation:** Use XDG Desktop Portal `cursor_mode=hidden` on Wayland; hide cursor in X11
- **Acceptance Criteria:** Recorded video contains no cursor; events.jsonl contains mouse coordinates at ≥60Hz

#### F-002: Synthetic Cursor Rendering
- **User Story:** As a viewer, I see a smooth, professional cursor that doesn't jitter
- **Implementation:** Vector cursor overlay rendered from smoothed event data
- **Acceptance Criteria:** Visual jitter reduction ≥50% compared to raw capture

#### F-003: Auto-Zoom (Activity Following)
- **User Story:** As a tutorial creator, I want the camera to automatically zoom to areas where I'm working
- **Implementation:** Heatmap analysis → centroid calculation → smooth keyframe generation
- **Acceptance Criteria:** Generated zoom timeline requires <3 manual adjustments per 5-minute video

#### F-004: Project-Based Workflow
- **User Story:** As a user, I can re-edit my recording without re-recording
- **Implementation:** Save raw media + events + timeline as versioned project bundle
- **Acceptance Criteria:** Opening a project loads all assets and editing state

#### F-005: MP4 Export
- **User Story:** As a user, I can export my edited recording to a standard video format
- **Implementation:** Offline render pipeline with H.264 encoding
- **Acceptance Criteria:** Exported video matches preview; no crashes on 30-minute recordings

### P1 Features (Post-MVP)

#### F-101: Local Transcription
- **User Story:** As a content creator, I need subtitles without using cloud services
- **Implementation:** Whisper.cpp integration with offline model inference
- **Acceptance Criteria:** Generated subtitles have ≥90% accuracy; processing <2x realtime

#### F-102: Audio Cleanup
- **User Story:** As a podcaster, I want background noise removed automatically
- **Implementation:** RNNoise integration in audio pipeline
- **Acceptance Criteria:** Measurable noise floor reduction; no voice artifacts

#### F-103: Social/Vertical Mode
- **User Story:** As a social media creator, I need 9:16 vertical video with smart framing
- **Implementation:** Auto-crop with cursor-centered viewport
- **Acceptance Criteria:** Cursor stays in frame; no important content clipped

#### F-104: Webcam Occlusion Avoidance
- **User Story:** As a presenter, my webcam should never hide the cursor
- **Implementation:** Dynamic webcam repositioning when cursor approaches
- **Acceptance Criteria:** Webcam relocates within 200ms; smooth transitions

#### F-105: Per-App Audio Capture
- **User Story:** As a developer, I want to record only my app's audio, not system sounds
- **Implementation:** PipeWire node isolation by PID/app name
- **Acceptance Criteria:** Only target app audio in recording; other apps silent

### P2 Features (Future)

- GIF export with palette optimization
- Clipboard export (temp file → OS clipboard)
- High-DPI cursor asset switching
- Manual keyframe override UI
- Multi-monitor recording
- Redaction zones for sensitive content
- Collaborative project sharing

---

## User Journeys

### Journey 1: Quick Tutorial Recording
1. User launches GrabMe
2. Clicks "New Recording" → selects screen region
3. Records 5-minute demo
4. Stops recording → auto-processing runs
5. Preview shows auto-zoomed, smooth-cursor video
6. Clicks "Export" → MP4 ready in 2 minutes
7. **Result:** Professional video without manual editing

### Journey 2: Polished Product Demo
1. User records 15-minute feature walkthrough
2. Auto-zoom misses one important section
3. User opens timeline editor, adds manual zoom keyframe
4. Adjusts cursor smoothing strength
5. Enables webcam overlay
6. Exports with burned-in subtitles
7. **Result:** Broadcast-quality demo with minimal effort

### Journey 3: Social Media Clip
1. User imports existing 10-minute recording
2. Selects 30-second segment
3. Enables "Social Mode" (9:16 crop)
4. Preview shows cursor-centered vertical framing
5. Exports optimized GIF
6. Copies to clipboard, pastes into Twitter
7. **Result:** Platform-optimized content from desktop recording

---

## Technical Constraints

### Linux-Specific Constraints
- **Wayland:** Must use XDG Desktop Portal; no direct screen access
- **Input Tracking:** Global mouse polling requires privileges or focused-window APIs
- **Sandboxing:** Flatpak/Snap permissions needed for screen/audio/input capture
- **DPI Scaling:** Fractional scaling requires coordinate normalization

### Performance Requirements
- **Capture:** Real-time encoding at 1080p60 on mid-range hardware (4-core CPU)
- **UI:** Preview maintains ≥30 FPS with cursor overlay
- **Export:** Background processing; system remains responsive

### Security & Privacy
- **Local-First:** No cloud APIs for core functionality
- **Consent:** Explicit prompts before screen/audio/input access
- **Data Retention:** Users control project storage; clear deletion workflows

---

## Technology Stack

### Core
- **Language:** Rust (safety, performance, C library bindings)
- **GUI:** Tauri v2 (React/TypeScript frontend)
- **Media:** GStreamer (PipeWire integration, encoding)
- **Storage:** JSON (schemas), JSONL (events), SQLite (future indexing)

### Platform
- **Linux Capture:** XDG Desktop Portal (DBus), PipeWire
- **Audio:** CPAL or GStreamer Audio, PulseAudio/PipeWire
- **Input:** evdev (privileged) or focused-window APIs

### AI/Audio
- **Transcription:** whisper.rs / whisper.cpp
- **Noise Suppression:** RNNoise

---

## Data Model

### Project Structure
```
projects/<id>/
├── sources/
│   ├── screen.mkv      (raw video, no cursor)
│   ├── webcam.mkv      (separate track)
│   ├── mic.wav         (microphone)
│   └── system.wav      (desktop audio)
├── meta/
│   ├── events.jsonl    (timestamped input events)
│   ├── timeline.json   (zoom keyframes, effects)
│   └── project.json    (metadata, schema version)
├── cache/
│   ├── waveforms/
│   └── proxies/
└── exports/
    └── final.mp4
```

### Event Schema
```json
{"t": 1234567890123, "type": "pointer", "x": 0.5, "y": 0.3}
{"t": 1234567890456, "type": "click", "button": "left", "state": "down"}
{"t": 1234567890789, "type": "key", "code": "KeyA", "state": "down"}
```

### Timeline Schema
```json
{
  "version": "1.0",
  "keyframes": [
    {"t": 0.0, "viewport": {"x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0}},
    {"t": 5.2, "viewport": {"x": 0.1, "y": 0.1, "w": 0.5, "h": 0.5}}
  ],
  "effects": [
    {"type": "cursor_smooth", "strength": 0.7}
  ]
}
```

---

## Development Roadmap

### Phase 0: Foundations (Week 1-2)
- Workspace setup, crate boundaries
- JSON schema definitions
- CLI project validator

### Phase 1: Recorder POC (Week 3-5)
- PipeWire/portal capture with cursor hidden
- Parallel audio track recording
- Input event logging with clock sync
- **Exit Criteria:** 10-min recording with <100ms drift

### Phase 2: Playback + Synthetic Cursor (Week 6-8)
- Tauri preview player
- Event-driven cursor overlay
- Cursor smoothing implementation
- **Exit Criteria:** Visible jitter reduction, no lag

### Phase 3: Auto-Director (Week 9-12)
- Heatmap analysis algorithm
- Keyframe generation
- Camera motion smoothing
- Manual override UI
- **Exit Criteria:** 70%+ recordings need no manual edits

### Phase 4: Export Engine (Week 13-16)
- Offline render pipeline
- H.264 encoding
- Webcam compositing
- **Exit Criteria:** Output matches preview

### Phase 5: Audio Intelligence (Week 17-20)
- Whisper integration
- Subtitle generation
- Noise suppression
- **Exit Criteria:** Subtitles aligned, noise reduced

### Phase 6: Polish (Week 21-24)
- Social/vertical mode
- Webcam occlusion avoidance
- Clipboard export
- UX refinement
- **Exit Criteria:** Production-ready MVP

---

## Success Criteria

### MVP Launch Readiness
- [ ] 10 users can record, preview, and export without critical bugs
- [ ] Auto-zoom acceptable in ≥70% of test recordings
- [ ] Export completes without crashes on 30-min videos
- [ ] Performance targets met on reference hardware
- [ ] Documentation covers installation and basic workflows

### Post-MVP Success (6 months)
- [ ] 1000+ active users on Linux
- [ ] ≥4.0 rating in user feedback
- [ ] Community contributions (bug reports, feature requests)
- [ ] Feature parity with OBS for tutorial use case

---

## Open Questions & Risks

### Technical Risks
- **Q:** Can we reliably track mouse position on Wayland without focused-window limitations?
  - **Mitigation:** Implement fallback hierarchy; document privileged helper option
  
- **Q:** Will heatmap algorithm work across diverse content types (coding, design, gaming)?
  - **Mitigation:** Build test fixture library; allow algorithm parameter tuning

- **Q:** Can whisper.cpp run fast enough on CPU-only systems?
  - **Mitigation:** Make transcription optional; support multiple model sizes

### Product Risks
- **Q:** Will users trust local AI quality vs. cloud services?
  - **Validation:** Early user testing; benchmark against cloud accuracy
  
- **Q:** Is auto-zoom "magic" worth the complexity vs. manual keyframes?
  - **Validation:** A/B test with manual-only mode; measure time savings

---

## Appendix: Competitive Analysis

| Feature | GrabMe | OBS Studio | SimpleScreenRecorder | Loom |
|---------|--------|------------|---------------------|------|
| Cursor Smoothing | ✅ | ❌ | ❌ | ⚠️ (cloud) |
| Auto-Zoom | ✅ | ❌ | ❌ | ❌ |
| Local Transcription | ✅ | ❌ | ❌ | ⚠️ (cloud) |
| Non-Destructive Edit | ✅ | ❌ | ❌ | ✅ |
| Linux Native | ✅ | ✅ | ✅ | ❌ |
| Privacy (Local-First) | ✅ | ✅ | ✅ | ❌ |

**Key Differentiator:** GrabMe is the only Linux-native tool combining cursor smoothing, auto-zoom, and local AI without cloud dependencies.

---

**Document Owner:** Development Team  
**Review Cycle:** Bi-weekly during active development  
**Next Review:** February 24, 2026
