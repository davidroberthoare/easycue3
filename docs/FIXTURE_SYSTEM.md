# Fixture Profile & Patching System

**Implementation Date:** April 2026  
**Phase:** 1-4 (Foundation)  
**Status:** ✅ Complete - Ready for Testing

---

## Overview

EasyCue3 now supports **fixture-based lighting control** in addition to raw DMX channels. This allows users to control intelligent lighting fixtures (RGB LEDs, moving lights, etc.) using theater-friendly parameters like color pickers and intensity sliders instead of manually managing individual DMX channels.

### Key Concepts

- **Fixture Profile**: A JSON template defining a fixture type's capabilities (RGB, intensity, pan/tilt, etc.)
- **Patching**: Mapping a fixture instance to specific DMX addresses (e.g., "RGB #1" starts at channel 10)
- **Parameter-Based Control**: Control fixtures via color pickers and sliders that automatically update the correct DMX channels

---

## What Was Implemented

### Phase 1: Fixture Profile System

**Files Created:**
- `src/fixtures/profiles.rs` - Core data structures
- `src/fixtures/mod.rs` - FixtureLibrary manager
- `fixture_profiles/*.json` - 13 default fixture definitions

**Capabilities:**
- **17 Parameter Types**: Intensity, Red, Green, Blue, Amber, White, UV, Strobe, Iris, Gobo, Pan, PanFine, Tilt, TiltFine, Focus, Zoom, Prism, Frost, Custom
- **JSON-Based Profiles**: Simple format for defining fixture capabilities
- **Cross-Platform Loading**: Profiles load from:
  - Bundled: `fixture_profiles/` (ships with app)
  - User: `~/.config/easycue3/fixture_profiles/` (Linux/macOS) or `%APPDATA%\easycue3\fixture_profiles\` (Windows)
- **Validation**: Ensures channel counts match parameter definitions

**Default Profiles:**
1. `dimmer.json` - Single channel intensity (1ch)
2. `rgb.json` - Red, Green, Blue (3ch)
3. `rgba.json` - RGB + Amber (4ch)
4. `rgbw.json` - RGB + White (4ch)
5. `rgbaw.json` - RGB + Amber + White (5ch)
6. `rgbawuv.json` - RGB + Amber + White + UV (7ch - note: 6 colors + intensity)
7. `irgb.json` - Intensity + RGB (4ch)
8. `irgba.json` - Intensity + RGBA (5ch)
9. `irgbw.json` - Intensity + RGBW (5ch)
10. `irgbaw.json` - Intensity + RGBAW (6ch)
11. `irgbawuv.json` - Intensity + RGBAWUV (7ch)
12. `led_par.json` - Professional LED PAR with dimmer + strobe (7ch)
13. `moving_head.json` - Full moving light with pan/tilt/gobo/iris/focus/zoom/prism/strobe (16ch)

### Phase 2: Patching System

**Files Created:**
- `src/fixtures/patching.rs` - Patch data structures

**Files Modified:**
- `src/show/mod.rs` - Added patch persistence to show files
- `src/app.rs` - Integrated patch loading/saving
- `Cargo.toml` - Added `dirs = "5.0"` dependency

**Capabilities:**
- **Patch Definition**: ID, label, profile type, start address, universe, notes
- **Address Validation**: Prevents overlapping fixture assignments
- **Channel Range Calculation**: Automatically computes occupied DMX channels
- **Show Integration**: Patches save/load with cues in `.json` show files
- **Backwards Compatible**: Old show files without patches still load correctly

### Phase 3: Patching UI

**Files Created:**
- `src/ui/patching.rs` - Interactive patching panel

**Files Modified:**
- `src/ui/mod.rs` - Exported patching module
- `src/app.rs` - Added Patching tab to dock layout

**Capabilities:**
- **Fixture Table**: Displays all patched fixtures with ID, label, type, address range, channel count
- **Add Fixture**: Dialog with profile selector, label input, address picker
- **Delete Fixture**: Remove patches with confirmation
- **Edit Fixture**: Dialog structure (backend incomplete - returns "Not implemented")
- **Visual Feedback**: Shows address overlaps and validation errors

**UI Location:**
- New "Patching" tab in dock layout (bottom row by default)

### Phase 4: Fixture-Aware Properties Panel

**Files Modified:**
- `src/ui/properties.rs` - Extended with fixture detection and parameter controls

**Capabilities:**
- **Automatic Fixture Detection**: When clicking a channel, checks if it belongs to a patched fixture
- **Fixture Controls** (when fixture detected):
  - Fixture name and profile type display
  - **Color Picker**: HSV color wheel for RGB/RGBA/RGBW/etc. fixtures (updates all color channels simultaneously)
  - **Intensity Master**: Slider for fixtures with separate intensity channel (iRGB variants)
  - **Individual Color Sliders**: Fine control for Red, Green, Blue, Amber, White, UV
  - **All Parameters View**: Collapsible grid showing every fixture parameter
- **Fallback**: Shows raw DMX channel values for unpatched channels
- **Multi-Channel Selection**: Detects if all selected channels belong to same fixture

**Color Picker Details:**
- Uses `egui::color_edit_button_srgba()` - rectangular HSV/RGB picker
- Real-time DMX updates as color changes
- Converts sRGB (0.0-1.0) ↔ DMX (0-255) automatically
- Works with any RGB-capable fixture (RGB, RGBA, RGBW, RGBAW, RGBAWUV, iRGB variants)

---

## Architecture

### Data Flow

```
User Clicks Channel 10
    ↓
