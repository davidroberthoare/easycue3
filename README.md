# EasyCue3

A simple theatrical lighting console I've been building for myself. After years of running lights for amateur theatre productions — mostly with whatever aging console came with the venue — I wanted something small, fast, and easy to hand off to a board op who's never touched a lighting console before.

It's a hobby project. The scope is deliberately narrow: small venues, simple shows, a cue list that just works.

---

## What it does

- Cue list with GO / BACK / STOP and smooth crossfades
- Record cues from current channel state
- Edit cue labels, fade times, and notes inline
- EOS-style command line (`1 Thru 10 At 50`)
- Fixture patching with parameter-based control (color pickers, intensity sliders)
- Virtual intensity — scale brightness of RGB fixtures without losing color hue
- Channel grid and fixture instrument list for live control
- Audio cues with fade in/out and cross-triggering to/from lighting cues
- Save and load show files (JSON, human-readable)
- Virtual DMX backend for working without hardware
- USB DMX output (Enttec USB Pro)

Supports up to 2 universes (1024 channels). That's plenty for a 200-seat black box.

## What it doesn't do (yet)

- Groups and presets
- Video playback — planned, not started
- Art-Net DMX over ethernet
- Effects/chases
- Anything resembling a moving light programmer

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

13 fixture profiles ship with the app: dimmer, RGB/RGBA/RGBW/RGBAW/RGBAWUV variants, LED PAR, and moving head. Add your own in `fixture_profiles/` (bundled) or `~/.config/easycue3/fixture_profiles/` (user, survives updates).

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

Restart EasyCue3 and the profile appears in the Patching panel dropdown. See `fixture_profiles/rgb.json` for a working example.

---

## Show files

JSON format, lives in `shows/`. Human-readable and git-friendly — you can diff them between rehearsals.

---

## Status

Core cue engine, fixture patching, audio playback, and USB DMX have all been tested. The UI is functional but not polished, and a few corners are rough.

**Working:**
- Lighting cues with smooth crossfades
- Fixture patching (add/delete, color picker, individual channel sliders)
- Virtual intensity for RGB fixtures (preserves hue when scaling brightness)
- Audio cues with cross-triggering
- EOS-style command line
- USB DMX (Enttec USB Pro, tested)

**Still rough or missing:**
- No groups, presets, or effects
- Art-Net planned, not implemented

---

## License

GPL-3.0-or-later
