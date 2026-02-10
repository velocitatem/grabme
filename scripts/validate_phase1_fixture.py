#!/usr/bin/env python3
"""Validate Phase 1 fixture invariants.

Checks:
- events.jsonl exists and has a valid header comment
- duration is >= 10 minutes
- timestamps are monotonic
- nominal sample cadence drift is < 100ms
"""

import json
from pathlib import Path


def main() -> int:
    path = Path("fixtures/sample-project/meta/events.jsonl")
    if not path.exists():
        print(f"ERROR: missing fixture file: {path}")
        return 1

    with path.open("r", encoding="utf-8") as f:
        lines = [line.strip() for line in f if line.strip()]

    if not lines or not lines[0].startswith("# "):
        print("ERROR: first line must be JSON header comment")
        return 1

    header = json.loads(lines[0][2:])
    pointer_rate = int(header.get("pointer_sample_rate_hz", 60))
    expected_step_ns = int(1e9 / pointer_rate)

    event_ts = []
    pointer_ts = []
    for raw in lines[1:]:
        event = json.loads(raw)
        ts = int(event["t"])
        event_ts.append(ts)
        if event.get("type") == "pointer":
            pointer_ts.append(ts)

    if len(event_ts) < 2:
        print("ERROR: fixture has too few events")
        return 1

    if event_ts != sorted(event_ts):
        print("ERROR: event timestamps are not monotonic")
        return 1

    duration_ns = event_ts[-1] - event_ts[0]
    duration_secs = duration_ns / 1e9
    if duration_secs < 600:
        print(f"ERROR: duration too short: {duration_secs:.2f}s")
        return 1

    max_pointer_drift_ms = 0.0
    for a, b in zip(pointer_ts, pointer_ts[1:]):
        drift_ms = abs((b - a) - expected_step_ns) / 1e6
        if drift_ms > max_pointer_drift_ms:
            max_pointer_drift_ms = drift_ms

    if max_pointer_drift_ms >= 100.0:
        print(f"ERROR: pointer cadence drift too high: {max_pointer_drift_ms:.2f}ms")
        return 1

    print("Phase 1 fixture validation passed")
    print(f"  duration: {duration_secs:.2f}s")
    print(f"  events: {len(event_ts)}")
    print(f"  pointer cadence max drift: {max_pointer_drift_ms:.2f}ms")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
