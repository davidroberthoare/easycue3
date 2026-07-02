# Release Notes

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