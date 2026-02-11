# Roadmap Status

## Completed in this hardening pass

- [x] X11 monitor-region bounds fix + tests
- [x] Strict monitor-index validation (fail-fast)
- [x] Deterministic monitor metadata persisted to project
- [x] Pointer coordinate-space schema contract + legacy fallback
- [x] Grouped pipeline startup and duration-based offset correction
- [x] Export offset alignment for webcam/mic/system
- [x] Mic + system audio mixing in export
- [x] Sync diagnostic artifact (`*.sync-report.json`)
- [x] Timeline-first export behavior (no forced full-screen by default)
- [x] Monitor pre-crop fallback for mismatched source dimensions
- [x] Canvas style config in export contract
- [x] Optional cursor motion-trail rendering
- [x] Platform core abstraction crate and Windows/macOS scaffolds

## Next

- [ ] Live webcam preview PiP in overlay UI (non-capturing)
- [ ] Desktop timeline editor interactions and save/load UX polish
- [ ] Native Windows backend implementation
- [ ] Native macOS backend implementation