Properties Panel Checks: Is channel 10 part of a patched fixture?
    ↓
FixtureLibrary.find_patch_at_channel(10) → Returns Patch ("RGB #1", starts at 10, 3 channels)
    ↓
Load Profile: FixtureLibrary.get_profile("rgb") → Returns FixtureProfile (3ch: R, G, B)
    ↓
Render Fixture Controls: Color picker, RGB sliders
    ↓
User Drags Color Picker to Red (255, 0, 0)
    ↓
Update DMX: Channel 10=255, Channel 11=0, Channel 12=0
    ↓
Universe propagates to DMX output (40 Hz)
```

### Module Structure

```
src/fixtures/
├── mod.rs              # FixtureLibrary (profile + patch manager)
├── profiles.rs         # FixtureProfile, FixtureParameter enums
└── patching.rs         # Patch, PatchList (address management)

src/ui/
├── patching.rs         # Patching panel (add/edit/delete fixtures)
└── properties.rs       # Properties panel (fixture controls + color picker)

fixture_profiles/       # Bundled JSON profiles (13 files)
```

### Key Types

```rust
// Parameter definition
pub enum FixtureParameter {
    Intensity, Red, Green, Blue, Amber, White, Uv,
    Strobe, Iris, Gobo, Pan, PanFine, Tilt, TiltFine,
    Focus, Zoom, Prism, Frost, Custom(String)
}

// Fixture type definition
pub struct FixtureProfile {
    pub id: String,                // "rgb", "moving_head"
    pub manufacturer: String,       // "Generic"
    pub name: String,              // "RGB LED"
    pub channel_count: usize,      // 3
    pub parameters: Vec<ParameterMapping>  // [(Red, ch 1), (Green, ch 2), ...]
}

