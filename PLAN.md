# GrabMe Production Hardening + Cross-Platform Handoff Plan

## Mission
Ship GrabMe as a production-ready screen recorder/editor with:
- reliable single-monitor targeting (no accidental dual-monitor render),
- correct webcam/audio/screen sync,
- live webcam visibility during recording UX,
- easy record -> analyze -> export flow,
- clean abstraction for Windows/macOS backends.

## Current Findings (must be treated as blockers)
- Monitor targeting bug risk on X11 capture region math in `crates/capture-engine/src/pipeline.rs`.
- Export is forced to full-screen mode via `FORCE_FULL_SCREEN_RENDER` in `crates/render-engine/src/export.rs`, which bypasses timeline camera work.
- Offset handling is incomplete: export applies timing offset to webcam only; mic/system are not offset-aligned in `crates/render-engine/src/export.rs`.
- Audio selection maps either mic or system, not both mixed, in `crates/render-engine/src/export.rs`.
- Capture backend abstraction still depends on Linux-only monitor type in `crates/capture-engine/src/backend/mod.rs`.
- Evidence in debug artifacts shows source dimension mismatch risk (for example source `4480x1440` while selected monitor metadata is `2560x1440`).

## Guardrails For Next Agent
- Do not revert unrelated user changes.
- Keep existing behavior behind defaults where possible; add flags for risky behavior changes.
- Preserve backward compatibility for old recordings (`project.json` and `events.jsonl`) with serde defaults.
- Add tests with every logic change in capture/render mapping paths.

## Execution Order (strict)
1. Capture correctness and monitor isolation.
2. Sync engine (offset correction and export-time alignment).
3. Export pipeline behavior (timeline/canvas, audio mix, monitor pre-crop fallback).
4. Live webcam preview UX during recording.
5. Platform abstraction prep for Windows/macOS.
6. Timeline editor prototype in desktop app.
7. Validation and docs.

---

## Phase 0: Capture Correctness (P0)

### Task P0.1: Fix X11 region bounds
Files:
- `crates/capture-engine/src/pipeline.rs`

Changes:
- In X11 pipeline builder, compute `endx` and `endy` correctly for `ximagesrc` rectangular capture.
- Ensure rectangle width and height are valid positive values before building launch string.
- Add unit tests for region-to-launch conversion.

Acceptance:
- X11 selected monitor capture records only selected monitor content.
- No accidental full virtual-desktop capture when monitor index is non-zero.

### Task P0.2: Strict monitor selection + metadata sanity
Files:
- `crates/capture-engine/src/session.rs`
- `crates/capture-engine/src/backend/linux.rs`
- `crates/platform-linux/src/display.rs`

Changes:
- If requested monitor index is invalid, fail with explicit error listing available monitors (instead of silently falling back).
- Persist selected monitor identity and geometry deterministically in project metadata.
- Add explicit warning if captured source dimensions differ from selected monitor dimensions.

Acceptance:
- Recording fails fast on invalid monitor index.
- `meta/project.json` always records correct monitor geometry at start.

### Task P0.3: Pointer coordinate space contract
Files:
- `crates/project-model/src/event.rs`
- `crates/project-model/src/project.rs`
- `crates/input-tracker/src/lib.rs`
- `crates/input-tracker/src/backends.rs`
- `crates/render-engine/src/export.rs`

Changes:
- Add pointer coordinate-space metadata (serde-defaulted for old projects).
- Normalize and label emitted coordinates deterministically (capture-space vs virtual-space).
- Export should prefer explicit coordinate-space mapping and only use heuristics for legacy projects.

Acceptance:
- New recordings render cursor in correct monitor-local position without heuristic guessing.
- Legacy projects still export correctly.

---

## Phase 1: Sync Engine (P0)

### Task P1.1: Capture start/stop alignment improvements
Files:
- `crates/capture-engine/src/session.rs`
- `crates/capture-engine/src/pipeline.rs`

