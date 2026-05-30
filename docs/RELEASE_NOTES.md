# Release Notes

## v0.3.3

- Fixed Enttec Open DMX USB output on Linux by using explicit DMX serial framing and resetting FTDI control-line state on startup.
- Added persistence for the selected DMX backend so the app restores the last chosen device on launch and falls back to Virtual DMX if it is unavailable.
- Updated documentation to reflect Open DMX USB support and Linux serial-access requirements.