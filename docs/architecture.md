# GrabMe Architecture

## System layout

```text
apps/overlay-ui or tools/grabme-cli
            |
            v
     grabme-capture-engine  ---> grabme-input-tracker
            |                         |
            |                         +--> events.jsonl
            v
      project bundle (sources + meta)
            |
            v
      grabme-processing-core
            |
            v
       grabme-render-engine
            |
            +--> output.mp4
            +--> output.ffmpeg-debug.txt
            +--> output.sync-report.json
            +--> output.verification.json
```

## Platform abstraction

- `grabme-platform-core` defines shared monitor/display contracts:
  - `MonitorInfo`
  - `DisplayServer`
  - virtual-desktop geometry helpers
- `grabme-platform-linux` implements active capture/display logic.
- `grabme-platform-windows` and `grabme-platform-macos` provide compile-safe
  scaffolds to prevent interface churn while native backends are built.

## Capture-sync model

- All pipelines are built first, then started as a near-simultaneous group.
- Track offsets are recorded against the recording clock.
- On stop, media durations are probed and offsets are corrected relative to the
  screen track.

## Export model

- Timeline viewports drive crop/scale by default.
- `GRABME_FORCE_FULL_SCREEN_RENDER=1` keeps a debugging fallback.
- Audio tracks are aligned to the screen timeline and mixed when both mic and
  system tracks are present.
