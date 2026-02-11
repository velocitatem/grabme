# Export Pipeline

## Overview

Export reads `project.json`, `timeline.json`, source media, and `events.jsonl`
to build a deterministic FFmpeg graph.

## Key behavior

- Timeline viewports are respected by default.
- Full-screen override is available only for debugging:
  - `GRABME_FORCE_FULL_SCREEN_RENDER=1`
- Cursor coordinates prefer explicit schema metadata.
- Legacy projects still use heuristic cursor projection fallback.

## Stream alignment

- Screen is the timeline reference (`t0`).
- Webcam, mic, and system tracks apply per-track `offset_ns` relative to screen
  via `-itsoffset`.
- Both audio tracks are mixed when present:
  - `amix=inputs=2:weights='1 1':normalize=0`

## Monitor pre-crop fallback

If source dimensions look like a full virtual-desktop capture while recording
metadata indicates a single monitor target, export pre-crops source video to
the selected monitor slot before timeline transforms.

Inputs used for fallback:

- `recording.monitor_x`
- `recording.monitor_y`
- `recording.monitor_width`
- `recording.monitor_height`
- `recording.virtual_x`
- `recording.virtual_y`
- `recording.virtual_width`
- `recording.virtual_height`

## Canvas style controls

`project.export.canvas` controls the cinematic frame styling:

- `background`
- `corner_radius`
- `shadow_intensity`
- `padding`

Defaults preserve existing look.

## Cursor motion trail

`timeline.cursor_config.motion_trail` enables optional ghosted cursor layers:

- disabled by default
- 2-4 trailing layers
- speed-threshold gated

## Diagnostics artifacts

Each export writes:

- `output.ffmpeg-debug.txt`
- `output.sync-report.json`
- `output.verification.json`
