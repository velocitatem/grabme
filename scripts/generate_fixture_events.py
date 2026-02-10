#!/usr/bin/env python3
"""Generate synthetic 10-minute events.jsonl for the sample fixture project."""

import json
import math


def main():
    duration_secs = 600
    sample_rate_hz = 60
    total_samples = duration_secs * sample_rate_hz + 1

    events = []

    # Header comment
    header = {
        "schema_version": "1.0",
        "epoch_monotonic_ns": 0,
        "epoch_wall": "2026-01-01T00:00:00Z",
        "capture_width": 1920,
        "capture_height": 1080,
        "scale_factor": 1.0,
        "pointer_sample_rate_hz": sample_rate_hz,
    }

    for i in range(total_samples):
        t_ns = int(i * (1e9 / sample_rate_hz))
        phase = (i // sample_rate_hz) % 60

        if phase < 20:
            x = 0.15 + 0.02 * math.sin(i * 0.1)
            y = 0.15 + 0.02 * math.cos(i * 0.13)
        elif phase < 40:
            p = (phase - 20) / 20.0
            x = 0.15 + (0.72 - 0.15) * p + 0.01 * math.sin(i * 0.2)
            y = 0.15 + (0.52 - 0.15) * p + 0.01 * math.cos(i * 0.25)
        else:
            p = (phase - 40) / 20.0
            x = 0.72 + (0.48 - 0.72) * p + 0.04 * math.sin(i * 0.08)
            y = 0.52 + (0.48 - 0.52) * p + 0.04 * math.cos(i * 0.08)

        x = max(0.0, min(1.0, x))
        y = max(0.0, min(1.0, y))
        events.append(
            {"t": t_ns, "type": "pointer", "x": round(x, 4), "y": round(y, 4)}
        )

        if i % (sample_rate_hz * 5) == 0:
            events.append(
                {
                    "t": t_ns,
                    "type": "click",
                    "button": "left",
                    "state": "down",
                    "x": round(x, 4),
                    "y": round(y, 4),
                }
            )
            events.append(
                {
                    "t": t_ns + 80_000_000,
                    "type": "click",
                    "button": "left",
                    "state": "up",
                    "x": round(x, 4),
                    "y": round(y, 4),
                }
            )

        if i % (sample_rate_hz * 3) == 0:
            events.append(
                {
                    "t": t_ns,
                    "type": "key",
                    "code": "KeyA",
                    "state": "down",
                }
            )
            events.append(
                {
                    "t": t_ns + 40_000_000,
                    "type": "key",
                    "code": "KeyA",
                    "state": "up",
                }
            )

    # Sort by timestamp
    events.sort(key=lambda e: e["t"])

    # Write output
    output_path = "fixtures/sample-project/meta/events.jsonl"
    with open(output_path, "w") as f:
        f.write(f"# {json.dumps(header)}\n")
        for event in events:
            f.write(json.dumps(event) + "\n")

    print(
        f"Generated {len(events)} events ({duration_secs}s @ {sample_rate_hz}Hz) to {output_path}"
    )


if __name__ == "__main__":
    main()
