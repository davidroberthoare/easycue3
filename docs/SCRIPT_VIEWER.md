# Script Viewer (Cue Markers) — Implementation & Handoff

Implements `docs/lighting-script-viewer-spec-v2.md`: load a PDF script, place
cue markers on it, and click markers during playback to fire cues.

## Status

Working first pass, feature-complete against the spec:

- ✅ PDF loading + rasterization (`pdfium-render`), GPU textures
- ✅ Edit / Playback modes (toolbar toggle)
- ✅ Double-click empty page → add-cue popup (link existing cue OR create
  Lighting / Sound / Adjustment inline)
- ✅ Click to select, drag to move, Delete to remove markers
- ✅ Marker reassignment via combo in the edit strip
- ✅ Click marker in Playback mode → fires the linked cue (GO behaviour)
- ✅ Zoom (Ctrl/Cmd+scroll + buttons) / pan (scroll, or middle/right/shift-drag)
- ✅ "⟲ Fit" fits the whole current page into the canvas, centred
- ✅ Zoom-out re-rasterization at higher resolution (debounced)
- ✅ Persistence in the show file (`ShowFile::script_viewer`)
- ✅ Lazy PDF load from the show file on panel open
- ✅ Marker dots colour-coded by **live cue status** (fading / active / on-deck
  → the `CueColorSettings` status colours, else the cue kind's base colour),
  mirroring the Cue list rows
- ✅ Selection is **linked across all three views**: clicking a dot selects the
  cue in the Cue list (+ Cue Properties); selecting a cue in the Cue list
  (row click, context menu, or ↑/↓ arrows) highlights the matching marker and
  brings it into view if it isn't already on screen. Background click
  deselects everything.
- ✅ Dot labels read `2.0: Label` (label omitted when empty)
- ✅ Left/Right + PageUp/PageDown step through pages while the panel is active
- ✅ Firing a cue anywhere (GO/BACK/goto/autofollow) brings its marker into
  view — **unless it is already visible on screen** (no jump). Jumping to a
  different page always re-centres the marker.

## Files

| File | Purpose |
| --- | --- |
| `src/scriptviewer/mod.rs` | Data model (`CueMarker`, `ScriptViewerData`, `NewCueKind`), runtime state (`ScriptViewer`, `PageImage`), PDF loading/rasterization, page cache, library binding. |
| `src/ui/script_viewer.rs` | The dockable panel: toolbar, canvas, marker interactions, add-cue popup, marker editor strip. |
| `src/app.rs` | `EasyCueApp::script_viewer` field; save/load wiring; `fire_cue_by_id()`, `add_cue_of_kind()`; `TabKind::ScriptViewer`. |
| `src/show/mod.rs` | `ShowFile.script_viewer: ScriptViewerData` (serde-defaulted for older files). |
| `src/main.rs` / `src/ui/mod.rs` | Module registration + View-menu entry. |

## Data model

```rust
CueMarker { page_index: usize, x: f32, y: f32, cue_id: u32 }
ScriptViewerData { pdf_path: Option<PathBuf>, markers: Vec<CueMarker> }
```

- Markers are stored in **PDF point space** (native pdfium units, origin
  top-left); the screen↔page transform is computed per frame in the panel.
- Markers reference cues by stable `cue_id` — no cue data is duplicated.
- `pdf_path` is stored as a bare filename resolved via
  `crate::paths::resolve_media_path` (same `media/` convention as audio cues).

## PDFium library (runtime, not build-time)

`pdfium-render` loads `libpdfium.so` (`libpdfium.dylib` / `pdfium.dll`) via
libloading at runtime. The build needs no library and downloads nothing.

Resolution order in `ScriptViewer::pdfium()` → `bind_pdfium_library()`:

1. `PDFIUM_LIBRARY_PATH` env var (path to the library file)
2. A bundled copy in a `lib/` subdirectory next to the executable, then
   directly next to the executable / cwd (`crate::paths::bundled_library_candidates`)
3. System library search (`LD_LIBRARY_PATH`, standard paths)

### Getting the library (development)

```bash
# Debian/Ubuntu (or download from bblanchon/pdfium-binaries GitHub releases)
# e.g. curl -L -o pdfium-linux-x64.tgz \
#   https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7881/pdfium-linux-x64.tgz
# then set:  export PDFIUM_LIBRARY_PATH=/path/to/libpdfium.so
```

### Release packaging (users install nothing)

The GitHub release workflow (`.github/workflows/release.yml`) downloads the
matching prebuilt library from `bblanchon/pdfium-binaries` (pinned to
`chromium/7881`, matching `pdfium-render` 0.9.3's `pdfium_7881` bindings) and
ships it in a `lib/` subdirectory of each package, keeping the bundle root
clean:

```
easycue3-linux-x86_64/
├── easycue3
├── lib/libpdfium.so        ← linux job (pdfium-linux-x64.tgz)
├── media/  shows/  fixture_profiles/
```

`lib/pdfium.dll` (Windows, from `pdfium-win-x64.tgz`) and
`lib/libpdfium.dylib` (macOS universal, from `pdfium-mac-univ.tgz`) follow the
same layout.

A clear error is shown in the panel if the library is missing.

## Rendering pipeline

- Each page is rasterized once at a working width (1600 px) and uploaded as an
  egui texture (`PageImage`). The render target is auto-capped so the longer
  side never exceeds egui's 2048 px max texture side (important for portrait
  A4 pages — a naive 1600 px width would yield 2071 px tall and trip
  `Context::load_texture`'s debug assertion).
- Only the page window `[current−1, current, current+1]` is kept in memory
  (`ensure_pages_rendered`), bounding RAM for long scripts.
- Zooming past `REFINE_ZOOM_THRESHOLD` re-rasterizes the current page at the
  display resolution (debounced by `REFINE_COOLDOWN`, capped at 4096 px).
- Pan/zoom reuse the magic-sheet interaction pattern (scroll = zoom,
  middle/right/shift-drag = pan).

## Design decisions & notes

- **Module placement:** `scriptviewer` is app-level (declared in `src/main.rs`)
  because it owns egui textures; only the persisted data types are shared
  (via `ShowFile`).
- **PDFium lifetime:** the binding is `Box::leak`ed once to give the loaded
  `PdfDocument` a `'static` lifetime (avoids self-referential borrows). One
  leak per process — acceptable for a single long-running app.
- **Sound cues created inline** open the OS file picker at creation time
  (mirrors the Cues panel's "Add Snd" flow); the file can still be changed
  later in Cue Properties.
- **Adjustment cues** auto-target the most recent audio cue, matching the
  existing Cues-panel behaviour.
- **Marker colours** reuse `CueColorSettings` (`base_lighting`/`base_audio`/
  `base_adjust`); missing cues render grey.
- **Firing** goes through `app.go_to_cue()` (a real GO with fade) via
  `fire_cue_by_id()`.
- `cargo fmt` across the crate is currently blocked by a pre-existing
  trailing-whitespace line in `src/command.rs:444`; format touched files with
  `rustfmt --edition 2021 <files>`.

## Verification

```bash
cargo check
cargo test    # +5 new tests in scriptviewer, +1 in show (marker serde, PDF smoke test)
# PDF smoke test runs only when a PDFium library is available:
PDFIUM_LIBRARY_PATH=/path/to/libpdfium.so cargo test scriptviewer::tests::loads_and_rasterizes_a_pdf
```

No new `cargo test` failures (the sole failure, `test_protocol_message_format`,
is pre-existing).

## Known limitations / follow-ups

- PDF loading and rasterization are synchronous on the UI thread (fine for
  typical script sizes; a background thread + repaint could be added later).
- No multi-page thumbnail strip yet — page navigation is prev/next buttons.
- Markers are not auto-adjusted if the cue they reference is deleted (they show
  grey "?" and can be relinked/deleted in Edit mode).
- The add-cue popup is centered rather than anchored to the click point.
- Playback "click = fire" is by design (no select-without-fire mode).
- Scroll-over-canvas panning relies on egui's `smooth_scroll_delta` reaching the
  panel (the tab's scroll area has no overflow, so events pass through).
