# Virtual Intensity

**Status:** Implemented (`src/fixtures/intensity.rs`)

## Concept

RGB-only fixtures (no dedicated intensity channel) present a usability problem: adjusting individual R/G/B values is unintuitive when the user just wants to "turn it up." Virtual intensity adds a unified intensity control that preserves color hue while scaling brightness.

**Algorithm** (proportional scaling — same approach as QLC+):

```
dmx_value = color_ratio × virtual_intensity × 100
```

`color_ratio` is each color channel's normalized value (0.0–1.0) relative to the maximum color channel. The user sees one intensity control; all color channels scale proportionally.

**iRGB fixtures** (with a dedicated intensity channel) route intensity directly to that channel — no virtual intensity needed.

## Key Detail: Internal DMX Range

EasyCue3's `Universe` stores values as **0–100** (percentage), not 0–255 (standard DMX). This was confirmed during virtual intensity implementation and all math uses this range. The command parser accepts raw values 101–255 and converts them, but internally everything is 0–100.

## Data Flow

```
User adjusts intensity slider
    ↓
VirtualIntensity::set_intensity(fixture_id, 0.0–1.0)
    ↓
For each color channel: dmx = ratio × intensity × 100
    ↓
Universe::set_channel(channel, dmx_value)
```

## State Per Fixture

```rust
FixtureColorState {
    color_ratios: HashMap<FixtureParameter, f32>,  // normalized 0.0-1.0
    intensity: f32,                                 // current intensity 0.0-1.0
}
```

Ratios are updated whenever color values change (via color picker or individual sliders). `update_from_universe()` recalculates ratios after cue playback restores DMX values.

## Multi-Color Fixtures (RGBAWUV)

The RGB color picker only yields R/G/B values. When storing ratios, **all** color channels (Amber, White, UV, etc.) must be read from the universe and included — otherwise they default to 0.0 when intensity is adjusted, causing those channels to snap to black. Fixed in `src/ui/properties.rs` by reading non-RGB channels from the universe before calling `set_color()`.

## Key Files

- `src/fixtures/intensity.rs` — core algorithm, `VirtualIntensity` and `FixtureColorState` structs
- `src/ui/properties.rs` — virtual intensity slider, color picker integration, multi-color fix
- `src/ui/channels.rs` — instrument list UI with click-drag intensity control
