# GrabMe Export Pipeline

## Overview

The export pipeline transforms raw source media + editing decisions
into a final rendered video file. This is an offline (non-real-time)
process that prioritizes quality over speed.

## Pipeline Architecture

```
┌─────────────┐     ┌──────────┐     ┌─────────────┐     ┌────────┐
│ Source Video │────▶│ Crop/    │────▶│   Cursor    │────▶│ Encode │
│ (screen.mkv)│     │ Scale    │     │   Overlay   │     │ (H.264)│
└─────────────┘     └──────────┘     └─────────────┘     └────────┘
                         ▲                  ▲                  │
                         │                  │                  ▼
                    ┌────┴────┐        ┌────┴────┐      ┌──────────┐
                    │Timeline │        │ Events  │      │ output   │
                    │Keyframes│        │(smoothed│      │ .mp4     │
                    └─────────┘        └─────────┘      └──────────┘
```

## Per-Frame Processing

For each output frame:

1. **Time Mapping**: Convert frame number to source timestamp
2. **Cut Check**: Skip if timestamp falls in a cut segment
3. **Viewport Lookup**: Interpolate camera keyframes for this time
4. **Source Crop**: Extract viewport region from source frame
5. **Scale**: Resize cropped region to output resolution
6. **Cursor Render**: Draw synthetic cursor at smoothed position
7. **Webcam Composite**: Overlay webcam if present
8. **Subtitle Burn**: Draw subtitle text if configured
9. **Encode**: Feed frame to video encoder

Webcam composition details:
- Uses per-track clock offsets to keep webcam aligned with screen timeline
- Preserves webcam aspect ratio via scale+pad into a configurable PiP box
- Supports corner placement and opacity via `export.webcam` settings

## Codec Presets

### MP4 H.264 (Default)
- Profile: High
- Bitrate: 8 Mbps (configurable)
- Keyframe interval: 2 seconds
- Audio: AAC 192kbps

### MP4 H.265
- Profile: Main
- Bitrate: 5 Mbps (better compression)
- Audio: AAC 192kbps

### GIF
- Palette: 256 colors, per-frame optimization
- Max width: 800px (configurable)
- FPS: 15 (configurable)

### WebM
- Codec: VP9
- Bitrate: 5 Mbps
- Audio: Opus 128kbps

## Performance Targets

- Export speed: ≥0.5x realtime for H.264 baseline
- Memory: <2GB for 1080p60 exports
- Disk: Temporary space ≈ 2x source size