// Fixture instance
pub struct Patch {
    pub id: usize,                 // Unique ID
    pub label: String,             // "Stage Left #1"
    pub profile_id: String,        // "rgb"
    pub start_address: u16,        // 10 (DMX channel)
    pub universe: u16,             // 1
    pub notes: String              // Optional description
}
```

---

## Usage

### Creating Custom Fixture Profiles

1. Create a JSON file in `~/.config/easycue3/fixture_profiles/` (or OS equivalent)
2. Follow this template:

```json
{
  "id": "my_fixture",
  "manufacturer": "MyBrand",
  "name": "Custom RGB",
  "channel_count": 4,
  "parameters": [
    {
      "parameter": "Intensity",
      "channel_offset": 1
    },
    {
      "parameter": "Red",
      "channel_offset": 2
    },
    {
      "parameter": "Green",
      "channel_offset": 3
    },
    {
      "parameter": "Blue",
      "channel_offset": 4
    }
  ]
}
```

3. Restart EasyCue3 - profile appears in Patching panel dropdown

### Patching a Fixture

1. Open **Patching** tab in dock layout
2. Click **Add Fixture**
3. Select profile type (e.g., "RGB LED")
4. Enter label (e.g., "Stage Left #1")
5. Set start address (e.g., 10 for channels 10-12)
6. Click **Add** - fixture appears in table

### Controlling a Patched Fixture

1. Open **Channels** panel
2. Click a channel that belongs to a fixture (e.g., channel 10 of RGB at 10-12)
3. **Properties** panel now shows:
   - Fixture name: "Stage Left #1 (RGB LED)"
   - Color picker button - click to open HSV/RGB color selector
   - Individual sliders: Red, Green, Blue (channels 10, 11, 12)
4. Drag color picker or sliders - DMX updates in real-time
5. Record cue as normal - stores DMX values (not fixture parameters)

### Show File Persistence

Patches save automatically with show files:

```json
{
  "name": "My Show",
  "created": "2026-04-29T12:00:00Z",
  "cues": [...],
  "patch": [
    {
      "id": 1,
      "label": "Stage Left #1",
      "profile_id": "rgb",
      "start_address": 10,
      "universe": 1,
      "notes": ""
    }
  ]
}
```

---

## Design Decisions

### Why Store DMX in Cues (Not Fixture Parameters)?

**Chosen Approach:** Cues store raw DMX channel values (1-512 = 0-255)

**Rationale:**
- **Simplicity**: No tracking system needed when fixtures are patched/unpatched/edited
- **Flexibility**: Cues work even if fixture profiles change
- **Industry Practice**: EOS-style consoles store absolute DMX for reliability
- **Beginner-Friendly**: Easier mental model (cue = snapshot of DMX state)

**Trade-off:** Cues don't track "this was RGB(255, 0, 0)" - they track "channels 10=255, 11=0, 12=0". If you re-patch the fixture to different addresses, old cues won't follow it.

### Why Rectangular Color Picker?

**Current:** egui's built-in `color_edit_button_srgba()` (rectangular HSV/RGB picker)

**Future:** Circular color wheel matching theatrical consoles (QLab, EOS)

**Rationale:**
- MVP speed - egui includes this widget
- Good enough for basic color mixing
- Circular wheel requires custom egui implementation (~100+ lines)

### Why Cross-Platform Config Directories?

**Locations:**
- Linux: `~/.config/easycue3/fixture_profiles/`
- macOS: `~/Library/Application Support/easycue3/fixture_profiles/`
- Windows: `%APPDATA%\easycue3\fixture_profiles\`

**Rationale:**
- Standard practice for user-modifiable app data
- Profiles survive app updates
- Users can share custom profiles between shows
- Bundled profiles remain read-only

---

## Technical Notes

### Borrow Checker Challenges

The properties panel needed careful data collection to avoid simultaneous immutable (reading fixtures) and mutable (updating universe) borrows:

```rust
// ❌ FAILS - tries to borrow fixtures immutably while app is mutably borrowed
if let Some(patch) = app.fixtures.find_patch_at_channel(ch) {
    if let Some(profile) = app.fixtures.get_profile(&patch.profile_id) {
        render_fixture_properties(ui, app, ...);  // app mutably borrowed here!
    }
}

// ✅ WORKS - collect data before mutable borrow
let fixture_data: Option<(Patch, FixtureProfile)> = {
    app.fixtures.find_patch_at_channel(ch).and_then(|patch| {
        app.fixtures.get_profile(&patch.profile_id).map(|profile| {
            (patch.clone(), profile.clone())
        })
    })
};

