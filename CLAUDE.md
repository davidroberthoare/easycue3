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

Unit tests exist for `effects`, `cue` serde compatibility, `fixtures`, and `show` (`cargo test`). One pre-existing failure in `enttec_usb_pro::tests::test_protocol_message_format` is unrelated to app logic.

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

**Scope constraints:** 2 universes currently, ~200 fixtures, 8-bit channels only.

**Art-Net** is designed for in `DmxBackend` but not yet implemented. USB (Enttec USB Pro) is implemented and tested.

### Module Map

| Module | Purpose |
|---|---|
| `src/app.rs` | `EasyCueApp` — central state, egui `update()` loop, all subsystem coordination |
| `src/dmx/` | 512-channel `Universe` struct + pluggable `DmxBackend` trait (Virtual, USB/Enttec) |
| `src/cue/` | Lighting cue recording/playback with linear crossfades |
| `src/effects/` | Waveform effects (sine/square/saw/random) applied at the output stage; cue-triggered, tracking-style |
| `src/audio/` | Parallel audio cue system; cross-triggering into lighting cues (feature-gated) |
| `src/fixtures/` | Fixture profiles (JSON), patching (fixture→DMX address), `intensity.rs` for virtual intensity |
| `src/ui/` | egui immediate-mode panels (dockable via `egui_dock`): cue list, audio cues, channels (dual-mode), patching, properties |
| `src/show/` | `ShowFile` — JSON serialization of cue list + audio list + patch list + metadata |
| `src/command.rs` | EOS-style command parser (`"1 Thru 10 At 50"`) with context-aware routing |
| `src/serde_helpers.rs` | Custom serializers that round floats to 2 decimal places (prevents `0.800000011920929` in JSON) |

### Data Flow — Cue Playback

1. User presses GO (spacebar) → UI dispatches to `App`
2. `App` calls `PlaybackEngine::go()` → reads next cue from `CueList`
3. Each frame: interpolate channels with `prev + (next - prev) * progress` (progress clamped 0.0–1.0)
4. Write interpolated values to `Universe`
5. `Universe` forwards to the active `DmxBackend` (Virtual logs; USB sends serial)
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

## Internal DMX Range

**Important:** `Universe` stores channel values as **0–100** (percentage), not 0–255 (standard DMX). This affects all channel math, including virtual intensity calculations. The command parser accepts raw values 101–255 and converts them, but internally everything is 0–100.

## Virtual Intensity

RGB-only fixtures (no dedicated intensity channel) get a virtual intensity layer that scales all color channels proportionally, preserving hue. See `docs/VIRTUAL_INTENSITY.md` for the full design.

**Routing logic:** fixtures with `has_intensity() == true` route to the dedicated DMX channel directly; RGB-only fixtures go through `VirtualIntensity` in `src/fixtures/intensity.rs`.

**RGBAWUV gotcha:** when storing color ratios, all non-RGB channels (Amber, White, UV) must be read from the universe explicitly — otherwise they default to 0.0 and snap to black when intensity is adjusted. See `src/ui/properties.rs`.

## Effects

Waveform effects (sine, square, sawtooth, random with smoothing) modulate fixture parameters (intensity, color, pan/tilt/position) *relative to the base look*. Because there are no HTP/LTP layers, the `EffectEngine` never writes into the stored universes — it modulates a per-frame clone in the output path (`app.rs`, just before `apply_masters`), so recording and tracking never see effect values. The UI *displays* the live modulated values in FX cyan via `app.effect_display` (staged universes + `EffectFootprint`), but all interactions edit the base. Cues carry `effect_actions` (start/stop, tracking-style); `CueList::effect_state_up_to` replays them for GOTO/BACK. While any effect runs, `update()` must keep requesting repaints. See `docs/EFFECTS.md`.

## Channels Panel — Dual Mode

`src/ui/channels.rs` has two display modes toggled by the user:
- **Instrument list** (default): fixture-centric view with intensity drag controls and multi-select
- **Channel grid**: traditional 512-channel view

## Show File Format

JSON, human-readable, git-friendly. All floats serialized with max 2 decimal places via `serde_helpers.rs`. Show files live in `shows/`; fixture profiles live in `fixture_profiles/` and `~/.config/easycue3/fixture_profiles/`.

Audio file paths in show files are stored as bare filenames when the file lives in `media/` — the player resolves `song.mp3` → `media/song.mp3` at load time. Full paths still work.

## Documentation

- `docs/VIRTUAL_INTENSITY.md` — virtual intensity concept, algorithm, key files
- `docs/EFFECTS.md` — effects concept, output-stage overlay architecture, key files
