# GrabMe Testing Strategy

## Test Categories

### Unit Tests (per-crate)
- Schema serialization roundtrips
- Coordinate normalization/denormalization
- Smoothing algorithm correctness
- Auto-zoom chunk analysis
- Viewport interpolation and easing
- Subtitle formatting

### Golden Tests (processing-core)
- Fixed event fixtures → expected keyframe output
- Ensures Auto-Director stability across refactors
- Stored in `fixtures/` directory

### Integration Tests
- Project create → load → validate → save cycle
- Event write → read → parse cycle
- End-to-end: record (stub) → analyze → export (stub)

### Performance Tests
- Cursor smoothing on 100K+ events (must be <100ms)
- Auto-zoom on 30-minute fixture (must be <1s)
- Event parsing throughput (must handle 60Hz * 30min = 108K events)

## Fixture Data

### Sample Project Bundle
Location: `fixtures/sample-project/`

Contains a minimal valid project with synthetic events
for deterministic testing of the analysis pipeline.

### Event Fixtures
Pre-generated event streams covering:
- Dwell behavior (hover in small area)
- Scan behavior (rapid full-screen movement)
- Mixed behavior (typical tutorial recording)
- Edge cases (no events, single event, extreme coordinates)

## Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p grabme-processing-core

# With output
cargo test -- --nocapture

# Performance-sensitive tests
cargo test --release
```

## CI Pipeline

```yaml
steps:
  - cargo fmt --check
  - cargo clippy -- -D warnings
  - cargo test
  - cargo build --release
```