if let Some((patch, profile)) = fixture_data {
    render_fixture_properties(ui, app, &patch, &profile);  // Now safe!
}
```

### Performance Considerations

- **Profile Loading**: Happens once at startup (minimal cost)
- **Patch Lookups**: O(n) linear search through patches (acceptable for ~200 fixtures)
- **Color Picker Updates**: Direct DMX writes (no intermediate calculations)
- **UI Rendering**: egui immediate-mode - rebuilds every frame at 60 FPS

Future optimization: Use `HashMap<u16, usize>` to map channels → patch IDs for O(1) lookup.

---

## Known Limitations

### Phase 1-4 Scope

**✅ Complete:**
- Fixture profile definitions
- Profile loading system
- Patch data structures
- Patching UI (add/delete)
- Properties panel fixture detection
- Color picker integration
- Show file persistence

**🚧 Incomplete:**
- Patch editing (dialog exists, returns "Not implemented")
- Right-click patching workflow (no UI)
- Circular color wheel (using rectangular picker)
- Fixture-aware channel selection (clicking fixture should select all its channels)
- Fixture groups
- Color palettes/presets
- Multi-fixture control (selecting multiple fixtures)
- Parameter presets (focus/gobo/color palettes)

### Current Constraints

- **Single Fixture Selection Only**: Properties panel shows fixture controls for one fixture at a time
- **No Undo**: Patching changes are immediate (no undo/redo yet)
- **Linear Search**: Patch lookups are O(n) - fine for ~200 fixtures, may need optimization for larger rigs
- **No Fixture Visualization**: No stage plot or fixture layout view

---

## Future Improvements

### Phase 5: Fixture-Aware Channel Selection
- Clicking channel 10 of RGB fixture (10-12) should auto-select all 3 channels
- Selecting multiple channels of same fixture should show fixture controls
- Visual indicator in channel list showing fixture groups

### Phase 6: Advanced Fixture Control
- Fixture groups (e.g., "All Stage Washes" controls 10 fixtures simultaneously)
- Color palettes (save/recall RGB presets)
- Focus/gobo/beam presets for moving lights
- Multi-fixture color picker (update 10 RGB fixtures at once)

### Phase 7: UI Polish
- Right-click channel → "Patch Fixture Here" workflow
- Patch editing (change address, profile, label)
- Circular color wheel (theatrical-style)
- Fixture icons in channel list
- Stage plot view (visual layout of patched fixtures)

### Phase 8: Advanced Features
- Fixture libraries from GDTF/MVR standards
- Pan/tilt palettes with position tracking
- Effects engine (color chase, dimmer pulse, etc.)
- Blind mode (program fixtures without affecting output)

---

## Testing Checklist

### Profile Loading
- [ ] Run `cargo run` - check logs for "Loaded X profiles"
- [ ] Verify 13 bundled profiles appear in Patching panel dropdown
- [ ] Create custom profile in user config dir - verify it loads

### Patching Workflow
- [ ] Open Patching tab
- [ ] Add RGB fixture at address 10
- [ ] Verify shows in table: "RGB LED" at channels 10-12
- [ ] Try adding overlapping fixture - verify error message
- [ ] Delete fixture - verify removed from table

### Fixture Control
- [ ] Patch RGB at channels 10-12
- [ ] Click channel 10 in Channels panel
- [ ] Verify Properties shows "RGB #1 (RGB LED)" with color picker
- [ ] Drag color picker to red - verify channels 10=255, 11=0, 12=0
- [ ] Use individual sliders - verify DMX updates

### Show Persistence
- [ ] Patch 3 fixtures
- [ ] Save show
- [ ] Close and reopen
- [ ] Verify patches reload correctly
- [ ] Test backwards compatibility: load old show without patches

### Edge Cases
- [ ] Patch fixture at address 510 (3ch - should wrap to 512)
- [ ] Try patching at address 511 (3ch - should fail: exceeds 512)
- [ ] Select unpatched channel - verify shows raw DMX value
- [ ] Select channel of iRGB fixture - verify shows intensity + color picker

---

## Files Changed

**New Files:**
```
src/fixtures/profiles.rs          (230 lines)
src/fixtures/patching.rs          (150 lines)
src/ui/patching.rs                (340 lines)
fixture_profiles/dimmer.json      (13 files total)
fixture_profiles/rgb.json
fixture_profiles/rgba.json
fixture_profiles/rgbw.json
fixture_profiles/rgbaw.json
fixture_profiles/rgbawuv.json
fixture_profiles/irgb.json
fixture_profiles/irgba.json
fixture_profiles/irgbw.json
fixture_profiles/irgbaw.json
fixture_profiles/irgbawuv.json
fixture_profiles/led_par.json
fixture_profiles/moving_head.json
```

**Modified Files:**
```
Cargo.toml                        (+ dirs = "5.0")
src/fixtures/mod.rs               (replaced stub with full implementation)
src/ui/mod.rs                     (+ pub mod patching)
src/ui/properties.rs              (+ fixture detection, color picker)
src/show/mod.rs                   (+ pub patch: Vec<Patch>)
src/app.rs                        (+ patching_state, TabKind::Patching, patch save/load)
```

**Total Additions:** ~1500 lines of code + 13 JSON files

---

## Commit Message Suggestion

```
feat: Add fixture profile and patching system (Phases 1-4)

Implements theatrical fixture control with parameter-based UI:

- 17 parameter types (intensity, RGBAWUV, strobe, pan/tilt, etc.)
- 13 default fixture profiles (dimmer, RGB variants, LED PAR, moving head)
- Cross-platform profile loading (bundled + user config directories)
- Patching system with address validation and overlap detection
- Patching UI panel (add/delete fixtures)
- Fixture-aware properties panel with color picker
- Show file persistence for patches

Closes #X (fixture control feature request)
```

---

## References

- **ETC EOS Consoles**: Industry standard for theater lighting (fixture-centric workflow)
- **QLab**: Media playback with fixture control (inspiration for color picker UI)
- **GDTF/MVR**: Industry standards for fixture definitions (future integration)
- **egui Color Picker Docs**: https://docs.rs/egui/latest/egui/fn.color_edit_button_srgba.html

---

**Document Version:** 1.0  
**Last Updated:** April 29, 2026  
**Implementation Status:** ✅ Complete - Ready for Testing
