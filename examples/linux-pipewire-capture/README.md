# Linux Window Capture Example

This example demonstrates how to capture a specific window on Linux using XComposite and the `XCompositeInputSourceBuilder`.

## Requirements

- Linux with X11 (not Wayland) 
- XComposite extension enabled
- X11 development libraries
- OBS Studio plugins installed

## Usage

First, find the window ID you want to capture:

```bash
# Get window ID interactively (click on the window)
xwininfo | grep "Window id:"

# Or list all windows
xwininfo -tree -root | grep "Window id:"
```

Then modify the `window_id` variable in `src/linux.rs` with the actual window ID and run:

```bash
cd examples/linux-window-capture
cargo run
```

## What it does

1. Initializes an OBS context
2. Creates an XComposite window capture source for the specified window
3. Sets up video and audio encoders
4. Records the window for 10 seconds
5. Saves the output to `linux-window-recording.mp4`

## Configuration

You can modify the following settings in `src/linux.rs`:

- `set_capture_window("0x...")` - Window ID to capture (hexadecimal)
- `set_show_cursor(true/false)` - Show/hide cursor in recording
- `set_include_border(true/false)` - Include window decorations/borders
- `set_exclude_alpha(true/false)` - Disable/enable transparency
- Cropping: `set_cut_top()`, `set_cut_left()`, etc.

## Window ID Format

Window IDs are typically in hexadecimal format like `0x1400001`. You can get them using:

```bash
# Interactive selection
xwininfo

# List all windows with names
wmctrl -l

# Get window info by name
xwininfo -name "Firefox"
```

## Troubleshooting

- **"Unable to open X display"**: Make sure you're running on X11 (not Wayland)
- **Black screen**: 
  - Check if XComposite extension is enabled: `xdpyinfo | grep -i composite`
  - Verify the window ID is correct and the window is visible
  - Some windows may not support XComposite capture
- **Window not found**: Double-check the window ID format (should include `0x` prefix)
- **Permissions**: Some windows may require elevated privileges to capture

## XComposite Extension

To check if XComposite is available:

```bash
xdpyinfo | grep -i composite
```

If not available, you may need to enable it in your X11 configuration.

## See Also

- `linux-screen-capture` - Example for capturing the entire screen
- Monitor the terminal output for any OBS-related errors or warnings