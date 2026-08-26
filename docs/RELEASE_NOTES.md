# Release Notes

## v0.8.7

- **Audio cues: start time, looping, and a live progress readout.**
  - **Start Time** — audio cues can begin partway through the file (seconds from
    the start; defaults to 0.0). `Length` counts from the start time, so a 30s
    file starting at 10s can play up to 20s of material.
  - **Loop** — audio cues can loop: the file restarts when it reaches the end
    and keeps playing until you stop it or its length expires.
  - **Progress in the cue list** — while an audio cue plays, its Info column
    turns into a live countdown (remaining seconds) with a left-to-right
    progress fill; cues without a length show elapsed time instead. Looping
    cues show a ↻ icon.
- **Right-click = toggle.** In the fixture list and the magic sheet, right-click
  now does exactly what Ctrl/Cmd+click does — toggling the fixture, channel, or
  shape's membership in the selection — for faster multi-picking without the
  keyboard.
- **Duplicate cue.** A new copy button in the Cue list toolbar (before Delete)
  duplicates the selected cue right where you're working: the copy is numbered
  between the original and the next cue (or the next whole number at the end of
  the list) and is selected ready to edit.

## v0.8.6

- **Lighter on older machines.** The UI now animates at ~30fps instead of 60 —
  the console has no fast animation (just sliders, PDF reading and colour
  pickers), and ~30fps looks smooth while using a fraction of the CPU. During
  a fade, CPU load drops from ~90% to ~30–40% on older hardware, and the UI
  still looks smooth. DMX output is unaffected: the USB Pro / Open DMX /
  Art-Net senders keep running on their own threads at 40/30/40Hz.
  Override with the `EASYCUE_REPAINT_MS` environment variable if you want
  different pacing (e.g. `20` for 60fps).
- **Trustworthy debug overlay.** The Debug Info window now reports the *actual*
  repaint rate over the last second (plus the target interval and both egui
  timesteps) instead of a fixed `1/60` prediction — so you can see the app
  genuinely idle or hit its target rate, rather than always showing 60fps.

## v0.8.5

- **Faster, steadier rendering (wgpu).** EasyCue3 now renders through wgpu —
  Vulkan on Linux, Metal on macOS — instead of OpenGL, roughly halving typical
  frame times.
- **No more 30–40fps hitches.** Animation now paces cleanly with your display
  (a solid 60fps on a 60Hz screen) instead of stuttering every few frames. The
  cause was the app racing ahead of the swapchain — fixed by a repaint cadence
  tuned just above one refresh period.
- **Idles when nothing is moving.** During a show the app only redraws while
  something is actually animating (a fade, an effect, audio). When a cue is
  holding levels, it stops rendering entirely, saving CPU and battery.

## v0.8.3

- **Custom fixture profiles can now be moved between machines.** Your own fixture profiles live in the app's config folder (not in the show file), so a show created on one computer wouldn't patch on another. The **Custom Fixture Profiles** window now has:
  - **Export…** — save the selected profile as a JSON file (name defaults to `<id>.json`) to share or back up.
  - **Import…** — pick one or more profile JSON files to bring into the app. Files are validated on import; invalid ones are reported and skipped.
  - If an imported profile matches one that's already installed (by ID or by name), you're asked to **Replace** or **Skip** — with "Replace all remaining" / "Skip all remaining" when importing a batch. Replacing a bundled profile supersedes it with your copy.

## v0.8.2

- **New: Absolute cues.** Lighting cues have an **Absolute** checkbox in Cue Properties. Ordinary cues are *tracking* — they store only the channels that changed, and anything they don't mention holds. An absolute cue instead stores a **full snapshot** of the output state: every patched channel across all universes, exactly as the board was outputting it.
  - **Backward navigation stops hunting.** When you BACK or GOTO, the console reconstructs the target state by replaying cues from the top of the list. With absolute cues in place, that replay now **stops at the closest absolute cue** and works forward from its snapshot — so a cue way down the list no longer depends on every cue before it being right.
  - **Playback reproduces the snapshot.** Firing an absolute cue fades *everything* (channels not in the cue go to 0), so it lands on precisely what was captured. Effects reset at the same boundary.
  - **Convert after the fact.** Flipping a tracking cue to absolute expands it to the full tracked state automatically; flipping it back collapses it to just the changes. **Update from Stage** captures the full state on an absolute cue, or just the deltas on a tracking cue.
  - Absolute cues show a distinct **amber filament-lightbulb** icon in the cue list.
  - Only patched channels are snapshotted, so show files stay compact. See `docs/ABSOLUTE_CUES.md`.

## v0.8.1

