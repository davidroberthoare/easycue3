# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release                  # minimal build (no media)
cargo build --release --features full  # all features (requires system libs)
cargo run --release                    # build and run
cargo run --features audio             # with audio only
RUST_LOG=debug cargo run               # with debug logging
cargo check                            # fast type-check without building
cargo fmt                              # format code
cargo clippy                           # lint
```

There is no test suite currently.

## Feature Flags

- `usb` — USB DMX interfaces (requires `libudev` on Linux)
- `audio` — audio playback (requires ALSA on Linux; uses `rodio`)
- `video` — video playback (requires GStreamer; uses `lumina-video` from git)
- `media` — shorthand for `audio + video`
- `full` — all of the above
- **default** — `usb + audio`

egui is pinned to **0.31** for `lumina-video` compatibility — do not bump it.

## Architecture

EasyCue3 is a theatrical lighting and media console combining ETC EOS-style lighting control with QLab-style media playback. Target: small venues (50–300 seats), educational theatre.

**Scope constraints:** 2–16 universes, ~200 fixtures, 8-bit channels only.

### Module Map

| Module | Purpose |
|---|---|
| `src/app.rs` | `EasyCueApp` — central state, egui `update()` loop, all subsystem coordination |
| `src/dmx/` | 512-channel `Universe` struct + pluggable `DmxBackend` trait (Virtual, USB/Enttec, Art-Net) |
| `src/cue/` | Lighting cue recording/playback with linear crossfades |
| `src/audio/` | Parallel audio cue system; cross-triggering into lighting cues (feature-gated) |
| `src/fixtures/` | Fixture profiles (JSON), patching (fixture→DMX address), virtual intensity for color channels |
| `src/ui/` | egui immediate-mode panels (dockable via `egui_dock`): cue list, audio cues, channel grid, patching, properties |
| `src/show/` | `ShowFile` — JSON serialization of cue list + audio list + metadata |
| `src/command.rs` | EOS-style command parser (`"1 Thru 10 At 50"`) |
| `src/serde_helpers.rs` | Custom serializers that round floats to 2 decimal places (prevents `0.800000011920929` in JSON) |

### Data Flow — Cue Playback

1. User presses GO (spacebar) → UI dispatches to `App`
2. `App` calls `PlaybackEngine::go()` → reads next cue from `CueList`
3. Each frame: interpolate channels with `prev + (next - prev) * progress` (progress clamped 0.0–1.0)
4. Write interpolated values to `Universe`
5. `Universe` forwards to the active `DmxBackend` (Virtual logs; USB sends serial; Art-Net broadcasts UDP)
6. If the cue has a cross-trigger, `AudioPlaybackEngine` starts or stops the linked `AudioCue`

### Threading Model

- **Main thread only** for all UI and app state (egui requirement)
- **Tokio** for async file I/O and media loading
- `rodio` / `lumina-video` manage their own internal playback threads
- Use `Arc<Mutex<T>>` only when state must cross thread boundaries

## Conventions

- **Naming:** `snake_case` functions/variables, `CamelCase` types, `SCREAMING_SNAKE` constants
- **Errors:** `anyhow::Result` throughout; propagate with `?`; never `.unwrap()` in production paths
- **Logging:** `log::info!` / `debug!` / `warn!` / `error!` — never `println!`
- **Feature gates:** wrap optional code with `#[cfg(feature = "...")]`
- **Hot paths:** avoid heap allocations; fade interpolation runs every frame at 60 FPS
- **Public APIs:** `///` doc comments explaining *why*, not *what*

## Show File Format

JSON, human-readable, git-friendly. All floats serialized with max 2 decimal places via `serde_helpers.rs`. Show files live in `shows/`; fixture profiles live in `fixture_profiles/` and `~/.config/easycue3/`.

## Documentation

Detailed docs in `docs/`:
- `ARCHITECTURE.md` — threading model, data formats, extension points
- `STATUS.md` — development phases and completion status
- `COMMAND_LINE.md` — EOS-style command syntax reference
- `FIXTURE_SYSTEM.md` — fixture profiles, patching, color space handling
