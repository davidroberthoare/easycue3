# Virtual Intensity System Implementation Plan

**Date:** April 30, 2026  
**Feature:** Unified intensity control for RGB and iRGB fixtures  
**Status:** Phase 1, 2 & 3 Complete ✅

---

## Overview

Implementing a virtual intensity system that provides consistent "dial up/down" control for all fixture types, regardless of whether they have a dedicated intensity channel (iRGB) or not (RGB). The system preserves color hue while scaling brightness.

### Problem Statement

- **RGB fixtures** (no intensity channel): Changing individual R/G/B values is unintuitive for users
- **iRGB fixtures** (has intensity channel): Natural intensity control already exists
- **Goal**: Same interface and feel for both types, preserving color when adjusting intensity

### Solution

Proportional scaling algorithm (industry standard from QLC+):
```
finalChannel = colorRatio × virtualIntensity × 100
```

Where:
- `colorRatio`: Normalized ratio (0.0-1.0) for each color channel
- `virtualIntensity`: User-controlled intensity (0.0-1.0)
- DMX range: 0-100 (EasyCue3 uses 0-100, not standard 0-255)

---

## Implementation Phases

### ✅ Phase 1: Virtual Intensity Core (COMPLETE)

**File:** `src/fixtures/intensity.rs`

**Implemented:**
- [x] `VirtualIntensity` struct with per-fixture state storage
- [x] `FixtureColorState` struct storing color ratios and intensity
- [x] `calculate_intensity()` - reads current DMX and returns max channel / 100
- [x] `set_color()` - stores normalized color ratios
- [x] `set_intensity()` - scales all color channels proportionally
- [x] `apply_to_universe()` - writes calculated DMX values
- [x] `update_from_universe()` - recalculates ratios after cue playback
- [x] Unit tests: purple at half, zero/restore, max clamping

**Key Discovery:**
Universe uses 0-100 DMX range, not 0-255. All calculations adjusted accordingly.

**Test Results:**
```bash
cargo test --lib intensity
# 3 tests passing:
# - test_purple_at_half_intensity
# - test_intensity_to_zero_and_back
# - test_intensity_stops_at_max
```

---

### ✅ Phase 2: App Integration (COMPLETE)

**File:** `src/app.rs`

**Implemented:**
- [x] Added `virtual_intensity: VirtualIntensity` field to `EasyCueApp`
- [x] Initialized in `new()` constructor
- [x] Exported `fixtures` module in `src/lib.rs` for testing
- [x] Application builds successfully

**Build Status:**
```bash
cargo build
# Success with 10 warnings (all non-critical)
```

---

### ✅ Phase 3: Instrument List UI (COMPLETE)

**File:** `src/ui/channels.rs`

**Implemented:**
- [x] Replaced channel grid with fixture-centric instrument list
- [x] Display format: `[#ID] Label (Type) - Intensity: XX%`
- [x] Click-drag intensity control (vertical/horizontal)
- [x] Added "Show Unpatched Channels" toggle button
- [x] Tracked `selected_fixtures: HashSet<usize>` in `UIState`
- [x] Implemented fixture selection (click/shift-click/ctrl-click)
- [x] Quick intensity buttons for selected fixtures (0%, 25%, 50%, 75%, FL)
- [x] Dual-mode panel: instrument list (default) and channel grid (toggle)

**Key Features:**
- **Instrument List Mode**: Shows all patched fixtures with intensity controls
- **Channel Grid Mode**: Traditional 512-channel view (via toggle)
- **Intensity Control**: Works for both RGB (virtual) and iRGB (direct) fixtures
- **Selection System**: Multi-select with Shift/Ctrl modifiers
- **Visual Feedback**: Color-coded intensity levels, selection highlighting

**Build Status:**
```bash
cargo build
# Success - 7 warnings (all non-critical, unused code)
cargo test --lib
# 15 tests pass (including all fixture/intensity tests)
```

---

### 🔄 Phase 4: Command Parser Extension (TODO)

**File:** `src/ui/channels.rs`

