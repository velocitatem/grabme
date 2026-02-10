# GrabMe Data Contracts

All data formats are versioned. The `version` field in each file allows
forward-compatible migrations.

## Project Bundle Layout

```
projects/<id>/
├── sources/
│   ├── screen.mkv      Raw video, full resolution, NO cursor
│   ├── webcam.mkv      Separate webcam track (optional)
│   ├── mic.wav         Microphone audio
│   └── system.wav      Desktop/system audio
├── meta/
│   ├── project.json    Project metadata and configuration
│   ├── timeline.json   Editing decisions (keyframes, effects, cuts)
│   └── events.jsonl    Timestamped input events (append-only)
├── cache/
│   ├── waveforms/      Pre-computed audio waveforms
│   └── proxies/        Low-res proxy videos for preview
└── exports/
    └── output.mp4      Rendered final video
```

## Event Stream (events.jsonl)

JSONL format, one event per line. First line is a comment-header.

### Header
```json
# {"schema_version":"1.0","epoch_monotonic_ns":0,"epoch_wall":"2026-01-01T00:00:00Z","capture_width":1920,"capture_height":1080,"scale_factor":1.0,"pointer_sample_rate_hz":60}
```

### Event Types
```json
{"t":0,"type":"pointer","x":0.5,"y":0.3}
{"t":100000000,"type":"click","button":"left","state":"down","x":0.5,"y":0.3}
{"t":200000000,"type":"key","code":"KeyA","state":"down"}
{"t":300000000,"type":"scroll","dx":0.0,"dy":-0.1,"x":0.5,"y":0.3}
{"t":400000000,"type":"window_focus","window_title":"Terminal","app_id":"gnome-terminal"}
```

### Coordinate System
- All `x`, `y` values: normalized [0.0, 1.0] relative to capture region
- `t`: monotonic nanoseconds since recording start
- Buttons: `left`, `right`, `middle`, `back`, `forward`
- States: `down`, `up`

## Timeline (timeline.json)

```json
{
  "version": "1.0",
  "keyframes": [
    {
      "t": 0.0,
      "viewport": {"x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0},
      "easing": "ease_in_out",
      "source": "auto"
    }
  ],
  "effects": [
    {"type": "cursor_smooth", "strength": 0.7},
    {"type": "click_highlight", "color": "#FF0000", "radius": 0.02, "duration_secs": 0.3}
  ],
  "cursor_config": {
    "smoothing": "ema",
    "smoothing_factor": 0.3,
    "size_multiplier": 1.0,
    "custom_asset": null,
    "show_click_animation": true
  },
  "cuts": [
    {"start_secs": 10.0, "end_secs": 15.0, "reason": "silence"}
  ]
}
```

## Project (project.json)

```json
{
  "version": "1.0",
  "name": "My Tutorial",
  "id": "abc123",
  "created_at": "2026-01-01T00:00:00Z",
  "modified_at": "2026-01-01T00:05:00Z",
  "recording": {
    "capture_width": 1920,
    "capture_height": 1080,
    "fps": 60,
    "scale_factor": 1.0,
    "display_server": "wayland",
    "cursor_hidden": true,
    "audio_sample_rate": 48000
  },
  "tracks": {
    "screen": {"path": "sources/screen.mkv", "duration_secs": 300.0, "codec": "h264", "offset_ns": 0},
    "mic": {"path": "sources/mic.wav", "duration_secs": 300.0, "codec": "pcm", "offset_ns": 0}
  },
  "export": {
    "format": "mp4-h264",
    "width": 1920,
    "height": 1080,
    "fps": 60,
    "video_bitrate_kbps": 8000,
    "audio_bitrate_kbps": 192,
    "aspect_mode": "landscape",
    "burn_subtitles": false
  }
}
```
