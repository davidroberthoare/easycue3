# Magic Sheet Feature

## Overview

A freeform fixture-layout canvas panel, similar to the theatrical "magic sheet" used in live production. Operators place shapes on a canvas, assign each shape to a fixture, and use the canvas in live mode to select and control fixtures spatially.

Inspired by ETC EOS's "Magic Sheet" view. Lives alongside the Channels panel as an alternative spatial fixture interface.

## Design Decisions

| Decision | Choice | Reason |
|---|---|---|
| Persistence | Saved in show file | Sheet layout is show-specific |
| Multiple sheets | One per show (extendable later) | Simple for MVP; multi-sheet adds navigation complexity |
| Shape sizing | Fixed base size × `scale` per shape | Avoids resize-handle complexity; scale slider is enough |
| Background image | Future | Needs file-picker + image loading |
| Pan & zoom | Yes | Canvas can grow large in real venues |
| Shape-to-fixture | 1 shape = 1 fixture (groups later) | Groups not yet implemented |
| Selection sync | Bidirectional with Channels panel | Both panels use `app.ui_state.selected_fixtures` as source of truth |
| Colours | Per-shape `bg_color` + `outline_color` (RGBA `[u8;4]`) | Lets operator colour-code by circuit, zone, etc. |
| Shape types | Start: Rectangle, Circle, Diamond — expandable enum | Custom SVG dir is a future extension point |

## Data Model

```
ShowFile
  └── magic_sheet: MagicSheet
        ├── next_shape_id: u32
        └── shapes: Vec<MagicSheetShape>
              ├── id: u32
              ├── kind: ShapeKind (Rectangle | Circle | Diamond | …)
              ├── pos: [f32; 2]          — canvas-space centre (logical px)
              ├── scale: f32             — size multiplier (1.0 = ~80×60 px)
              ├── bg_color: [u8; 4]      — RGBA fill
              ├── outline_color: [u8; 4] — RGBA border
              └── fixture_id: Option<usize>   — links to Patch::id
```

Ephemeral per-session state (not serialized, in `EasyCueApp`):

```
MagicSheetState
  ├── edit_mode: bool
  ├── selected_shape_id: Option<u32>
  ├── canvas_offset: Vec2       — pan (screen pixels)
  └── canvas_zoom: f32          — zoom level (1.0 = 100%)
```

## Canvas Coordinate System

All shape positions are stored in **canvas space** (logical pixels, origin top-left of canvas).

Screen position = `canvas_pos * zoom + canvas_offset`

## Implementation Stages

- [x] **Commit 1** — Foundation: `MagicSheet` data structures, `ShowFile` integration, `TabKind::MagicSheet`, skeleton panel wired into dock + View menu
- [ ] **Commit 2** — Shape rendering: draw Rectangle/Circle/Diamond via egui Painter; display fixture label, id, intensity %, colour swatch
- [ ] **Commit 3** — Edit mode basics: toggle switch, shape palette (click to place at canvas centre), drag shapes to reposition
- [ ] **Commit 4** — Edit mode properties panel: selected-shape side panel with fixture dropdown, scale DragValue, bg/outline colour pickers, delete button
- [ ] **Commit 5** — Pan & zoom: middle-click drag or right-click drag to pan; scroll wheel to zoom; reset-view button
- [ ] **Commit 6** — Live mode interactions: click/Ctrl/Shift to select fixtures; vertical drag for intensity; bidirectional sync with Channels panel

## Key Files

| File | Role |
|---|---|
| `src/magic_sheet/mod.rs` | Serialisable data structures (`MagicSheet`, `MagicSheetShape`, `ShapeKind`) |
| `src/ui/magic_sheet.rs` | Panel rendering (egui) |
| `src/show/mod.rs` | `magic_sheet` field in `ShowFile` |
| `src/app.rs` | `TabKind::MagicSheet`, `MagicSheetState`, load/save wiring |
| `src/ui/mod.rs` | Module export, tab viewer dispatch, View menu entry |

## Future Extensions

- Custom SVG shapes loaded from `magic_sheet_shapes/` directory
- Background stage-plot image
- Multiple named sheets (tabs within the panel)
- Group shapes (one shape → one fixture group)
- Label font size control
- Shape locking (prevent accidental moves in live mode)
