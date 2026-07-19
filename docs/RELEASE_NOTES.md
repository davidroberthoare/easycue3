# Release Notes

## v0.4.0

- **New: Effects** — repeating waveform patterns (sine, square, sawtooth, random) applied to fixture parameters, ETC-style but kept deliberately simple.
  - Targets: intensity, hue (color-wheel rotation — a sawtooth at full size cycles the whole rainbow), saturation (fade toward white and back), pan, tilt, and position (pan+tilt circles). Hue/saturation hold brightness constant.
  - Controls: rate (Hz), size, per-fixture phase spread (offset fixtures make waves and chases), and a smoothing slider on random blending stepped flicker into smooth fire/water drift.
  - Effects modulate *relative to the base look* and ride on top of it at the DMX output stage — recording a cue never bakes effect output in, and cue tracking is unaffected.
  - Cue-triggered, tracking-style: lighting cues can start/stop effects (ramping with the cue's fade times); a running effect persists until a cue stops it, and BACK/GOTO land with the correct effects running. Cue 0 stops all effects.
  - New dockable **Effects panel** (View → Effects) to build the effect library and test on the current fixture selection; effect actions are attached to cues in Cue Properties.
  - Live FX display: modulated channels show their moving values in cyan with an "FX" tag in the Channels panel and Magic Sheet (hover for the base value); linked magic-sheet shapes animate with the effect.
  - Show files remain backward compatible — older shows load unchanged.

## v0.3.6

- Added support for generic FTDI-based USB-to-DMX512 cables (e.g. DSD TECH) via the Enttec Open DMX USB backend — these have no onboard microcontroller and speak the same host-timed DMX protocol as the genuine Enttec Open DMX USB.
- Improved port-recommendation heuristics to recognize bare FTDI chip product strings (FT232/FT231/USB Serial) so these cables surface correctly in the device picker.

## v0.3.4

- Added `goto12` / `go12` command to jump to and fire a cue by number from the command line.
- Added `q12` command to arm cue 12 as the on-deck cue without firing it; updates the yellow on-deck highlight and play-head arrow in the cue list.
- Added `Ctrl+G` goto prompt: type a cue number then Enter to fire it.
- Arrow keys (`↑`/`↓`) now navigate the on-deck cue through the list, always starting from the current on-deck position.
- Escape key now pauses playback: freezes lighting at its current state and fades out any running audio, even when a text field has focus.
- Updated in-app keyboard shortcuts help and README to document all new commands.

## v0.3.3

- Fixed Enttec Open DMX USB output on Linux by using explicit DMX serial framing and resetting FTDI control-line state on startup.
- Added persistence for the selected DMX backend so the app restores the last chosen device on launch and falls back to Virtual DMX if it is unavailable.
- Updated documentation to reflect Open DMX USB support and Linux serial-access requirements.