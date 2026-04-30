# Virtual Intensity Multi-Color Fixture Fix

**Date:** April 30, 2026  
**Issue:** All colors except red snap to 0 when adjusting intensity on RGBAWUV fixtures  
**Status:** FIXED ✅

## Problem Description

When using an RGBAWUV fixture with virtual intensity:
1. User sets mid-range color values (e.g., r=55, g=20, b=30, a=66, w=44, uv=0)
2. User adjusts the intensity slider
3. **BUG:** All colors except red snap to 0, only red follows the intensity slider

## Root Cause

The RGB color picker in the properties panel was only updating the virtual intensity state with RGB ratios, leaving Amber, White, and UV channels without stored ratios. When the intensity slider was adjusted, those missing channels defaulted to 0.0.

### Code Flow (Before Fix)

1. User adjusts RGB color picker → `set_color()` called with **only RGB values**
2. Virtual intensity state stores ratios for R, G, B only
3. User adjusts intensity slider → `set_intensity()` called
4. For each color parameter:
   - R, G, B: Have stored ratios → calculate correctly
   - A, W, UV: **No stored ratios → unwrap_or(0.0) → snap to 0**

## Solution

### 1. Color Picker Fix (`src/ui/properties.rs`)

When the RGB color picker changes, now:
- Update RGB channels in universe (as before)
- **Read ALL other color channels from universe** (A, W, UV, etc.)
- Pass **all color values** to `set_color()` to store complete ratios

```rust
// For fixtures without dedicated intensity, update ALL color ratios
// (not just RGB) to preserve other color channels like Amber, White, UV
if !profile.has_intensity() {
    let mut color_values = std::collections::HashMap::new();
    color_values.insert(FixtureParameter::Red, new_r);
    color_values.insert(FixtureParameter::Green, new_g);
    color_values.insert(FixtureParameter::Blue, new_b);
    
    // Read other color channels from universe to preserve them
    for param_mapping in profile.color_parameters() {
        if !matches!(param_mapping.parameter, 
            FixtureParameter::Red | 
            FixtureParameter::Green | 
            FixtureParameter::Blue) {
            let ch = patch.start_address + param_mapping.channel_offset;
            if let Ok(value) = universe.get_channel(ch) {
                color_values.insert(param_mapping.parameter.clone(), value);
            }
        }
    }
    
    app.virtual_intensity.set_color(patch.id, color_values);
}
```

### 2. Individual Color Slider Fix (`src/ui/properties.rs`)

When any individual color slider changes (R, G, B, A, W, UV):
- Update the DMX channel (as before)
- **Refresh virtual intensity state** from universe using `update_from_universe()`

```rust
if ui.add(egui::Slider::new(&mut val, 0..=100)).changed() {
    let _ = universe.set_channel(ch, val);
    // Update virtual intensity state if applicable
    if !profile.has_intensity() {
        let patch_clone = patch.clone();
        let profile_clone = profile.clone();
        app.virtual_intensity.update_from_universe(
            patch.id, universe, &patch_clone, &profile_clone
        );
    }
}
```

This ensures the virtual intensity state always has current ratios for all color channels.

## Testing

Added comprehensive test: `test_multi_color_fixture_intensity()`

```rust
// Set initial: r=55, g=20, b=30, a=66, w=44, uv=10
// Ratios: r=0.833, g=0.303, b=0.455, a=1.0, w=0.667, uv=0.152

// At 50% intensity:
assert_eq!(r, 41);   // ✅ All colors scale
assert_eq!(g, 15);   // ✅ proportionally
assert_eq!(b, 22);   // ✅
assert_eq!(a, 50);   // ✅
assert_eq!(w, 33);   // ✅
assert_eq!(uv, 7);   // ✅

// At 75% intensity:
assert_eq!(r, 62);   // ✅ All colors preserved
assert_eq!(g, 22);   // ✅ at new intensity
assert_eq!(b, 34);   // ✅
assert_eq!(a, 75);   // ✅
assert_eq!(w, 50);   // ✅
assert_eq!(uv, 11);  // ✅
```

**Test Results:** All 4 tests pass ✅

## Verification Steps

To verify the fix works:

1. **Patch an RGBAWUV fixture:**
   - Go to Patching panel
   - Add fixture using `rgbawuv.json` profile
   - Set start address (e.g., channel 1)

2. **Set multi-color values:**
   - Select the fixture in Instrument List
   - Open Properties panel → Color Channels
   - Set: R=55, G=20, B=30, A=66, W=44, UV=10

3. **Test intensity control:**
   - Adjust Virtual Intensity slider to 50%
   - **Expected:** All channels scale proportionally
   - **Before Fix:** Only R scaled, others → 0
   - **After Fix:** R=41, G=15, B=30, A=50, W=33, UV=7 ✅

4. **Test color picker:**
   - Change color using RGB picker
   - Adjust intensity slider
   - **Expected:** A, W, UV remain at their previous ratios ✅

5. **Test individual sliders:**
   - Adjust Amber slider to 80
   - Adjust intensity
   - **Expected:** New Amber value preserved in ratio ✅

## Impact

- **Scope:** All RGB-based fixtures without dedicated intensity (RGB, RGBAW, RGBAWUV, etc.)
- **Behavior Change:** Color preservation now works correctly for all color channels
- **Backward Compatibility:** No breaking changes, only fixes incorrect behavior
- **Performance:** Minimal overhead (reads extra channels when color picker used)

## Files Modified

1. `src/ui/properties.rs` - Color picker and slider updates
2. `src/fixtures/intensity.rs` - Added test coverage

## Related Documentation

- [VIRTUAL_INTENSITY_PLAN.md](VIRTUAL_INTENSITY_PLAN.md) - Original implementation plan
- [FIXTURE_SYSTEM.md](FIXTURE_SYSTEM.md) - Fixture system architecture