**Current State:**
- Displays 512-channel grid OR fixture list (toggle mode)
- Click-drag intensity control implemented
- Fixture selection working (click/shift-click/ctrl-click)

**No Additional Changes Required** - Phase 3 fully implemented

---

### 🔄 Phase 4: Command Parser Extension (TODO)

**File:** `src/command.rs`

**Current State:**
- EOS-style channel-based commands only
- Examples: `4a33`, `1thru10a50`, `a50`

**Required Changes:**
- [ ] Add `SetFixtureIntensity { fixtures: Vec<usize>, intensity: f32 }`
- [ ] Add `SelectFixtures { fixtures: Vec<usize> }`
- [ ] Context-aware parsing: fixture IDs when instrument list focused, channels otherwise
- [ ] Route iRGB to direct channel control, RGB to virtual intensity

**Command Examples:**
```
1a50        # Fixture 1 to 50% (if instrument list focused)
1thru5a75   # Fixtures 1-5 to 75%
```

---

### 🔄 Phase 5: Properties Panel Enhancement (TODO)

**File:** `src/ui/properties.rs`

**Current State:**
- Has `render_fixture_properties()` for RGB fixtures
- Shows color picker for RGB, intensity slider for iRGB

**Required Changes:**
- [ ] Add "Virtual Intensity" slider for RGB-only fixtures
- [ ] Intercept color picker changes to update ratios via `VirtualIntensity::set_color()`
- [ ] Preserve intensity when color changes
- [ ] Display current intensity percentage
- [ ] Update UI to call `VirtualIntensity::set_intensity()` on slider drag

**UI Design:**
```
Properties Panel (RGB fixture selected)
┌────────────────────────────────┐
│ Fixture #1: LED Par (RGB)      │
│                                 │
│ Color Picker:                  │
│ [Color Wheel UI]               │
│                                 │
│ Virtual Intensity:             │
│ [=========>       ] 50%        │
│                                 │
└────────────────────────────────┘
```

---

### 🔄 Phase 6: Integration Testing (TODO)

**Test Cases:**
- [ ] RGB fixture: Set purple, drag intensity from 0-100%, verify hue preserved
- [ ] iRGB fixture: Verify intensity control still routes to dedicated channel
- [ ] Command execution: `1a50` sets RGB fixture to 50% virtual intensity
- [ ] Color picker: Change color, verify intensity preserved
- [ ] Cue playback: After fade, intensity control still works
- [ ] Multiple fixtures: Select fixtures 1-5, drag all to same intensity
- [ ] Edge cases: Zero intensity, max intensity, unpatched channels

**Manual Test Procedure:**
1. Patch RGB fixture (e.g., `led_par.json`) to channel 10
2. Patch iRGB fixture (e.g., `irgb.json`) to channel 20
3. Open instrument list, verify both fixtures shown
4. Click-drag RGB fixture intensity slider
5. Verify DMX output maintains color ratios
6. Repeat for iRGB fixture
7. Test command parser: `1a75` and `2a50`
8. Test cue recording and playback

---

## Architecture Details

### Data Flow

```
User Input (UI/Command)
    ↓
VirtualIntensity::set_intensity(fixture_id, intensity)
    ↓
Calculate: dmx_value = ratio × intensity × 100
    ↓
Universe::set_channel(channel, dmx_value)
    ↓
DMX Output Thread → Physical Fixtures
```

### State Management

**Per-Fixture State:**
```rust
FixtureColorState {
    color_ratios: HashMap<FixtureParameter, f32>,  // Normalized 0.0-1.0
    intensity: f32,                                 // Current intensity 0.0-1.0
}
```

**Storage:**
```rust
VirtualIntensity {
    fixture_states: HashMap<usize, FixtureColorState>
}
```

### Dual Fixture Support

**Routing Logic:**
```rust
if fixture_profile.has_intensity() {
    // iRGB: Direct channel control
    universe.set_channel(intensity_channel, value)
} else {
    // RGB: Virtual intensity
    virtual_intensity.set_intensity(fixture_id, value, ...)
}
```

---

## Code Locations

