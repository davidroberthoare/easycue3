# EasyCue3 Work Plan

Generated 2026-05-03. Ordered by phase; build-test + git commit after each numbered item.

---

## Phase 1 — Bug Fixes (do first, low risk, high value)

### 1. Virtual Intensity not updating during cue playback
**Root cause:** `channels.rs` and `properties.rs` call `get_intensity()` (stored state) before
falling back to `calculate_intensity()` (live from universe). During cue fades, `playback.update()`
writes interpolated values to the DMX universe but never calls `update_from_universe()` to sync the
stored VirtualIntensity state. So `get_intensity()` returns stale pre-cue values for the whole fade.

**Fix:** In `src/app.rs`, after `self.playback.update(universe)` in the frame update loop (~line 883),
iterate all patched RGB-only fixtures and call `virtual_intensity.update_from_universe()` for each.
Only call it when a fade is active (state is `Fading`) to avoid unnecessary work every frame at idle.

**Files:** `src/app.rs`, possibly `src/fixtures/intensity.rs` (check signature of `update_from_universe`)

---

### 2. Icons not showing (cue list, info column, enter button)
**Root cause:** No emoji/icon font is loaded. Linux egui default fonts don't include emoji glyphs;
characters like 💡 🎚 🔊 ⏱ render as blank squares.

**Fix:** Add the `egui-phosphor` crate (Phosphor icon set as a font with Rust constants).
- Add to `Cargo.toml`: `egui-phosphor = "0.7"` (check latest compatible with egui 0.31)
- In app setup (`src/app.rs` or `main.rs`), load the phosphor font via `egui_phosphor::setup_fonts`
- Replace emoji literals in `src/ui/cues.rs` with Phosphor constants, e.g.:
  - `"💡"` → `egui_phosphor::regular::LIGHTBULB` (lighting cue icon)
  - `"🎚"` → `egui_phosphor::regular::SLIDERS` (adjust cue icon)
  - `"🔊"` → `egui_phosphor::regular::SPEAKER_HIGH` (audio cue icon)
  - Seconds icon in info column: `egui_phosphor::regular::CLOCK`
  - Enter button: `egui_phosphor::regular::ARROW_BEND_DOWN_LEFT` (or similar return symbol)

**Files:** `Cargo.toml`, `src/main.rs` or `src/app.rs` (font setup), `src/ui/cues.rs`

---

### 3. Ondeck cue: icon not displaying + unique colour
**Current:** `COLOR_NEXT` is `(30, 50, 80)` — nearly invisible dark blue. The ondeck highlight
applies to the correct next cue in the list but the colour blends with the background.

**Fix:**
- Change `COLOR_NEXT` to something distinctly visible and different from active/selected, e.g.
  a warm amber/gold: `Color32::from_rgb(160, 120, 20)`.
- If the ondeck cell in the cue toolbar (line 40-46) is meant to show an icon alongside the
  command textbox, add a Phosphor icon label there using the icon from item 2 above.

**Files:** `src/ui/cues.rs`

---

### 4. Go / goto_cue should deselect selected cue
**Fix:** In `src/app.rs`, in both `go_next()` and `go_to_cue()`, add:
```rust
self.ui_state.selected_cue_id = None;
```
after the cue fires.

**Files:** `src/app.rs`

---

## Phase 2 — Cue List Enhancements

### 5. Different colours for lighting vs audio cues in list
**Fix:** Add two new base row colours (non-state colours that show when a cue is idle and
unselected). Currently all idle rows use a neutral background.

Suggested: lighting cues get a faint blue tint, audio cues a faint green tint.

Apply in `src/ui/cues.rs` row rendering logic alongside the existing state-colour logic —
state colours (active, fading, selected) should override the type colour.

**Files:** `src/ui/cues.rs`

---

### 6. Cue number editable; remove move up/down buttons; auto-resort
**Current:** Move-up/down buttons at `src/ui/cues.rs:156-163`. Cue numbers are display-only.

**Changes:**
- Remove `can_move_up`, `can_move_down`, the ↑/↓ buttons, and the `move_up`/`move_down` calls
  from `src/ui/cues.rs`.
- In cue properties panel (`src/ui/properties.rs`), make the cue number field an editable
  `DragValue` or text input (currently read-only).
- On commit (focus lost / Enter), call a new `CueList::renumber_cue(id, new_number)` method in
  `src/cue/list.rs` that:
  1. Checks the new number is not already taken by another cue (return Err if overlap).
  2. Sets the cue's `number` field.
  3. Re-sorts the `cues` Vec by number (using the existing binary-search insertion logic or a
     simple `sort_by`).
  4. Updates `current` index to point to the correct cue after re-sort.
- Show an inline error in properties if the number is a duplicate.