Changes:
- Build all enabled pipelines first, then start as a near-simultaneous group.
- Record initial clock offsets for all active tracks.
- After stop, probe media durations and compute corrected effective offsets for webcam/mic/system relative to screen.

Acceptance:
- Webcam lag reduced significantly in short and long recordings.
- Track offsets in `project.json` reflect corrected start alignment.

### Task P1.2: Apply offsets to ALL streams in export
Files:
- `crates/render-engine/src/export.rs`

Changes:
- Apply `-itsoffset` logic not only to webcam but also mic and system tracks.
- Use screen as time-zero reference for all streams.
- Handle positive and negative offsets safely.

Acceptance:
- Webcam/mic/system all align to screen timeline in exported file.

### Task P1.3: Mix mic + system audio (instead of picking one)
Files:
- `crates/render-engine/src/export.rs`
- `crates/render-engine/src/compositor.rs` (if needed for API shape)

Changes:
- Build filter graph audio section to:
  - pass-through single source when only one exists,
  - `amix` when both mic and system are present.
- Keep a simple config default (equal weights), add optional future ducking hook.

Acceptance:
- Exports contain both mic and system when both tracks exist.
- No regression when only one audio source is present.

### Task P1.4: Sync diagnostics artifact
Files:
- `crates/render-engine/src/export.rs`

Changes:
- Write a `*.sync-report.json` beside output including:
  - per-track offsets,
  - probed durations,
  - effective deltas vs screen,
  - warnings when drift exceeds threshold.

Acceptance:
- Sync report generated on every export and useful for debugging.

---

## Phase 2: Export Behavior + Cinematic Defaults (P0/P1)

### Task P2.1: Remove forced full-screen rendering
Files:
- `crates/render-engine/src/export.rs`

Changes:
- Remove or default-disable `FORCE_FULL_SCREEN_RENDER`.
- Respect timeline viewports by default.
- Keep optional env fallback only for debugging.

Acceptance:
- Timeline keyframes visibly affect final output again.

### Task P2.2: Monitor pre-crop fallback for mismatched source dimensions
Files:
- `crates/render-engine/src/export.rs`
- `crates/project-model/src/project.rs`

Changes:
- If source dimensions suggest full virtual desktop but recording metadata indicates single monitor target, pre-crop source to selected monitor slot before camera timeline transforms.
- Use `monitor_x`, `monitor_y`, `monitor_width`, `monitor_height`, and virtual bounds from metadata.

Acceptance:
- Exports from accidental full-desktop recordings still isolate selected monitor correctly.

### Task P2.3: Canvas mode defaults and controls
Files:
- `crates/project-model/src/project.rs`
- `crates/render-engine/src/export.rs`
- `docs/export-pipeline.md`

Changes:
- Add export config fields for canvas style (background, corner radius, shadow intensity, padding).
- Keep current visual style as default.
- Ensure deterministic ffmpeg graph generation for new fields.

Acceptance:
- Canvas look is configurable without code edits.

### Task P2.4: Cursor motion blur trail (initial implementation)
Files:
- `crates/processing-core/src/cursor_smooth.rs`
- `crates/render-engine/src/export.rs`
- `crates/project-model/src/timeline.rs`

Changes:
- Compute per-sample cursor velocity.
- Add optional render effect that blends 2-4 trailing cursor ghosts based on speed threshold.
- Keep disabled by default; make it export-configurable.

Acceptance:
- Motion blur produces visible pro cursor feel at high velocity without static blur artifacts.

---

## Phase 3: Live Webcam UX During Recording (P1)

### Task P3.1: Live webcam preview in overlay UI
Files:
- `apps/overlay-ui/src/main.rs`
- new module `apps/overlay-ui/src/webcam_preview.rs`
- `apps/overlay-ui/Cargo.toml`

Changes:
- Add lightweight webcam preview feed while recording (small movable PiP preview).
- Keep overlay unobtrusive and ensure preview window does not pollute captured monitor region.
- Add toggle in overlay settings.

