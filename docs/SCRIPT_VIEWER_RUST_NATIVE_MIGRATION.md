# Migration plan: pdfium → stet (pure-Rust PDF) for the script viewer

**Status:** Proposed — not started. Filed for future consideration.
**Date:** 2026-08-10
**Context:** Current viewer rasterizes PDFs via `pdfium-render = "0.9.3"`,
which dlopens `libpdfium.so` at runtime (`src/scriptviewer/mod.rs`).

## Goal

Replace the pdfium native-library dependency with the pure-Rust
`stet-pdf-reader` crate so the script viewer needs no runtime `.so`.

## Decisions (confirmed)

1. **Keep the current rasterize-on-zoom model** — 1:1 engine swap, no
   behavior change. (stet's display-list viewport rendering is a possible
   later optimization.)
2. **Pin `stet-pdf-reader` by git rev**, like `lumina-video` (young 0.2.x
   crate; everything public is `#[non_exhaustive]` so churn is additive).
3. **No Cargo feature gate** — stays unconditional, like today.

## Why stet

- `PdfDocument::from_bytes(&bytes)` → `page_count()`, `page_size(i) ->
  (f64, f64)` (rotation-aware), `render_page_to_rgba(i, dpi) -> (Vec<u8>,
  u32, u32)` — RGBA natively (pdfium gives BGRA; `as_rgba_bytes()`
  currently normalizes). Annotations rendered automatically.
- Self-contained: embeds 35 URW fonts + ICC profiles. No `libpdfium.so`,
  no `PDFIUM_LIBRARY_PATH`, no runtime library resolution.
- `PdfDocument<'a>` is `!Send` → stays on the main thread, same as today.
- Workspace is egui/eframe 0.31 (matches this project's pin), though we
  only use `stet-pdf-reader` (no egui dependency).
- On crates.io, Apache-2.0 OR MIT. 262 downloads at time of research.

## Work

### 1. Cargo.toml (`:51`)
Replace `pdfium-render = "0.9.3"` with a git-pinned rev:
`stet-pdf-reader = { git = "https://github.com/AndyCappDev/stet", rev = "<latest main rev>" }`
(resolve rev via `git ls-remote` at implementation time).

### 2. src/scriptviewer/mod.rs (the only real surgery)
- Remove: `pdfium` field (`:150`), `pdfium()` (`:218`), `bind_pdfium_library()`
  (`:502`), `PdfiumLibraryBindingsDyn` (`:546`), the `use pdfium_render` import (`:32`).
- Add: `pdf_bytes: Option<&'static [u8]>` field backing the doc borrow.
- `load_pdf()` (`:248`): `fs::read` + `PdfDocument::from_bytes(&bytes)`
  with `Box::leak(bytes.into_boxed_slice())` → `PdfDocument<'static>`
  (same idiom as the current `Box::leak(Pdfium)`).
- `rasterize_page()` (`:335`) / `refine_current_page()` (`:357`):
  `page.width()/height()` → `document.page_size(i)?`;
  `page.render_with_config(...)` → `document.render_page_to_rgba(
  i, 72.0 * target_px / width_pts)?` (exact inverse of `set_target_width`);
  `render_form_data/render_annotations` config goes away.
- `upload_bitmap()` (`:471`): take `(Vec<u8>, u32, u32)`, drop the BGRA→RGBA pass.
- Smoke test (`:607`): drop the PDFium-availability skip; runs on `media/lorem.pdf`.
- Module doc header (`:20`): rewrite "PDF library" section.

### 3. Docs
- `docs/SCRIPT_VIEWER.md`: remove libpdfium install steps, `PDFIUM_LIBRARY_PATH`,
  resolution order.
- `AGENTS.md`: fix pdfium mention (`:33`); note script viewer is now native.

### 4. Verify
- `cargo check` (per AGENTS.md), then manual test.
- `cargo test` — expect only the known pre-existing `enttec_usb_pro` failure.
- Visual fidelity pass on `media/lorem.pdf` + a real script.

## Effort

~Half a day. The pdfium surface is ~15 API calls confined to one file; the
UI (`src/ui/script_viewer.rs`) is pdfium-agnostic and untouched. `CueMarker`
storage and point↔screen math are unchanged, so existing show files migrate
without changes.

## Risks / tradeoffs

- **Maturity:** 0.2.x, single maintainer, young — hence the git-rev pin.
- **Speed:** pure-Rust rasterization slower than C pdfium; page-turn/refine
  may hitch on large scripts. Mitigations: tune `WORKING_WIDTH_PX`, or later
  adopt display-list viewport rendering (`prepare_display_list` +
  `render_region_prepared`) which also makes zoom-refine continuous.
- **Binary/build:** ~5 MB embedded fonts/ICC + ~30 crates of build time;
  slight binary growth.
- **Regression vs pdfium:** `render_form_data(true)` (form-field values).
  stet renders widget appearance streams, equivalent for plain theater
  scripts. Rotation handling also changes (stet bakes rotation into
  `page_size`/render — arguably more correct, edge case only).

## Alternatives considered

- `hayro` — the renderer stet's reader builds on; younger/less complete.
- `lopdf` / `oxipdf` / `printpdf` — parse/generate only, no rasterizer.
- `mupdf` / `poppler` bindings — still native libs, same distribution problem.
- pdfium `static` feature — needs a C++ toolchain build; painful.
- Conclusion: stet is the only complete pure-Rust rasterizer.