- **Remote control — Groups & Fixtures parity.** New **Groups** tab: tap a group to recall its fixtures, then drive them with level buttons + a live slider, or paint a colour across the RGB fixtures. The **Fixtures** tab gained a Select mode (multi-pick fixtures with the same level slider) so the two pages work the same way.
- **Remote control — colour matching the console.** The phone's colour picker is now the same circular HSV wheel as the desktop (hue by angle, saturation by radius, value 1), with RGB sliders; the fixture sheet also shows a live swatch on the RGB colour button. Fixture sheets order intensity first, then colour, then the other parameters.
- **Remote control — more of the console on the phone.** Channel/fixture readouts now mirror effect-modulated values while an FX runs; a sound-master fader and audio cue status line joined the Cues page; patch edit lets you change a fixture's profile (validated with rollback); level buttons read Out → Full left-to-right everywhere.
- **Magic Sheet — autonumbering.** New "Auto #" tool in the toolbar: choose **Fixtures** or **Groups**, watch the *next* number and its name in the toolbar, and click shapes to assign them in sequence. The counter stops at "done" once the last number is placed.
- **Naming tidy-up** — the fixture-centric tab is now labelled **Fixtures** (was "Channels") and the properties panel **Fixture Properties** (was "Instrument Properties").
- **Remote control — IP-only pairing.** Dropped mDNS (`easycue3.local`) and the service worker: pairing is by the console's local IP address (QR code in Settings → Remote Control). Simpler, and reliable on any venue wifi.

## v0.8.0

- **New: Cue hotkeys (Ctrl+0…Ctrl+9)** — trigger cues from the keyboard when running a show. Assign any existing lighting, sound, or adjust cue in the new **Hotkeys** panel (View → Hotkeys):
  - **Trigger** — the cue runs exactly as if you'd pressed GO (fade timing respected), but the on-deck/play-head cue is left untouched and autofollows aren't armed.
  - **Hold** — the cue plays for as long as the key is held down, then fades back out on release using the cue's fade up/down times (lighting returns to where the stage was before you pressed; audio fades out with its `fade_out`).
  - **Latch** — like Hold, but you don't have to keep the key held: first press starts the cue, second press stops it.
  - Assignments are saved in the show file (older show files load unchanged; an empty hotkey map is omitted on save), and the default show ships with a few working examples. Holds are robust against key auto-repeat and releases that happen while a text field is focused.
- **Smarter cue numbering when inserting** — new cues (LX/Snd/Adj, drag-and-drop, and script-viewer creates) now appear next to where you're working instead of at the end of the list:
  - In the Cue list, the number is computed from the *selected* cue (falling back to the active cue) and the cue after it — preferring an available whole number, then the decimal midpoint: between 3 and 6 you get 4, then 4.5, 4.25…; between 3 and 4 you get 3.5, 3.25…
  - In the Script Viewer, double-clicking between two markers numbers the new cue halfway between those two cues (again preferring a whole number).
  - Recording now baselines cue tracking at the insertion point rather than the end of the list.

## v0.7.2

- **Sleep/resume & device-loss recovery** — EasyCue3 now survives the machine going to sleep and hot-unplug/re-plug of audio and DMX hardware without a restart:
  - **Audio survives sleep.** If an output device's stream dies on resume (ALSA `POLLERR` and friends), the app re-opens it automatically and re-attaches any playing cues at their current position — volume, pan and in-progress fades are preserved. Recovery retries with backoff (2s→5s→15s→60s) so a genuinely-removed device goes quiet instead of spamming the log, and it logs through the app's own logger rather than raw stderr.
  - **USB DMX reconnects.** If an Enttec USB Pro / Open DMX device is lost (unplug, cable bump, sleep), the console falls back to Virtual (so the show keeps "outputting") while retrying the hardware in the background with backoff; when the device reappears it's swapped back in automatically — no more permanent Virtual until you restart. A "… reconnecting" indicator appears next to the DMX status. Manually choosing a backend cancels the retry.
- **Settings-file corruption guard.** A crash or Ctrl+C that killed eframe's background settings write used to leave `app.ron` truncated, silently resetting your UI layout and last-loaded show on the next launch. The app now detects a corrupt settings file at startup, backs it up as `app.ron.corrupt-<timestamp>`, and starts clean.
- **Script Viewer — dark mode.** New toggle (also remembered across launches) inverts the PDF page rendering for reading scripts in dark rooms; cue markers and the add-cue popup pick up the theme too.
- **Script Viewer — page navigation box.** Type a page number directly and press Enter to jump straight to it (alongside the existing ←/→ and PageUp/PageDown navigation). The page indicator doubles as the input field.
- **Script Viewer stability fixes**: page content stays put on screen when the marker-editor strip appears/disappears above the canvas (no more nudging a just-placed marker), the wheel no longer pans/zooms the PDF while a dropdown or the add-cue popup is open, and loading a new show resets pan while keeping your zoom.
- Various cue-list, fixture-editor and properties-panel tweaks.

