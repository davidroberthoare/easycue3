# Phase 4 Improvements - Audio Volume & Inline Editing

## Changes Made

### 1. Fixed Audio Volume Control
**Problem**: Volume slider in Sound Cues panel snapped between full/0, and volume wasn't controlled by Sound Master.

**Solution**:
- Removed per-cue volume slider from Sound Cues panel
- Audio volume now controlled by **Sound Master** slider in Controls panel
- Calculation: `effective_volume = cue_volume × fade × sound_master`
  - `cue_volume`: Per-cue volume (0-100%, editable in table)
  - `fade`: Fade in/out progress (0.0-1.0)
  - `sound_master`: Global audio level (0-100%, in Controls panel)

**Files Modified**:
- `src/app.rs`: Added sound_master application in update loop
- `src/audio/playback.rs`: Added comments and `current_base_volume()` method
- `src/ui/sound_cues.rs`: Removed volume slider, added effective volume display

**How It Works**:
1. AudioPlaybackEngine calculates base volume (cue volume × fade)
2. App update loop applies sound_master: `base × sound_master`
3. Real-time control: Adjusting Sound Master immediately affects playing audio

---

### 2. Inline Editing for Audio Cues
**Problem**: No way to edit audio cue properties after creation.

**Solution**: Implemented inline editing using egui's built-in widgets (no new dependencies).

**Editable Fields**:
- **Label**: TextEdit (click to edit name)
- **Fade In**: DragValue (0-30s, drag or type)
- **Fade Out**: DragValue (0-30s, drag or type)
- **Volume %**: DragValue (0-100%, per-cue base volume)
- **→ Light**: Checkbox + DragValue (trigger lighting cue number)

**UI Changes**:
- Table columns reorganized: Cue | Label | File | Fade In | Fade Out | Vol % | → Light | Delete
- Click any cell to select and edit
- Changes save immediately (no "Apply" button needed)
- Selected row tracked in `app.ui_state.selected_audio_cue_index`

**Files Modified**:
- `src/app.rs`: Added `selected_audio_cue_index: Option<usize>` to UiState
- `src/ui/sound_cues.rs`: Replaced read-only table with inline editable widgets

**How It Works**:
1. Table iterates by index (not by cloned cues)
2. Each cell reads immutable data, renders widget with value
3. On change, gets mutable reference and updates cue directly
4. Selection tracked on click for potential future use

---

### 3. Status Display Improvements
- Removed individual volume slider (was redundant with per-cue volume + sound master)
- Added "Output: X%" display showing effective volume
- Added hint text: "Click cells to edit • Volume controlled by Sound Master"

---

## Testing

**Test Volume Control**:
1. Add audio cue with Volume % = 80
2. Set Sound Master = 50%
3. Play cue → Should hear at 40% (80% × 50%)
4. Adjust Sound Master while playing → Volume changes immediately

**Test Inline Editing**:
1. Add audio cue
2. Click label cell → Edit name
3. Drag Fade In value → Changes immediately
4. Check → Light checkbox → Enable trigger
5. Drag trigger value → Set lighting cue number
6. Close app, reopen → Edits persist

**Test Cross-Triggering**:
1. Create Audio Cue 1.0, enable trigger → 2.0
2. Press GO on audio → Should auto-trigger Lighting Cue 2.0

---

## Architecture Notes

**Why Not egui-data-table?**
- Would add ~15KB dependency
- Built-in egui widgets provide sufficient functionality
- More control over behavior and styling
- Easier to maintain (no external API changes)

**Volume Flow**:
```
AudioCue.volume (0.0-1.0)
    ↓ [per-cue setting]
AudioPlaybackEngine.base_volume
    ↓ [fade calculations]
AudioPlayer.volume (base with fades)
    ↓ [app.rs applies sound_master]
Rodio Sink (final output)
```

**Future Enhancements**:
- Multi-select for bulk editing
- Copy/paste cue properties
- Keyboard shortcuts for editing (Enter to edit, Esc to cancel)
- Undo/redo for edits
