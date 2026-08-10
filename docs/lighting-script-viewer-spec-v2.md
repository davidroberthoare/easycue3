# Script Viewer with Cue Annotations — Draft Spec v2

## Goal
Let a technician load a PDF script and place cue markers at specific points on the page, then use those markers to trigger cues during playback.

## Scope
- In: PDF rendering, two operating modes (Edit / Playback), marker placement, marker persistence, zoom/pan, marker → cue linking and triggering.
- Out: scanned images, OCR, multi-technician/shared annotation sets.

## Modes

### Edit mode
- Double-click an empty area of the page → opens "add cue" popup at that point:
  - Pick an existing cue from the show file's cue list, **or**
  - Create a new cue inline, choosing type: `Lighting`, `Sound`, or `Adjustment`.
  - Either way, result is a marker at `(page_index, x, y)` linked to a `cue_id`.
- Click existing marker → select/edit it (reassign cue, or delete).
- Drag existing marker → update stored `(x, y)`.

### Playback mode
- Markers are read-only (no placement/editing).
- Click a marker → triggers the linked cue directly (same effect as firing it from the cue list).
- No "select but don't fire" behavior needed — click = go, matching how a running board op would use it live.

## Coordinate model
- Markers stored as `(page_index, x, y)` in **PDF point space** (native page coordinates from pdfium), never screen space.
- Screen↔page transform computed at click time and render time only.
- Stable across zoom/pan by construction.

## Rendering pipeline
1. Load PDF via `pdfium-render`.
2. Rasterize each page once to a bitmap at a fixed working resolution; upload as GPU texture.
3. egui scales/pans the texture for zoom/pan — no re-rasterization per frame.
4. Re-rasterize a page at higher resolution only if zoom exceeds the cached texture's clarity, debounced.
5. Draw markers each frame via `egui::Painter`, transforming stored page-space coords to current screen space.

## Data model
- `CueMarker { page_index: usize, x: f32, y: f32, cue_id: CueId }`
- `cue_id` references the existing show-file cue list — this feature does not duplicate cue data, only adds a spatial index into it.
- New cues created inline (Lighting/Sound/Adjustment) go through the same creation path as adding a cue from the normal cue list UI, then get a marker attached.

## Persistence
- Marker list stored per script, associated with the show file (exact storage location/format — e.g. new field in show file vs. sidecar file — left to implementer, consistent with existing show-file conventions).
- PDF source: store a path reference (not embedded), unless existing show-file convention already embeds assets.

## Suggested crates
- `pdfium-render` for PDF rasterization (permissive license, fits open-source distribution — preferred over AGPL `mupdf`).

## Open questions
1. Exact existing `Cue` struct/enum shape (fields for Lighting/Sound/Adjustment types) — needed to wire up "create new cue inline."
2. Show file format/storage convention — where should the marker list live (embedded field vs. sidecar file)?
3. Mode switching UI — toggle button, menu, or keyboard shortcut?
4. Any visual distinction needed between marker types (Lighting/Sound/Adjustment) on the page, e.g. color-coding?