### Implemented (Phase 1, 2 & 3)
- `src/fixtures/intensity.rs` - Virtual intensity core (226 lines) ✅
- `src/fixtures/mod.rs` - Module exports ✅
- `src/app.rs` - Integration into EasyCueApp + UIState extensions ✅
- `src/lib.rs` - Module visibility for tests ✅
- `src/ui/channels.rs` - Instrument list UI with dual-mode panel ✅

### To Modify (Phase 4-6)
- `src/command.rs` - Fixture-based commands
- `src/ui/properties.rs` - Add virtual intensity slider
- `src/app.rs` - Command execution routing

### Supporting Files
- `src/fixtures/profiles.rs` - Already has `has_intensity()`, `is_color()`
- `src/fixtures/patching.rs` - Patch system (no changes needed)
- `src/dmx/universe.rs` - DMX storage (no changes needed)

---

## Testing Strategy

### Unit Tests (Phase 1) ✅
- Color preservation during intensity changes
- Zero/restore cycle maintains ratios
- Max intensity clamping

### Integration Tests (Phase 6)
- UI interaction testing
- Command parser execution
- Cue playback integration
- Multi-fixture selection

### Manual Testing
- Visual verification with physical fixtures
- USB DMX output validation
- Virtual DMX logging verification

---

## Dependencies

**Existing Systems:**
- `Universe` (DMX channel storage) ✅
- `FixtureLibrary` (profiles and patching) ✅
- `FixtureProfile::has_intensity()` ✅
- `FixtureParameter::is_color()` ✅

**New Systems:**
- `VirtualIntensity` ✅ (Phase 1)
- Instrument list UI (Phase 3)
- Fixture commands (Phase 4)

---

## Performance Considerations

- Proportional scaling is O(n) where n = color channels per fixture (typically 3-6)
- State lookup: HashMap O(1) average
- No allocations in hot path (set_intensity uses existing state)
- DMX calculations run at 40 Hz, UI at 60 FPS - no performance issues expected

---

## Industry Validation

**Research:** QLC+ open-source lighting console uses identical proportional multiplication approach for "Grand Master" intensity control, validating our algorithm choice.

**Standard Practice:**
- Professional fixtures typically have dedicated intensity channels (iRGB)
- RGB-only fixtures are common in hobbyist/educational contexts
- Our solution addresses a real gap in beginner-friendly console design

---

## Constraints

- **Universe Count:** 2-16 (not 100+)
- **Fixture Count:** ~200 max (not thousands)
- **DMX Range:** 0-100 (EasyCue3-specific, not standard 0-255)
- **Channel Precision:** 8-bit (no 16-bit support)
- **Target Audience:** Educational, small venues, hobbyists

---

## Next Steps

1. **Immediate:** Start Phase 4 (Command Parser Extension)
2. **Then:** Phase 5 (Properties Panel Enhancement)
3. **Finally:** Phase 6 (Integration testing with real fixtures)

**Estimated Completion:** Phases 4-6 should take 2-3 hours of focused development time.

**Phase 3 Complete!** The instrument list UI is fully functional with:
- Fixture-centric view showing all patched fixtures
- Click-drag intensity control for RGB and iRGB fixtures
- Multi-select with Shift/Ctrl modifiers
- Toggle to show traditional channel grid
- Color-coded intensity display

---

## Git Commit History

### Completed Commits
- `feat: Add virtual intensity core system for RGB fixtures`
  - Created src/fixtures/intensity.rs with proportional scaling algorithm
  - Integrated VirtualIntensity into EasyCueApp
  - Added unit tests (3/3 passing)
  - Adjusted for 0-100 DMX range compatibility

- `feat: Implement instrument list UI with intensity control`
  - Converted channels panel to fixture-centric interface
  - Added click-drag intensity control for fixtures
  - Implemented fixture selection state tracking
  - Added toggle for traditional channel grid view
  - Multi-select with Shift/Ctrl modifiers
  - Color-coded intensity display
  - Quick intensity buttons (0%, 25%, 50%, 75%, FL)

### Next Commit
- `feat: Add fixture-based command parser syntax`
  - Extend command parser to support fixture IDs
  - Context-aware parsing (fixtures vs channels)
