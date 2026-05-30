# EasyCue

A simple theatrical lighting console I've been building for my school and my community theatre group. I wanted something small, fast, and easy enough that students and amateur operators could pick it up easily, but also useful enough that I could run my own shows. It's a hobby project. The scope is deliberately narrow: small venues, simple shows, and a user-friendly interface.

![EasyCue3 screenshot](docs/screenshot.png)

---

## What it does

- Combined Lighting & Audio cue list with simple navigation, timing and auto-follows
- Record & update cues from current channel state
- EOS-style command line (`1 Thru 10 At 50`) and mouse-friendly adjustments 
- Fixture patching with parameter-based control (color pickers, intensity sliders)
- Channel grid, fixture list and magic sheet for live control
- Save and load show files (JSON, human-readable)
- USB DMX output (Enttec USB Pro, Enttec Open DMX USB)


## What it doesn't do (yet)

- Pallettes or presets
- Video playback — planned, not started
- Support more than 1 universe
- Effects/chases
- Moving light features (can do basic pan/tilt, and other straight DMX channels, but that's it)

---
## Download

- Just visit the [releases page](https://github.com/davidroberthoare/easycue3/releases) to download an executable for Windows, Mac or Linux. (note: Windows drivers for ENTTEC DMXUSB Pro will need to be downloaded [from the source](https://www.enttec.com/product/dmx-usb-interfaces/dmx-usb-pro-professional-1u-usb-to-dmx512-converter/)

---
## Building

You'll need Rust. The default build has no media dependencies:

```bash
cargo build --release
cargo run --release
```

With audio support (requires ALSA on Linux):

```bash
cargo run --release --features audio
```

With everything:

```bash
# Linux: needs libudev-dev, libasound2-dev (video support also needs GStreamer)
cargo build --release --features full
```

### Linux system libraries

For the default build (USB + audio):

```bash
sudo apt-get install build-essential pkg-config libx11-dev libxi-dev \
    libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev \
    libudev-dev libasound2-dev
```

For video (when that's eventually working):

```bash
sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

On Linux, you'll also need to be in the `dialout` group for USB DMX:

```bash
sudo usermod -a -G dialout $USER
```

Open DMX USB support on Linux also depends on getting exclusive access to the FTDI serial port. If another service probes the adapter at startup, wait a moment and reconnect, or disable that service.

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| Space | GO |
| B | BACK |
| S | STOP |
| Ctrl+R | Record cue |
| Ctrl+S | Save show |
| Ctrl+O | Open show |

---

## Command line

EOS-style command syntax. Type directly — no need to click a text field first.

```
4a50          → Channel 4 at 50%
1thru10a75    → Channels 1-10 at 75%
1t10a75       → Same (t is shorthand for thru)
1-10a75       → Same (hyphen works too)
1+3+5afull    → Channels 1, 3, 5 at 100%
4aout         → Channel 4 to 0%
a50           → Set currently selected channels to 50%
4a255         → Channel 4 at raw DMX 255
```

Click channels to build a selection, then type just the level (`a50` then Enter). Shift+click adds a range; Ctrl+click toggles individual channels. The command line always reflects your current selection.

The command line is context-aware: lighting commands activate when the Channels or Lighting Cues panel is focused.

---

## Media files

Put audio files in the `media/` directory next to your project. Show files reference them by filename only (`song.mp3` rather than a full path), so shows stay portable across machines. Absolute paths still work.

---

## Custom fixtures

13 fixture profiles ship with the app: dimmer, RGB/RGBA/RGBW/RGBAW/RGBAWUV variants, LED PAR, and moving head. Add your own in `fixture_profiles/` (bundled) or `~/.config//fixture_profiles/` (user, survives updates).

```json
{
  "id": "my_fixture",
  "manufacturer": "MyBrand",
  "name": "Custom RGB",
  "channel_count": 3,
  "parameters": [
    { "parameter": "Red",   "channel_offset": 1 },
    { "parameter": "Green", "channel_offset": 2 },
    { "parameter": "Blue",  "channel_offset": 3 }
  ]
}
```

Restart EasyCue and the profile appears in the Patching panel dropdown. See `fixture_profiles/rgb.json` for a working example.

---

## Show files

JSON format, lives in `shows/`. Human-readable and git-friendly — you can diff them between rehearsals.

---


## Disclaimer

This is a hobby project I've built and tested at my local theatre group and school. I've put genuine care into making it reliable, but I can't guarantee it'll work flawlessly in production. If you do use it, I'd really appreciate hearing about any issues you run into — bugs and feedback help me make it better. But please test thoroughly before relying on it for a show.

---

## Open Source Credits

EasyCue is built on top of excellent open source projects. Thank you to all maintainers and contributors.

### Core app and UI

- [Rust](https://www.rust-lang.org/) - Systems language used for the application.
- [egui](https://github.com/emilk/egui) - Immediate mode GUI framework.
- [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) - Native app framework for egui.
- [egui_dock](https://crates.io/crates/egui_dock) - Dockable/tabbed panel layout.
- [egui_extras](https://crates.io/crates/egui_extras) - Extra widgets such as table support.
- [egui-phosphor](https://crates.io/crates/egui-phosphor) - Phosphor icon integration for egui.

### Audio and media

- [rodio](https://github.com/RustAudio/rodio) - Audio playback (feature-gated).
- [lumina-video](https://github.com/lumina-video/lumina-video) - Video playback backend (feature-gated).

### DMX, data, and platform integration

- [serialport-rs](https://github.com/serialport/serialport-rs) - USB serial communication for DMX interfaces (feature-gated).
- [artnet_protocol](https://crates.io/crates/artnet_protocol) - Art-Net packet structures and protocol support.
- [serde](https://github.com/serde-rs/serde) - Serialization framework.
- [serde_json](https://github.com/serde-rs/json) - JSON read/write support.
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime.
- [rfd](https://github.com/PolyMeilex/rfd) - Native file dialogs.
- [image](https://github.com/image-rs/image) - PNG/image loading.
- [dirs](https://github.com/dirs-dev/dirs-rs) - Cross-platform user config/data directory resolution.

### Logging, errors, and utility crates

- [anyhow](https://github.com/dtolnay/anyhow) - Application-level error handling.
- [thiserror](https://github.com/dtolnay/thiserror) - Custom error types.
- [log](https://github.com/rust-lang/log) - Logging facade.
- [env_logger](https://github.com/rust-cli/env_logger) - Logger implementation for development/runtime logs.
- [chrono](https://github.com/chronotope/chrono) - Date/time utilities.

For a complete, reproducible dependency graph (including transitive crates), see [Cargo.lock](Cargo.lock).

---

## License

GPL-3.0-or-later
