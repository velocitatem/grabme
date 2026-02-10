# GrabMe Desktop (Phase 2)

Tauri v2 + React playback app for synthetic cursor preview.

## Features

- Video player preview for project recordings
- Event-driven cursor overlay from `events.jsonl`
- UI controls for smoothing strength
- Algorithm selector: EMA / Bezier / Kalman
- SVG cursor asset switching for high DPI
- Click pulse animation rendering

## Run

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Use `./recording` as the default project path, then click `Load Project`.
