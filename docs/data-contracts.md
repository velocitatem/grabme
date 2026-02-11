# Data Contracts

## Backward compatibility

All new fields are serde-defaulted so older `project.json` and `events.jsonl`
remain loadable.

## `events.jsonl` header

First line remains a comment JSON object:

```json
# {"schema_version":"1.0","capture_width":1920,"capture_height":1080,"pointer_sample_rate_hz":60,"pointer_coordinate_space":"virtual_desktop_normalized"}
```

New field:

- `pointer_coordinate_space`
  - `capture_normalized`
  - `virtual_desktop_normalized`
  - `virtual_desktop_root_origin`
  - `legacy_unspecified` (default for old files)

## `project.json` recording fields

`recording` now includes:

- `monitor_name` (default: `""`)
- `pointer_coordinate_space` (default: `legacy_unspecified`)

Existing monitor and virtual-desktop geometry fields are still used.

## `project.json` export fields

`export.canvas`:

- `background` (hex color)
- `corner_radius`
- `shadow_intensity`
- `padding`

## `timeline.json` cursor fields

`cursor_config.motion_trail`:

- `enabled`
- `ghost_count`
- `speed_threshold`
- `frame_spacing`

Defaults keep behavior unchanged unless explicitly enabled.

## Migration note

- Old `project.json` files: missing fields resolve to defaults.
- Old `events.jsonl` headers: missing `pointer_coordinate_space` resolves to
  `legacy_unspecified`, which activates legacy cursor-mapping heuristics at
  export time.
