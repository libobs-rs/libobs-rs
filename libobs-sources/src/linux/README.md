# Linux Sources for libobs-rs

This module provides Rust bindings for Linux-specific OBS sources. These sources leverage various Linux multimedia frameworks and APIs to capture audio, video, and screen content.

## Available Sources

### Video Sources

#### X11 Screen Capture (`X11CaptureSource`)
- **Source ID**: `xshm_input`
- **Description**: Captures the entire screen or specific displays using X11's XShm extension
- **Use Case**: Desktop recording, screen sharing

**Properties:**
- `screen`: Display/screen number to capture
- `show_cursor`: Include cursor in capture
- `advanced`: Enable advanced settings (custom X server)
- `server`: X server to connect to (when advanced is enabled)
- Cropping: `cut_top`, `cut_left`, `cut_right`, `cut_bot`

#### XComposite Window Capture (`XCompositeInputSource`)
- **Source ID**: `xcomposite_input`  
- **Description**: Captures individual X11 windows using the XComposite extension
- **Use Case**: Window-specific recording with transparency support

**Properties:**
- `capture_window`: Window ID to capture
- `show_cursor`: Include cursor in capture
- `include_border`: Include window decorations/borders
- `exclude_alpha`: Disable transparency
- Cropping: `cut_top`, `cut_left`, `cut_right`, `cut_bot`

#### V4L2 Camera Input (`V4L2InputSource`)
- **Source ID**: `v4l2_input`
- **Description**: Captures video from Video4Linux2 compatible devices (webcams, capture cards)
- **Use Case**: Camera recording, external video input

**Properties:**
- `device_id`: Device path (e.g., "/dev/video0")
- `input`: Input number on the device
- `pixelformat`: Video format (FOURCC code)
- `resolution`: Width and height (packed as `width << 16 | height`)
- `framerate`: Frame rate (packed as `num << 16 | den`)
- `color_range`: Color range setting
- `buffering`: Enable internal buffering
- `auto_reset`: Auto-reset on timeout
- `timeout_frames`: Frames until timeout

### Audio Sources

#### ALSA Audio Input (`AlsaInputSource`)
- **Source ID**: `alsa_input_capture`
- **Description**: Low-level audio capture through ALSA (Advanced Linux Sound Architecture)
- **Use Case**: Direct hardware audio access, professional audio workflows

**Properties:**
- `device_id`: ALSA device identifier
- `custom_pcm`: Custom PCM device name
- `rate`: Audio sample rate in Hz

#### PulseAudio Input (`PulseInputSource`)  
- **Source ID**: `pulse_input_capture`
- **Description**: Audio capture through PulseAudio sound server
- **Use Case**: Desktop audio, application audio routing

**Properties:**
- `device_id`: PulseAudio device name

#### JACK Audio Input (`JackInputSource`)
- **Source ID**: `jack_input_capture`  
- **Description**: Professional audio capture through JACK Audio Connection Kit
- **Use Case**: Low-latency audio, professional audio production

**Properties:**
- `client_match`: JACK client name pattern
- `connect_ports`: Auto-connect to system ports

### Modern Multimedia Sources

#### PipeWire Screen Capture (`PipeWireCaptureSource`)
- **Source ID**: `pipewire-desktop-capture-source`
- **Description**: Modern screen capture through PipeWire and desktop portals
- **Use Case**: Wayland screen capture, sandboxed applications

**Properties:**
- `restore_token`: Token for reconnecting to sessions
- `session_token`: Portal session token  
- `show_cursor`: Include cursor in capture

#### PipeWire Camera (`PipeWireCameraSource`)
- **Source ID**: `pipewire-camera-source`
- **Description**: Camera capture through PipeWire camera portal
- **Use Case**: Modern camera access, sandboxed camera capture

**Properties:**
- `camera_id`: Camera device node
- `video_format`: Video format (FOURCC)
- `resolution`: Resolution as "widthxheight"
- `framerate`: Frame rate as "num/den"

## Usage Examples

### Screen Recording
```rust
use libobs_sources::linux::X11CaptureSourceBuilder;

let screen_capture = context
    .source_builder::<X11CaptureSourceBuilder, _>("Desktop")?
    .set_screen(0)
    .set_show_cursor(true)
    .set_cut_top(0)
    .set_cut_left(0)
    .add_to_scene(&mut scene)?;
```

### Webcam Recording  
```rust
use libobs_sources::linux::{V4L2InputSourceBuilder, ObsV4L2ColorRange};

let webcam = context
    .source_builder::<V4L2InputSourceBuilder, _>("Webcam")?
    .set_device_id("/dev/video0".to_string())
    .set_resolution(1920 << 16 | 1080) // 1920x1080
    .set_framerate(30 << 16 | 1)       // 30/1 fps
    .set_color_range_enum(ObsV4L2ColorRange::Full)
    .set_buffering(true)
    .add_to_scene(&mut scene)?;
```

### Audio Recording
```rust
use libobs_sources::linux::{AlsaInputSourceBuilder, PulseInputSourceBuilder};

// ALSA audio
let alsa_mic = context
    .source_builder::<AlsaInputSourceBuilder, _>("Microphone")?
    .set_alsa_device("hw:1,0")
    .set_rate(48000)
    .add_to_scene(&mut scene)?;

// PulseAudio (easier device management)  
let pulse_mic = context
    .source_builder::<PulseInputSourceBuilder, _>("Microphone")?
    .set_default_device()
    .add_to_scene(&mut scene)?;
```

### Window Capture
```rust
use libobs_sources::linux::XCompositeInputSourceBuilder;

let window_capture = context
    .source_builder::<XCompositeInputSourceBuilder, _>("Window")?
    .set_capture_window("0x1400001".to_string()) // Window ID
    .set_show_cursor(false)
    .set_include_border(true)
    .add_to_scene(&mut scene)?;
```

## Platform Requirements

- **X11**: Required for X11CaptureSource and XCompositeInputSource
- **V4L2**: Linux kernel Video4Linux2 subsystem
- **ALSA**: Advanced Linux Sound Architecture
- **PulseAudio**: PulseAudio sound server  
- **JACK**: JACK Audio Connection Kit
- **PipeWire**: Modern multimedia framework (for Wayland support)

## Notes

1. **X11 vs Wayland**: X11-based sources (X11CaptureSource, XCompositeInputSource) only work on X11. For Wayland, use PipeWire sources.

2. **Permissions**: Some sources may require specific permissions or group membership (e.g., `video` group for camera access).

3. **Device Detection**: Use system tools like `v4l2-ctl --list-devices` for V4L2 devices, `aplay -l` for ALSA devices.

4. **Resolution Encoding**: V4L2 resolution is packed as `(width << 16) | height`. Use helper methods when available.

5. **Color Ranges**: V4L2 supports different color ranges (limited vs full). Use the enum helpers for better type safety.