## v0.7.1

- **Script Viewer quality-of-life**:
  - Double-clicking a page to add a **Lighting / Sound / Adjust** cue now selects the new cue and focuses its Label field in Cue Properties, so you can name it immediately.
  - The add-cue popup remembers your last action. If you last created a cue it re-selects that kind and focuses **Create & link** (just press Enter); if you last linked an existing cue, the cue picker keeps your last choice and is focused ready to use.
- **New: Edit → Re-number Cues…** — renumber a whole batch of cues in one go. Choose **all** cues or a `#x–#y` range, the new start number, and the step (default 1.0), then Apply. Adjustment-cue targets are re-linked to their renumbered audio cues and script markers (which reference cues by stable ID) track the new numbers automatically.

## v0.7.0

- **New: Script Viewer** — run the show straight off your PDF script. Load the script (View → Script Viewer), click anywhere on a page to drop a *cue marker* linked to a cue, and click markers during the show to fire their cues.
  - **Selection is synced across all three views**: clicking a marker selects its cue in the Cue list and Cue Properties; selecting a cue from the list (row click, context menu, or ↑/↓ arrows) highlights the matching marker and brings it into view if it isn't already on screen.
  - Markers sit on the exact script line in PDF point space, persist in the show file, and keep working even if the cue is renamed — they reference cues by stable ID.
  - Page navigation with `←`/`→` or `PageUp`/`PageDown`; scroll to zoom, drag to pan, with automatic re-rasterization when you zoom in so text stays sharp.
  - **Nothing to install**: the PDFium renderer is bundled in a `lib/` subdirectory of every download (the library is loaded at runtime, so Linux/macOS users need no system package and no `PDFIUM_LIBRARY_PATH`).
- **Release packaging** now ships a bundled PDFium library (`libpdfium.so` / `pdfium.dll` / `libpdfium.dylib`) in the `lib/` folder of each platform package, keeping the bundle root tidy. Script viewing works out of the box on every supported platform.

## v0.6.1

- **New: Check for Updates** — Help menu now has a "Check for Updates" item that looks up the latest GitHub release and tells you if a newer version is available, with a link to the release page. Notify-only: nothing downloads or installs itself.
  - Also checks automatically in the background at most once per day on launch; if a newer version is found, a small "Update Available" badge appears in the menu bar (click it to see details) — otherwise it stays out of the way.

## v0.6.0

- **New: Native multi-channel audio output** — route audio cues to any stereo pair of a multi-channel interface (e.g. a USB audio interface with separate front and rear outputs), not just the first pair.
  - Devices that report more than two channels are opened at full width, and each stereo pair shows up as its own entry in the output picker ("Interface · Out 1-2", "Interface · Out 3-4", ...) alongside plain stereo devices.
  - Cues can play on any pair, on several pairs at once, and an Adjust cue can crossfade between pairs exactly like crossfading between separate devices — including fading onto a pair the Play cue never routed to, joining in sync from silence.
  - The routing happens in the app itself, so it works the same on Windows, macOS, and Linux — no OS-level channel-mapping configuration needed.
  - Plain stereo devices and the default output are unaffected: the picker looks and behaves exactly as before if you never touch a multi-channel interface.
  - Show files remain backward compatible — routes and fades from older shows load and re-save unchanged.
  - Linux/PipeWire users routing to a secondary device (built on the v0.5.0 groundwork) should pin `channels N` on the device's `~/.asoundrc` alias so EasyCue3 knows its true channel count; see `docs/AUDIO_DEVICES.md`.

## v0.5.0

- **New: Remote control** — run EasyCue3 from any phone or tablet browser on the venue wifi, no app to install.
  - Embedded web server (off by default; enable in Settings → Remote Control...), serving a Framework7-based PWA over WebSocket + REST.
  - Five panels: Cues (GO/BACK/STOP, double-tap a cue to jump to it, grand master, blackout), Fixtures (intensity — virtual for RGB-only fixtures — color picker, profile-driven sliders), Channels (512-channel grid across active universes), Patch (add/edit/renumber/re-address/delete fixtures, with the same overlap validation as the desktop patch list), and a command line mirroring the desktop's EOS-style syntax.
  - QR code pairing using the console's local IP address; optional PIN to keep other people on the venue wifi out.
  - The desktop app remains the sole owner of engine state — the server only relays commands and state diffs, so recording, tracking, and playback behave identically whether driven from the console or a phone.
  - "Add to Home Screen" on iOS/Android gives a full-screen, icon-launched experience.

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
