# GrabMe Architecture

## System Overview

```
┌──────────────────────────────────────────────────────────┐
│                     GrabMe Desktop App                    │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Tauri v2 (apps/desktop/)              │  │
│  │         React/TypeScript Frontend (UI)             │  │
│  └─────────────────────┬──────────────────────────────┘  │
│                        │ Tauri Commands (IPC)             │
│  ┌─────────────────────┴──────────────────────────────┐  │
│  │                 Rust Backend                        │  │
│  │  ┌──────────────┐  ┌─────────────┐  ┌──────────┐  │  │
│  │  │   Capture    │  │ Processing  │  │  Render  │  │  │
│  │  │   Engine     │  │    Core     │  │  Engine  │  │  │
│  │  └──────┬───────┘  └──────┬──────┘  └────┬─────┘  │  │
│  │         │                 │               │        │  │
│  │  ┌──────┴─────────────────┴───────────────┴─────┐  │  │
│  │  │            Project Model (Shared)            │  │  │
│  │  │    Events · Timeline · Viewport · Project    │  │  │
│  │  └──────────────────────────────────────────────┘  │  │
│  │         │                                          │  │
│  │  ┌──────┴──────────────────────────────────────┐   │  │
│  │  │         Platform Layer (Linux)              │   │  │
│  │  │  Portal · PipeWire · Display · Permissions  │   │  │
│  │  └─────────────────────────────────────────────┘   │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Crate Dependency Graph

```
grabme-cli ──────────────┐
                         ├──▶ grabme-capture-engine
                         │        ├──▶ grabme-input-tracker
                         │        ├──▶ grabme-platform-linux
                         │        ├──▶ grabme-project-model
                         │        └──▶ grabme-common
                         │
                         ├──▶ grabme-processing-core
                         │        ├──▶ grabme-project-model
                         │        └──▶ grabme-common
                         │
                         ├──▶ grabme-render-engine
                         │        ├──▶ grabme-project-model
                         │        └──▶ grabme-common
                         │
                         ├──▶ grabme-audio-ai
                         │        ├──▶ grabme-project-model
                         │        └──▶ grabme-common
                         │
                         └──▶ grabme-project-model
                                  (no internal deps)
```

## Core Design Decisions

### 1. Non-Destructive Pipeline
Raw media is never modified. All editing decisions (zoom, cursor smoothing, cuts)
are stored as metadata in `timeline.json` and applied during export rendering.

### 2. Normalized Coordinates
All pointer coordinates use [0.0, 1.0] range relative to the capture region.
This survives DPI changes, monitor swaps, and resolution adjustments.

### 3. Monotonic Clock Authority
All stream synchronization is anchored to a monotonic nanosecond clock
established at recording start. Wall-clock time is stored for display
purposes only.

### 4. Append-Only Events
The event stream (events.jsonl) is append-only for crash safety.
Even if the process dies mid-recording, all events up to the last
flush are recoverable.

### 5. Backend Abstraction
Input tracking and render pipelines are behind trait interfaces,
allowing multiple backends (evdev, portal, X11 for input;
GStreamer, FFmpeg for render).

## Data Flow

### Recording
```
Screen ──GStreamer──▶ screen.mkv
Mic ─────GStreamer──▶ mic.wav
System ──PipeWire──▶  system.wav
Mouse ───evdev─────▶  events.jsonl (append-only)
                      project.json (written at stop)
```

### Analysis (Auto-Director)
```
events.jsonl ──▶ Chunk Analysis ──▶ Centroid + Velocity
             ──▶ Keyframe Gen   ──▶ timeline.json
             ──▶ Cursor Smooth  ──▶ (in-memory for preview/export)
```

### Export
```
screen.mkv ──┐
timeline   ──┼──▶ Crop/Scale ──▶ Cursor Overlay ──▶ Encode ──▶ output.mp4
events     ──┘
```
