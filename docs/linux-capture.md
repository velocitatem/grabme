# Linux Capture Guide

## Display Server Detection

GrabMe auto-detects the display server:
- **Wayland**: `$WAYLAND_DISPLAY` is set
- **X11**: `$DISPLAY` is set without `$WAYLAND_DISPLAY`

## Screen Capture on Wayland

### XDG Desktop Portal Flow

1. **DBus Connection**: Connect to `org.freedesktop.portal.ScreenCast`
2. **CreateSession**: Establish a capture session
3. **SelectSources**: Choose monitor/window with `cursor_mode`:
   - `1` = Hidden (our default - cursor NOT in video)
   - `2` = Embedded (cursor baked into video)
   - `4` = Metadata (cursor position as side-channel)
4. **Start**: Begin capture, receive PipeWire node ID
5. **PipeWire**: Connect to the node and receive video frames

### Compositor Support Matrix

| Desktop | Portal | Hidden Cursor | Metadata Cursor |
|---------|--------|--------------|-----------------|
| GNOME 42+ | Yes | Yes | No |
| KDE 5.27+ | Yes | Yes | Partial |
| Sway 1.8+ | Yes | Yes | No |
| wlroots | Yes | Yes | No |

## Webcam Capture

When `--webcam` is enabled, GrabMe records webcam video as a separate source track:

- Source: Video4Linux device (`/dev/video*`, first available device)
- Pipeline: `v4l2src -> videoconvert/videoscale/videorate -> x264enc -> matroskamux`
- Output: `sources/webcam.mkv`
- Project metadata: `tracks.webcam` in `meta/project.json`

The webcam track is composited during export (picture-in-picture) and stays optional.

## Input Tracking

### Backend Priority

1. **evdev** (best): Direct device access, works everywhere
   - Requires: user in `input` group
   - Setup: `sudo usermod -aG input $USER` (logout required)

2. **Focused-window API**: Only tracks when GrabMe has focus
   - No special permissions needed
   - Limited: loses tracking when user clicks other windows

3. **Portal metadata**: Future option when compositors support it
   - Ideal for sandboxed deployments (Flatpak)

## Audio Capture

### PipeWire Audio

GrabMe uses PipeWire for audio capture:
- **Microphone**: Default input device or selected source
- **System audio**: PipeWire monitor source (captures all desktop audio)
- **Per-app**: Connect to specific PipeWire node by app name/PID

### Permissions
- No special permissions for mic (user consent dialog)
- Monitor source requires PipeWire >= 0.3.x

## DPI / Fractional Scaling

All coordinates are normalized to [0.0, 1.0] at capture time:
```
normalized_x = (pixel_x - monitor_offset_x) / physical_width
normalized_y = (pixel_y - monitor_offset_y) / physical_height
```

This survives:
- Scale factor changes between recording and export
- Moving projects between machines with different DPI
- Mixed-DPI multi-monitor setups

## Sandboxing (Flatpak/Snap)

Required Flatpak permissions:
```
--socket=wayland
--socket=pulseaudio
--device=all           # for evdev input (optional)
--talk-name=org.freedesktop.portal.Desktop
```
