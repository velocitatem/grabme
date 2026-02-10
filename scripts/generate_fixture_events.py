#!/usr/bin/env python3
"""Generate synthetic events.jsonl for the sample fixture project.

Simulates a 10-second tutorial recording where the user:
1. Hovers in the top-left area (0-3s) — should trigger zoom-in
2. Clicks a button (3s)
3. Moves to center-right area (3-7s) — should trigger pan
4. Types some text (5-6s)
5. Returns to full screen sweep (7-10s) — should trigger zoom-out
"""

import json
import math


def main():
    events = []

    # Header comment
    header = {
        "schema_version": "1.0",
        "epoch_monotonic_ns": 0,
        "epoch_wall": "2026-01-01T00:00:00Z",
        "capture_width": 1920,
        "capture_height": 1080,
        "scale_factor": 1.0,
        "pointer_sample_rate_hz": 60,
    }

    # Phase 1: Hover in top-left (0-3s)
    for i in range(180):  # 60Hz * 3s
        t_ns = int(i * (1e9 / 60))
        # Small jitter around (0.15, 0.15)
        x = 0.15 + 0.02 * math.sin(i * 0.3)
        y = 0.15 + 0.02 * math.cos(i * 0.4)
        events.append(
            {"t": t_ns, "type": "pointer", "x": round(x, 4), "y": round(y, 4)}
        )

    # Click at 3s
    t_click = int(3e9)
    events.append(
        {
            "t": t_click,
            "type": "click",
            "button": "left",
            "state": "down",
            "x": 0.15,
            "y": 0.15,
        }
    )
    events.append(
        {
            "t": t_click + 100000000,
            "type": "click",
            "button": "left",
            "state": "up",
            "x": 0.15,
            "y": 0.15,
        }
    )

    # Phase 2: Move to center-right (3-7s)
    for i in range(240):  # 60Hz * 4s
        t_ns = int(3e9 + i * (1e9 / 60))
        progress = i / 240
        x = 0.15 + (0.7 - 0.15) * progress + 0.01 * math.sin(i * 0.5)
        y = 0.15 + (0.5 - 0.15) * progress + 0.01 * math.cos(i * 0.6)
        events.append(
            {"t": t_ns, "type": "pointer", "x": round(x, 4), "y": round(y, 4)}
        )

    # Type some text (5-6s)
    keys = "hello"
    for i, key in enumerate(keys):
        t_ns = int(5e9 + i * 200000000)  # 200ms between keys
        events.append(
            {"t": t_ns, "type": "key", "code": f"Key{key.upper()}", "state": "down"}
        )
        events.append(
            {
                "t": t_ns + 50000000,
                "type": "key",
                "code": f"Key{key.upper()}",
                "state": "up",
            }
        )

    # Phase 3: Full screen sweep (7-10s)
    for i in range(180):  # 60Hz * 3s
        t_ns = int(7e9 + i * (1e9 / 60))
        progress = i / 180
        x = 0.7 * (1 - progress) + 0.5 * progress + 0.05 * math.sin(i * 0.2)
        y = 0.5 * (1 - progress) + 0.5 * progress + 0.05 * math.cos(i * 0.3)
        events.append(
            {"t": t_ns, "type": "pointer", "x": round(x, 4), "y": round(y, 4)}
        )

    # Sort by timestamp
    events.sort(key=lambda e: e["t"])

    # Write output
    output_path = "fixtures/sample-project/meta/events.jsonl"
    with open(output_path, "w") as f:
        f.write(f"# {json.dumps(header)}\n")
        for event in events:
            f.write(json.dumps(event) + "\n")

    print(f"Generated {len(events)} events to {output_path}")


if __name__ == "__main__":
    main()
