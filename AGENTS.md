# AGENTS.md

Guidance for working on EasyCue3, a Rust theatrical lighting/media console (egui 0.31). See `CLAUDE.md` for the fuller design notes; `docs/` for deep dives.

## Commands

```bash
cargo check                            # fast type-check; preferred for iteration
cargo build --release                  # default features (usb + audio + remote)
cargo build --release --features full  # + video; needs GStreamer + ALSA + libudev
cargo test                             # unit tests for effects, cue serde, fixtures, show, universe
cargo fmt && cargo clippy
```

## IMPORTANT NOTE: 
to save context tokens, I'll run manual tests myself. You can stop after doing a cargo check for obvious code errors.

- `cargo test` has one pre-existing failure: `enttec_usb_pro::tests::test_protocol_message_format`. Unrelated to app logic — don't chase it.
- Releasing: GitHub Action `.github/workflows/release.yml` builds on tag `v*` (with `--no-default-features --features audio,usb`) and packages `media`, `shows`, `fixture_profiles/` alongside the binary.

## Hard constraints

- **egui/eframe pinned to 0.31** for `lumina-video` compatibility — never bump.
- `lumina-video` is a git dependency pinned by rev (`Cargo.toml`).
- **Internal DMX range is 0–100 (percent), not 0–255.** `Universe` stores 0–100; `universe_to_dmx()`/`intensity_to_dmx()` convert at output. The command parser accepts raw 101–255 and converts. All channel math (incl. virtual intensity) is 0–100.

## Architecture — non-obvious parts

- **Crate split:** `src/lib.rs` exports library modules (`dmx`, `cue`, `audio`, `effects`, `fixtures`, `paths`, `serde_helpers`); everything else (`app`, `ui`, `show`, `command`, `groups`, `magic_sheet`, `media`, `remote`, `update`) is declared in `src/main.rs`. New modules must be registered in the right place.
- **Universes:** `app.rs` creates 8 universes (IDs 1–8). `DmxBackend::send_universes()` default sends only the first; multi-universe backends (Art-Net) override it. USB Pro/Open DMX are single-universe.
- **Art-Net is implemented** (`src/dmx/backends/artnet.rs`, wired into `app.rs:switch_to_artnet` and the settings UI) — `CLAUDE.md`'s "not yet implemented" is stale.
- **Threading:** all UI/app state on the main thread (egui). Tokio for async file I/O. `rodio`/`lumina-video` own their threads. `Arc<Mutex<T>>` only when state crosses threads — note `copilot-instructions.md` is outdated here (claims a separate DMX thread; the code sends from the main loop in `app.rs`).
- **Remote client** (`remote_client/`) is embedded at compile time via `include_bytes!` — rebuild to see PWA changes. `EASYCUE3_REMOTE=<port>[:<pin>]` env var force-enables the server for one run.

## Output path gotchas

- **Effects** never write into stored universes: `app.rs` clones them, applies `EffectEngine` to the clone just before `apply_masters`, and sends that. Recording/tracking never see effect values. While an effect runs, `update()` must keep requesting repaints.
- **Virtual intensity:** fixtures with `has_intensity()` route to a dedicated DMX channel; RGB-only go through `VirtualIntensity`. **RGBAWUV gotcha:** when storing color ratios, non-RGB channels (Amber/White/UV) must be read from the universe explicitly or they default to 0.0 and snap to black.

## Files & conventions

- **Show files:** JSON in `shows/` (human-readable, git-friendly). Audio paths stored as bare filenames resolve to `media/<name>` at load. Fixture profiles in `fixture_profiles/` (bundled) and `~/.config/easycue3/fixture_profiles/` (user). All floats serialize rounded to 2dp via `serde_helpers.rs` — keep using it or show files get noisy.
- **Errors:** `anyhow::Result` + `?`; no `.unwrap()` in production paths.
- **Logging:** `log::info!/debug!/warn!/error!` only — never `println!`. RUST_LOG controls verbosity.
- **Feature gates:** optional subsystems wrapped in `#[cfg(feature = "...")]` (`usb`, `audio`, `video`, `remote`).
- **Hot paths:** fade interpolation runs per-frame; avoid heap allocations there.
- **UI:** egui immediate-mode, dockable panels via `egui_dock`. Channels panel has two modes (`src/ui/channels.rs`): instrument list (default) and channel grid.

## Docs

- `docs/EFFECTS.md`, `docs/VIRTUAL_INTENSITY.md`, `docs/REMOTE.md` (incl. env overrides + testing), `docs/AUDIO_DEVICES.md` (Linux `~/.asoundrc` multi-channel setup, `channels N` must be pinned), `docs/MAGIC_SHEET.md`, `docs/easycue3-remote-spec.md`.
