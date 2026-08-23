# Remote Control

Phone/browser remote for EasyCue3: the desktop app embeds a web server
(feature `remote`, on by default) that serves a Framework7-based PWA and keeps
it in sync over a WebSocket. No app store, no separate install — Settings →
Remote Control, scan the QR code, optionally "Add to Home Screen".

Original design spec: a Framework7 PWA over WebSocket + REST (the "plain
HTML/JS fallback" path, chosen up front): the client is hand-written JS on
Framework7 (vendored in `remote_client/`, embedded into the binary at compile
time via `include_bytes!`). egui/wasm was rejected because desktop panel code
is too coupled to `EasyCueApp` for real reuse, and egui's mobile
text input/bundle size are poor on phones.

## Architecture

```
egui main thread                      remote server thread (tokio ×2 workers)
┌───────────────────────────┐         ┌──────────────────────────────┐
│ EasyCueApp::update()      │ cmd     │ axum: static / REST / WS     │
│  remote::glue::           │◄────────│  handlers enqueue commands,  │
│  service_frame(app, ctx)  │ mpsc    │  request_repaint() to wake   │
│   1. drain + execute cmds │         │  the UI loop                 │
│   2. diff state, publish  ├────────►│ broadcast → all sockets      │
│      (50ms/500ms throttle)│ b'cast  │ snapshot cache → new conns   │
└───────────────────────────┘         └──────────────────────────────┘
```

- The desktop app owns all engine state. The server never touches it —
  handlers enqueue `protocol::ClientMessage`s; `glue::service_frame` (called
  once per frame from `app.rs`) executes them and publishes state diffs back.
- State flows as JSON envelopes `{type, payload}`:
  - `snapshot` — full state (sent on connect; cached for `GET /api/state`)
  - `structure` — cues/patch/profiles/groups (diffed at 2 Hz via hash)
  - `channels` — per-universe 512-value array (byte-diffed, ≤20 Hz); while an
    effect runs these carry the effect-modulated staged look so the phone
    mirrors the desktop FX readouts, not the pre-effect base
  - `playback` — play head, progress, blackout, grand/sound master, audio cue
    state (fade progress), status line
  - `log` — command-line echo/result
- Client → server: `cue_go/back/stop/goto`, `set_channels`, `set_intensity`,
  `set_params` (offset→value; keeps virtual-intensity ratios in sync),
  `command_line` (with `channel`/`fixture` context), `set_master`,
  `set_sound_master`, `set_blackout`, `patch_add`, `patch_update` (label edits
  in place; ID/universe/address/profile changes go through remove + re-add
  with rollback so overlap validation runs), `patch_remove`. Patch results echo as `log` messages with `text: "patch"` (toasted client-side).
  REST mirrors: `POST /api/cue/{go,back,stop}`, `/api/channel`,
  `/api/command`; `GET /api/state`, `/api/ping`.
- Auth: optional PIN, sent as `?token=` (WS) or `x-easycue-token` header
  (REST). Empty PIN disables the check. LAN-only by design.
- Discovery: the local IP address and QR code shown in the settings dialog.
- Dual-crate rule respected: `src/remote/` lives in the binary crate;
  protocol types are plain serde structs, fixture types never cross the wire.

## Key files

| File | Purpose |
|---|---|
| `src/remote/protocol.rs` | Wire types (serde only, no engine types) |
| `src/remote/server.rs` | axum server, WS sessions, embedded assets, tests |
| `src/remote/glue.rs` | Per-frame command execution + state diffing |
| `src/remote/mod.rs` | `RemoteServer` lifecycle, settings, local IP |
| `src/ui/mod.rs` | `render_remote_settings` — enable/port/PIN/QR dialog |
| `remote_client/` | Browser client: `index.html`, `app.js`, F7 bundle, manifest |

## Client notes

- Framework7 8.3.4 (MIT), vendored — **no CDN**; venue LANs may be offline.
  Custom CSS is limited to the channel grid and a few accents (see the
  `<style>` block in `index.html`); everything else is stock F7 components.
- Six bottom tabs: Cues (GO/BACK/STOP, double-tap-to-goto, grand + sound
  master, blackout, audio cue status), Fixtures (per-fixture sheet: intensity —
  virtual for RGB-only — custom canvas HSV colour wheel matching the desktop +
  RGB sliders, profile-driven sliders; a Select mode multi-picks fixtures to
  level), Channels (512 grid, multi-select + level buttons/live slider),
  Groups (recall a group → select its fixtures, then set a level via buttons + live
  slider or paint a colour across the RGB fixtures), Patch
  (add/edit/renumber/re-address/delete; `structure.profiles` carries the whole
  library for the add picker), Cmd (command line with context toggle + log).
- The client renders optimistically and reconciles against server pushes;
  controls being dragged are held for ~600 ms so pushes don't fight fingers.
- Dom7 gotcha: `toggleClass` takes no boolean second argument (unlike jQuery).
- F7 sheet gotcha: `sheet.destroy()` does NOT remove the element — remove
  `sheet.el` manually on close or stale sheets accumulate with duplicate IDs.
- F7 z-order gotcha: sheets stack above popups; the color picker popup needs
  the `z-index` bump in `index.html` to clear the fixture sheet.
- A page whose navbar has a subnavbar needs `page-with-subnavbar` on the
  page div or content hides underneath (see the Cmd view).
- Static shell assets are served with `Cache-Control: no-cache`; live state is
  never cached and always comes from the WebSocket.
- **Client files are embedded at compile time** — editing `remote_client/*`
  requires `cargo build` to be served.

## Testing

- `cargo test remote` — network-level tests: real server on an ephemeral
  port, raw-HTTP REST/auth/static checks, raw WebSocket handshake covering
  snapshot-on-connect, broadcast fan-out, and command enqueueing.
- `EASYCUE3_REMOTE=<port>[:<pin>]` force-enables the server for one run
  without persisting (port 0 = ephemeral) — used for headless end-to-end
  testing (drive the served client with headless Chromium + CDP).
