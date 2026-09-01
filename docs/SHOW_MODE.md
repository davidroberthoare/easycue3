# Show Mode

EasyCue3 has two operating modes, toggled from the **View → Mode** menu (the
checkbox reads "Show Mode — only GO/BACK/STOP/goto"):

- **Design Mode** (default) — the full workspace: every panel, full toolbars,
  inline label editing, drag-and-drop cue creation, command-line channel
  programming, record/delete/duplicate, masters, blackout, etc.
- **Show Mode** — operator-safe. The workspace swaps to a minimal layout and
  only transport operations are possible:
  `GO`, `BACK`, `STOP`, `goto` (including `Ctrl+G` and the on-deck box),
  cue-list scrolling / arrow navigation, and script page-turning.

The rationale: hired operators are often "clumsy" — they run cues but shouldn't
be able to accidentally rename a cue, move a script marker, change a level, or
drop a file onto the list mid-show.

## Layouts

Each mode keeps its **own independent dock layout** (egui_dock `DockState`),
persisted separately to eframe storage (`app.ron`):

| Mode | eframe key | Default layout |
| --- | --- | --- |
| Design | `dock_state` (backwards-compatible key) | The usual 4-quadrant workspace |
| Show | `show_dock_state` | Cues (left) + Script Viewer (right) |

`EasyCueApp` keeps the live workspace in `dock_state` plus the two persistent
slots `design_dock_state` / `show_dock_state`. `set_show_mode()` stashes the
current layout into the slot it belongs to, then swaps `dock_state` to the other
mode's saved layout. `save()` mirrors the active workspace back into its slot
before writing both keys, so dragging panels in one mode never clobbers the
other. The mode itself is persisted under `show_mode`.

The default Show layout has exactly two panels — Cues + Script Viewer — so a
stray click physically can't land on an editing panel. The View menu's "Add
Panel" entries still work in Show Mode if you deliberately want more panels.

## What's restricted in Show Mode

- **Cues panel**: the toolbar is replaced by a large transport row (on-deck
  number box + oversized GO / BACK / STOP buttons); the Record LX / Add Snd /
  Add Adj / duplicate / delete / masters / blackout controls are gone. Cue
  labels render as **read-only text** (no inline `TextEdit`), the right-click
  context menu is suppressed, drag-and-drop cue creation is disabled, and row
  text is bumped up a couple of points (with taller rows) for readability from
  across the room.
- **Script Viewer**: forced into Playback mode — markers are fire-only. The
  "Open PDF…" button and the Edit-mode toggle are hidden; page navigation, zoom
  and dark mode remain. No double-click-to-add, no marker drag, no delete.
- **Command line**: restricted to `go`/`goto<num>`, the `go`/`back`/`stop`
  keywords and plain `q<num>` on-deck arming. Channel levels, group commands and
  the `l`/`i` label/fade edits are rejected. The visible command-line box is
  hidden in the cues footer (Ctrl+G goto still works).
- **Menus**: Edit (re-number) and Settings (DMX / colours / fixture profiles /
  remote) are hidden. File (open/save/exit) and View (layout + mode) remain.
- **Hotkeys (Ctrl+0…9)**: deliberately **not** disabled — an operator may still
  be assigned Trigger cues to fire. Trigger mode leaves the play head untouched,
  so this is considered transport, not editing.

## On-deck visibility

Two "keep your place in the show" behaviours were added alongside:

- **Cue list auto-scroll.** Whenever the play head moves (GO / BACK / goto /
  arrows / `q<num>`), the next frame scrolls the cue table so the **on-deck row
  is centred** — keeping the just-fired (active) cue visible above it and at
  least one row below the on-deck. Implemented as a one-shot
  `UiState::pending_cue_scroll` consumed by the Cues panel via
  `TableBuilder::scroll_to_row(_, Some(Align::Center))`.
- **Script advances on fade complete.** When a cue fires, the script viewer
  still immediately brings the fired cue's marker into view. Additionally, once
  that cue's fade actually completes, the script advances to the page of the
  **on-deck** cue (the next one to fire), so the operator is always reading the
  part of the script that's coming up. Instant / audio / adjust cues advance
  right away. See `PlaybackEngine`'s `fade_seq` / `take_completed_fade()` — the
  follow-up is armed with the *specific* fade id so a blackout or a frozen fade
  resumed later can't trigger it by mistake (`UiState::script_follow_on_fade_complete`).

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| Space | GO (next cue) |
| Shift+Space | BACK (previous cue) — replaces the old `B` key, which never worked |
| S | STOP |
| Escape | Pause: freeze lighting, fade out audio |

## Files

| File | Role |
| --- | --- |
| `src/app.rs` | `show_mode`, `design_dock_state`, `show_dock_state`, `set_show_mode()`, `create_default_show_layout()`, persistence; the fade-complete script follow and cue-list scroll hooks; Shift+Space BACK |
| `src/cue/playback.rs` | `fade_seq` + `take_completed_fade()` fade-completion signal |
| `src/ui/cues.rs` | Show-mode transport row, read-only labels, hidden context menu / drag-drop / command line, `scroll_to_row` |
| `src/ui/script_viewer.rs` | Show-mode toolbar + forced Playback mode |
| `src/ui/mod.rs` | View-menu mode toggle, hidden Edit/Settings menus, show-mode command filtering |