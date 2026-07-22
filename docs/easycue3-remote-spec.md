# EasyCue3 Remote Control — Implementation Spec

## Overview

Add a network-accessible remote control interface to EasyCue3 so a designer or
techie can operate the console from a phone (iOS or Android) over the local
venue wifi/LAN. No native mobile apps, no App Store distribution. The remote
runs as a web client (installable as a PWA) served directly by EasyCue3.

## Goals

- Control running EasyCue3 instance from any phone browser on the same LAN
- No app store distribution — browser/PWA only
- Reuse existing egui UI code where practical
- Feature set: instrument control (incl. colour + extra channels), raw DMX
  channel control, patching, cue playback, command line entry

## Non-goals (v1)

- Internet/remote-WAN access (LAN only)
- Multi-user conflict resolution / permissions (assume trusted small crew)
- Full patch editing UI parity with desktop (view + basic edit is enough)

## Architecture

**Model: embedded server in EasyCue3 + thin client(s) over WebSocket/REST.**

```
┌─────────────────────┐        LAN (wifi)        ┌─────────────────────┐
│  EasyCue3 (desktop)  │◄─────────────────────────►│   Phone browser     │
│  - lighting engine   │  WebSocket (state+cmds)   │   (PWA, installed)  │
│  - embedded server   │  REST (one-shot actions)  │                     │
│  - static file host  │                            │                     │
└─────────────────────┘                            └─────────────────────┘
```

The desktop app remains the sole owner of engine state (patch, cue list, live
channel levels). The phone client is a thin, stateless view that renders
whatever the server pushes and sends commands back. No engine logic runs on
the client beyond input handling and optimistic UI feedback.

### Client implementation: egui/wasm primary, plain JS fallback noted

Primary approach — factor shared UI panels (channel grid, cue list, patch
table, colour picker) into a crate compiled to both the native desktop target
and `wasm32-unknown-unknown` via `eframe`. The wasm build runs standalone in
the phone browser and talks to the desktop over the same WebSocket/REST API
below; it does not run in-process with the desktop engine.

Flag for Claude Code: if egui/wasm touch ergonomics (tap target sizing,
scroll/drag behavior, first-load wasm bundle size on venue wifi) prove painful
during implementation, fall back to a hand-rolled HTML/CSS/JS UI hitting the
same WebSocket/REST API. The API layer below is UI-technology-agnostic by
design specifically so this swap is low-cost.

## Server component (Rust, in EasyCue3)

- Framework: `axum` (async, good WebSocket support, pairs with existing
  tokio runtime if present)
- Runs only when user enables "Remote Control" in EasyCue3 settings —
  not on by default
- Serves:
  - Static client bundle (wasm/js/html or plain HTML/JS) at `/`
  - `GET /api/*` REST endpoints for one-shot actions
  - `GET /ws` WebSocket upgrade for live state sync + streaming commands
- Discovery: advertise via mDNS (`mdns-sd` crate) as `easycue3.local` on the
  chosen port, so users don't need to know the desktop's LAN IP
- Pairing UX: desktop app shows a QR code (encoding `http://easycue3.local:PORT`
  or the resolved IP as fallback) when Remote Control is enabled
- Optional: simple shared PIN/token set in EasyCue3 settings, sent as a
  header or query param on connect, to stop randoms on venue wifi from
  connecting. No need for full auth/accounts in v1.

## WebSocket protocol

Bidirectional JSON messages. Suggested envelope:

```json
{ "type": "<message_type>", "payload": { ... } }
```

**Server → client (state push):**
- `state_snapshot` — full state on connect (patch, current cue, live levels)
- `channel_update` — one or more DMX channel levels changed
- `cue_update` — active cue / cue list position changed
- `patch_update` — patch changed (fixture added/removed/repatched)
- `log` — command-line echo / result / error line

**Client → server (commands):**
- `set_channel` — `{ channel: u16, value: u8 }` or array for multiple
- `set_instrument` — `{ instrument_id, intensity?, colour?, extra_channels? }`
- `cue_go` / `cue_back` / `cue_stop` / `cue_goto` `{ cue_number }`
- `patch_set` — basic patch edit (fixture → starting DMX address, profile)
- `command_line` — `{ text: "<raw command string>" }`, echoed via `log`

Client renders optimistically on send, reconciles against the next
authoritative `state_snapshot`/`*_update` from the server (server state always
wins on conflict).

## REST endpoints (for one-shot / scriptable actions)

- `GET /api/state` — full state snapshot (same shape as `state_snapshot`)
- `POST /api/cue/go`, `/api/cue/back`, `/api/cue/stop`
- `POST /api/channel` — body `{ channel, value }`
- `POST /api/command` — body `{ text }`, raw command line passthrough

REST is a convenience layer over the same underlying command handlers the
WebSocket path uses — no separate business logic.

## Client UI — screens/panels

1. **Channel grid** — DMX channels as a scrollable grid of faders/number
   entry; tap to select, drag or +/- to adjust; multi-select for grouped
   moves
2. **Instrument control** — patched instruments by name/number; intensity
   fader, colour picker (reuse existing CIE 1931 widget if porting to
   egui/wasm), extra channel sliders (gobo, zoom, etc. — driven by fixture
   profile's channel list, not hardcoded)
3. **Patch view** — list of patched instruments with DMX start address and
   profile; basic add/edit/remove
4. **Cue list / playback** — cue list with current position highlighted,
   Go/Back/Stop controls, tap-to-jump to a specific cue
5. **Command line** — text entry + scrollback, mirrors desktop command line
   input/output over `command_line`/`log` messages

Mobile-first layout: bottom tab bar or swipe between the five panels above
(one panel visible at a time on phone-width screens), since all five won't
fit simultaneously on a phone screen the way they might on desktop.

## PWA packaging

- `manifest.json` — name, icons, `display: standalone`, theme colour
- Service worker — cache the app shell (wasm/js/html/css) for fast reload
  and basic offline resilience; state itself is never cached, always live
  from WebSocket
- Result: "Add to Home Screen" on iOS Safari and Android Chrome gives a
  full-screen, icon-launched app-like experience with zero App Store
  involvement

## Open questions for Claude Code to flag back, not silently decide

- Does EasyCue3's current engine already expose an internal command/event
  bus that the server component can hook into, or does one need to be built?
- Confirm whether the existing CIE 1931 colour picker widget has any
  non-wasm-compatible dependencies before assuming it ports directly
- Confirm tokio/async runtime is already present in EasyCue3 or needs adding
  for axum

## Suggested build order

1. Embedded axum server with `/api/state` and static file serving (no
   client yet) — prove the server runs alongside the desktop app
2. WebSocket state push (read-only remote: view channels/cues, no control)
3. Command channel: `set_channel`, `cue_go/back/stop`
4. Instrument control panel + colour picker port
5. Patch view (read-only, then edit)
6. Command line panel
7. mDNS discovery + QR pairing + PWA manifest/service worker
8. Optional PIN/token gate
