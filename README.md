# EasyCue3

A simple theatrical lighting console I've been building for myself. After years of running lights for amateur theatre productions — mostly with whatever aging console came with the venue — I wanted something small, fast, and easy to hand off to a board op who's never touched a lighting console before.

It's a hobby project. The scope is deliberately narrow: small venues, simple shows, a cue list that just works.

---

## What it does

- Cue list with GO / BACK / STOP and smooth crossfades
- Record cues from current channel state
- Edit cue labels, fade times, and notes inline
- EOS-style command line (`1 Thru 10 At 50`)
- Fixture patching (assign fixtures to DMX addresses)
- Channel grid for live control
- Audio cues with fade in/out and cross-triggering to/from lighting cues
- Save and load show files (JSON, human-readable)
- Virtual DMX backend for working without hardware
- USB DMX output (Enttec USB Pro)
- Art-Net DMX over ethernet

Supports up to 2 universes (1024 channels). That's plenty for a 200-seat black box.

## What it doesn't do (yet)

- Video playback — planned, not started
- Multiple simultaneous audio streams
- Effects/chases
- Groups and presets
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

## Show files

JSON format, lives in `shows/`. Human-readable and git-friendly — you can diff them between rehearsals. Fixture profiles go in `fixture_profiles/` or `~/.config/easycue3/`.

---

## Status

Works well enough that I'd use it for a real show, with the caveat that it's still rough around the edges. The core cue engine, audio playback, and fixture patching are solid. Hardware DMX has been tested with an Enttec USB Pro. The UI is functional but not polished.

See `docs/STATUS.md` for details on what's built and what's next.

---

## License

GPL-3.0-or-later