Acceptance:
- User can see webcam live while recording.
- No measurable FPS drop in capture path.

### Task P3.2: Keep non-destructive workflow
Files:
- `crates/capture-engine/src/session.rs`
- `crates/project-model/src/project.rs`

Changes:
- Continue recording webcam as separate source track.
- Do NOT burn webcam into screen source by default.
- Optional future burn-in live composite mode can be scoped later.

Acceptance:
- Export remains editable and quality-preserving.

---

## Phase 4: Platform Abstraction for Windows/macOS (P1/P2)

### Task P4.1: Introduce platform-core crate
Files:
- new crate `crates/platform-core`
- workspace updates in `Cargo.toml`
- refactors in `crates/capture-engine`, `crates/input-tracker`, `crates/project-model`

Changes:
- Move shared platform contracts (monitor model, display server enum, capture trait data structs) out of Linux crate.
- Replace Linux-type leakage in `CaptureBackend` trait signatures.

Acceptance:
- Capture engine compiles without direct Linux type coupling in core interfaces.

### Task P4.2: Add backend scaffolds for Windows and macOS
Files:
- new crate `crates/platform-windows`
- new crate `crates/platform-macos`
- `crates/capture-engine/src/backend/windows.rs` (upgrade from placeholder)
- new macOS backend module in capture engine

Changes:
- Implement compile-safe backend skeletons with clear TODOs and feature-gated paths:
  - Windows target: `Windows.Graphics.Capture` + Raw Input API scaffolding.
  - macOS target: ScreenCaptureKit + Quartz event scaffolding.
- Keep Linux fully functional while adding these crates.

Acceptance:
- Workspace builds on Linux unchanged.
- Cross-platform backend architecture is ready for implementation without interface churn.

---

## Phase 5: Desktop Timeline Prototype (P2)

### Task P5.1: Visual timeline editor in React app
Files:
- `apps/desktop/src/App.tsx`
- new files under `apps/desktop/src/components/*`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/src/main.rs`

Changes:
- Add timeline track view for keyframes/cuts/events.
- Render zoom regions as draggable/resizable blocks.
- Add "Hide Mouse Jitter" toggle bound to cursor smoothing params.
- Add tauri commands to load/save timeline edits.

Acceptance:
- User can modify camera timing visually and save to `meta/timeline.json`.

---

## Test Plan (must pass before handoff complete)

### Automated
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p grabme-capture-engine`
- `cargo test -p grabme-render-engine`
- `cargo test -p grabme-processing-core`

### New tests to add
- X11 region math unit test (non-zero origin monitor).
- Export pre-crop monitor-slot test with synthetic metadata.
- Offset application tests for webcam/mic/system with positive and negative deltas.
- Audio mix graph test for dual-source audio.
- Cursor coordinate-space mapping test (new schema + legacy fallback).

### Manual scenarios
- Dual-monitor X11: record monitor 0 and monitor 1 separately; verify output only includes selected monitor.
- Webcam enabled recording: verify preview live, verify exported webcam alignment.
- Mic + system enabled: verify both audible in export.
- Timeline keyframe edit: verify export reflects edit.
- Long recording (>=10 min): verify no progressive webcam lag.

---

## Deliverables To Produce
- Code changes across listed files.
- Updated docs:
  - `README.md`
  - `docs/architecture.md`
  - `docs/export-pipeline.md`
  - `docs/linux-capture.md`
  - `docs/data-contracts.md`
  - `docs/roadmap.md`
- Example sync report artifact in `recording/exports/` from a real run.
- Brief migration note for old `project.json` and `events.jsonl` compatibility.

---

## Definition of Done
- Selected monitor is reliably isolated in capture and export.
- Webcam/audio/screen timing remains within acceptable sync threshold in real recordings.
- Export uses timeline by default and preserves cinematic canvas style.
- Live webcam preview UX exists during recording without capture regression.
- Platform abstraction no longer Linux-coupled at core interfaces.
- All tests pass and docs reflect final behavior.