**Files:** `src/ui/cues.rs`, `src/ui/properties.rs`, `src/cue/list.rs`

---

### 7. Linked cues (cross-triggers): dropdown showing cue number+name, storing stable ID
**Current state:**
- `LightingData.triggers_audio_cue: Option<f32>` — stores audio cue number
- `AudioData.triggers_lighting_cue: Option<f32>` — stores lighting cue number
Both use cue numbers (floats), not stable IDs.

**Changes:**
- In `src/cue/types.rs`, rename/change both fields to store `Option<u32>` (the cue's stable `id`):
  - `triggers_audio_cue_id: Option<u32>`
  - `triggers_lighting_cue_id: Option<u32>`
- Update show file loading: add a migration step in `src/show/mod.rs` (or wherever JSON is loaded)
  that converts old float-number references to IDs by looking up the cue by number. Fallback to
  `None` if the referenced cue can't be found (print a warning).
- In `src/ui/properties.rs`, replace the current text/number input for the cross-trigger with a
  `ComboBox`:
  - For a lighting cue: dropdown lists all audio cues as "Cue {number} — {name}"; stores selected
    cue's `id`.
  - For an audio cue: dropdown lists all lighting cues the same way.
  - First option in dropdown: "(none)" → `None`.
  - Exclude the current cue from the list (can't trigger itself; also not meaningful across types
    but guard it anyway).
- Update all call sites that resolve the trigger (in `src/app.rs` cross-trigger logic) to look up
  by ID using `cue_list.find_by_id()` instead of searching by number.

**Files:** `src/cue/types.rs`, `src/ui/properties.rs`, `src/app.rs`, `src/show/` (migration)

---

## Phase 3 — Patch Panel

### 8. Add multiple fixtures at once; starting fixture number pre-populated; label optional
**Changes to add-fixture dialog (`src/ui/patching.rs`):**
- Add a **Quantity** spinner (default 1, range 1–50).
- Add a **Starting fixture number** field, pre-populated with `next_available_fixture_id()` — a
  new helper on `PatchList` (or similar) that returns the lowest unused integer ID ≥ 1.
- Make **Label** optional — empty string is valid. When quantity > 1 and label is non-empty,
  auto-append a numeric suffix (e.g. "Fresnel" → "Fresnel 1", "Fresnel 2", …).
- When quantity > 1, also auto-increment DMX start address by the profile's channel count per
  fixture (show the calculated end address so user can see if it overflows 512).
- Validation: check each address slot + fixture ID for overlap before adding any; show a clear
  error if any would conflict.

**Files:** `src/ui/patching.rs`, `src/fixtures/patching.rs` (add `next_available_id` helper)

---

### 9. Fixture ID editable after creation (overlap checking)
**Current:** Fixture `id: usize` is auto-assigned and read-only in the UI.

**Fix:** In the fixture list in `src/ui/patching.rs`, render the fixture ID as an editable
`DragValue` (integer) instead of a label. On change:
1. Check the new value is a positive integer not already used by another patch.
2. If valid, update `patch.id` in `PatchList`.
3. If invalid, revert and show a tooltip/error message.

Note: `id` is also used as the key in `VirtualIntensity`'s HashMap. Update all `virtual_intensity`
state references after an ID change (simplest: clear that fixture's entry so it's recalculated on
next frame).

**Files:** `src/ui/patching.rs`, `src/fixtures/patching.rs`, `src/fixtures/intensity.rs`

---

### 10. Click background of fixture list deselects all; remove "clear selection" button
**Fix in `src/ui/channels.rs`:**
- Remove the "clear selection" button.
- After rendering all fixture tiles, check if the user clicked anywhere in the list panel that
  wasn't consumed by a tile (use `ui.interact(ui.max_rect(), …).clicked()` with `sense: Sense::click()`
  but only fire if no tile response was clicked — egui `Response::clicked()` is exclusive per frame).
- On background click, clear `app.ui_state.selected_fixtures` (or equivalent selection set).

**Files:** `src/ui/channels.rs`

---

## Notes

- **egui version:** pinned to 0.31 — confirm `egui-phosphor` 0.7.x is compatible before adding.
  Check: `cargo add egui-phosphor` and verify it doesn't pull a different egui version.
- **Show file migration (item 7):** the trigger-field rename is a breaking change to the JSON
  show format. Increment the show file version field if one exists, and handle old files gracefully.
- **Virtual intensity + fixture ID rename (items 1 + 9):** if a fixture's ID is edited, the
  `VirtualIntensity` HashMap entry needs to be re-keyed. Simplest: remove old key, let it
  reinitialise from universe on next frame.
- **No test suite currently** — after each phase, do a manual smoke test: open a show file,
  run a lighting cue with an RGB fixture, verify intensity updates, verify cue list colours and
  icons, verify patch add/edit.
