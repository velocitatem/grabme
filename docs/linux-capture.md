# Linux Capture Notes

## Monitor selection

- Full-screen capture now fails fast on invalid monitor index.
- Error messages include a numbered list of detected monitors.
- Capture no longer silently falls back to monitor `0`.

## X11 region math

- `ximagesrc` region bounds now use inclusive `endx/endy`.
- Width/height validation rejects zero-sized regions.
- Non-zero-origin monitor capture is covered by unit tests.

## Metadata persisted at start

`project.json` records:

- selected monitor index and name
- monitor geometry (`x/y/width/height`)
- virtual desktop bounds
- pointer coordinate-space contract

## Dimension sanity warning

On stop, capture probes screen source dimensions and logs a warning if they
differ from selected monitor metadata.

## Input coordinate contract

- Evdev backend emits virtual-desktop-normalized coordinates.
- Event header now stores `pointer_coordinate_space`.
