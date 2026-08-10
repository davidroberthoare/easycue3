//! Script Viewer with cue annotations.
//!
//! Loads a PDF script (via `pdfium-render`), displays it on a zoomable/panable
//! canvas, and lets the operator place *cue markers* on the page. Each marker
//! is a lightweight spatial index that references a cue in the show file's cue
//! list by stable ID — this module does not duplicate cue data.
//!
//! Two modes:
//! - **Edit** — double-click to add a marker (linked to an existing cue or a
//!   freshly created one), click/drag to select/reposition, delete via the
//!   marker editor strip.
//! - **Playback** — markers are read-only; clicking one fires the linked cue
//!   exactly as if it were triggered from the cue list.
//!
//! ## Persistence
//! [`ScriptViewerData`] is stored inside the show file (see `src/show/mod.rs`).
//! The PDF source is stored as a *path reference* (bare filename resolved via
//! the `media/` directory, matching the audio-cue convention), not embedded.
//!
//! ## PDF library
//! Rendering uses the `pdfium-render` crate. The PDFium native library
//! (`libpdfium.so` / `libpdfium.dylib` / `pdfium.dll`) is loaded **at runtime**
//! via libloading — the build itself needs no library. Resolution order:
//! `PDFIUM_LIBRARY_PATH` env var → a bundled library next to the executable /
//! cwd → system library search.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

use pdfium_render::prelude::*;

/// Base working resolution for a rasterized page (pixels, target width).
/// Pages are rendered at this width once and displayed as a GPU texture;
/// higher resolutions are only used when the user zooms in far enough.
const WORKING_WIDTH_PX: i32 = 1600;
/// Hard cap on re-rasterization resolution (pixels, target width) so an
/// extreme zoom-in can't allocate an absurd bitmap.
const MAX_RENDER_WIDTH_PX: i32 = 4096;
/// egui refuses textures larger than this side length (see
/// `Context::load_texture`), so rendered pages are always kept within it.
const MAX_TEXTURE_SIDE: u32 = 2048;
/// Zoom factor at which the cached texture is considered too blurry and the
/// current page is re-rasterized at the display resolution.
const REFINE_ZOOM_THRESHOLD: f32 = 1.5;
/// Minimum time between automatic re-rasterizations (debounce).
const REFINE_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(600);

// ─────────────────────────────────────────────────────────────────────────────
// Data model (persisted in the show file)
// ─────────────────────────────────────────────────────────────────────────────

/// A spatial marker linking a point on a PDF page to a show-file cue.
///
/// Coordinates are stored in **PDF point space** (native page units from
/// pdfium, origin top-left) — never screen space — so markers stay put across
/// zoom/pan changes by construction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CueMarker {
    /// Zero-based index of the page the marker sits on.
    pub page_index: usize,
    /// X position in PDF points (0 = left edge of the page).
    pub x: f32,
    /// Y position in PDF points (0 = top edge of the page).
    pub y: f32,
    /// Stable ID of the cue in the show file's cue list this marker links to.
    pub cue_id: u32,
}

impl CueMarker {
    pub fn new(page_index: usize, x: f32, y: f32, cue_id: u32) -> Self {
        Self {
            page_index,
            x,
            y,
            cue_id,
        }
    }
}

/// Per-script annotation set, embedded in the show file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptViewerData {
    /// Path reference to the source PDF. Stored as a bare filename relative to
    /// the `media/` directory (see [`crate::paths::resolve_media_path`]) so
    /// show files stay portable. `None` = no script loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_path: Option<PathBuf>,
    /// Cue markers placed on the pages.
    #[serde(default)]
    pub markers: Vec<CueMarker>,
}

impl ScriptViewerData {
    /// Normalise a picked PDF path so the stored show file keeps just the
    /// filename (stripping a `media/` prefix, matching the audio convention).
    pub fn set_pdf_path(&mut self, path: PathBuf) {
        if let Ok(stripped) = path.strip_prefix("media") {
            self.pdf_path = Some(stripped.to_path_buf());
        } else {
            self.pdf_path = Some(path);
        }
    }
}

/// The kind of cue to create inline from the script viewer's add-cue popup.
/// Maps onto the show file's `CueKind` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCueKind {
    Lighting,
    /// `CueKind::Audio` — requires picking an audio file at creation time.
    Sound,
    /// `CueKind::Adjust` — fades volume/pan on the targeted audio stream.
    Adjustment,
}

// ─────────────────────────────────────────────────────────────────────────────
// Runtime state (never persisted)
// ─────────────────────────────────────────────────────────────────────────────

/// A rasterized PDF page cached as an egui GPU texture.
pub struct PageImage {
    /// Page width in PDF points.
    pub width_pts: f32,
    /// Page height in PDF points.
    pub height_pts: f32,
    /// Width of the rendered bitmap in pixels (height follows the aspect ratio).
    pub render_px_w: u32,
    /// GPU texture holding the page image.
    pub texture: egui::TextureHandle,
}

/// One pending marker waiting for the operator to choose/create a cue in the
/// add-cue popup. Coordinates are in PDF point space on the given page.
#[derive(Debug, Clone, Copy)]
pub struct PendingMarker {
    pub page_index: usize,
    pub x: f32,
    pub y: f32,
}

/// Runtime state of the script viewer panel (not saved with the show file).
pub struct ScriptViewer {
    /// Persisted annotation set (synced with the show file on save/load).
    pub data: ScriptViewerData,

    /// PDFium library binding, leaked so the loaded document can borrow it
    /// with a `'static` lifetime. Bound once per process on first load.
    pdfium: Option<&'static Pdfium>,
    /// Currently loaded PDF document (kept alive so pages can be re-rasterized
    /// at higher resolution on zoom-in).
    document: Option<PdfDocument<'static>>,
    /// Total number of pages in the loaded document.
    page_count: usize,
    /// Rasterized page cache, indexed by page number. Only the page window
    /// `[current-1, current, current+1]` is kept resident to bound memory.
    pages: Vec<Option<PageImage>>,
    /// Page currently displayed.
    pub current_page: usize,

    /// View state.
    pub zoom: f32,
    pub pan: egui::Vec2,

    /// Edit / playback mode toggle.
    pub edit_mode: bool,
    /// Index (into `data.markers`) of the currently selected marker.
    pub selected_marker: Option<usize>,
    /// Marker being dragged this frame (index into `data.markers`).
    pub drag_marker: Option<usize>,
    /// Pending add-cue popup (set by double-clicking an empty area).
    pub pending_add: Option<PendingMarker>,
    /// Transient UI state for the add-cue popup.
    pub popup_new_kind: NewCueKind,
    pub popup_existing_cue: Option<u32>,

    /// Timestamp of the last re-rasterization (debounces zoom refinement).
    last_refine: Option<Instant>,
    /// Pending "bring this point into view" target `(page, x, y)` in PDF
    /// points, set when a cue fires elsewhere. Applied on the next panel frame
    /// once that page is rasterized, so the pan is computed from real pixel
    /// dimensions. `None` = nothing pending.
    pub pending_focus: Option<(usize, f32, f32)>,
    /// True when the Fit button was pressed; the next panel frame recomputes
    /// zoom/pan so the whole current page fills the canvas.
    pub pending_fit: bool,
    /// Last error message from a PDF load / rasterization attempt.
    pub error: Option<String>,
}

impl Default for ScriptViewer {
    fn default() -> Self {
        Self {
            data: ScriptViewerData::default(),
            pdfium: None,
            document: None,
            page_count: 0,
            pages: Vec::new(),
            current_page: 0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            edit_mode: false,
            selected_marker: None,
            drag_marker: None,
            pending_add: None,
            popup_new_kind: NewCueKind::Lighting,
            popup_existing_cue: None,
            last_refine: None,
            pending_focus: None,
            pending_fit: false,
            error: None,
        }
    }
}

impl ScriptViewer {
    // ── Library binding ──────────────────────────────────────────────────────

    /// Resolve the PDFium library and cache the binding for the process.
    /// Order: `PDFIUM_LIBRARY_PATH` env var → bundled copy → system libraries.
    fn pdfium(&mut self) -> Result<&'static Pdfium> {
        if let Some(p) = self.pdfium {
            return Ok(p);
        }

        let bindings = bind_pdfium_library()?;
        let pdfium: &'static Pdfium = Box::leak(Box::new(Pdfium::new(bindings)));
        self.pdfium = Some(pdfium);
        log::info!("[scriptviewer] PDFium library bound");
        Ok(pdfium)
    }

    /// True when a PDF is loaded and at least the current page is rendered.
    pub fn is_loaded(&self) -> bool {
        self.document.is_some() && self.page_count > 0
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// The rasterized image + texture for `index`, if cached.
    pub fn page_image(&self, index: usize) -> Option<&PageImage> {
        self.pages.get(index).and_then(|p| p.as_ref())
    }

    // ── Loading / caching ────────────────────────────────────────────────────

    /// Load a PDF and rasterize its first page. `ctx` is used to upload the
    /// page textures to the GPU. Any previous document is dropped first.
    pub fn load_pdf(&mut self, path: &std::path::Path, ctx: &egui::Context) -> Result<()> {
        let pdfium = self.pdfium()?;

        // Drop the old document and cache before loading the new one.
        self.document = None;
        self.pages.clear();
        self.page_count = 0;
        self.current_page = 0;
        self.selected_marker = None;
        self.pending_add = None;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.pending_focus = None;
        self.pending_fit = false;

        let document = pdfium.load_pdf_from_file(path, None)?;
        self.page_count = document.pages().len() as usize;
        self.document = Some(document);
        self.pages = (0..self.page_count).map(|_| None).collect();
        self.data.set_pdf_path(path.to_path_buf());

        if self.page_count > 0 {
            self.rasterize_page(ctx, 0)?;
        }

        log::info!(
            "[scriptviewer] Loaded {} ({} pages)",
            path.display(),
            self.page_count
        );
        self.error = None;
        Ok(())
    }

    /// Load the PDF referenced by `data.pdf_path` (resolved via the media dir).
    /// Returns true if a load attempt happened (success or not).
    pub fn load_pdf_from_data(&mut self, ctx: &egui::Context) -> Result<()> {
        let Some(rel) = self.data.pdf_path.clone() else {
            return Ok(());
        };
        let resolved = crate::paths::resolve_media_path(&rel);
        if !resolved.exists() {
            anyhow::bail!("Script PDF not found: {}", resolved.display());
        }
        self.load_pdf(&resolved, ctx)
    }

    /// Drop all runtime state (loaded document, page textures, view) but keep
    /// the persisted `data`. Called when a different show is loaded so the
    /// panel lazily re-loads the new show's script.
    pub fn reset_runtime(&mut self) {
        self.document = None;
        self.pages.clear();
        self.page_count = 0;
        self.current_page = 0;
        self.selected_marker = None;
        self.pending_add = None;
        self.drag_marker = None;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.last_refine = None;
        self.pending_focus = None;
        self.pending_fit = false;
        self.error = None;
    }

    /// Ensure the given page indices are rasterized, evicting pages outside the
    /// window `[current-1, current, current+1]` to bound memory.
    fn ensure_pages_rendered(&mut self, ctx: &egui::Context) -> Result<()> {
        if !self.is_loaded() {
            return Ok(());
        }
        // Build the keep-window: current ± 1 (clamped to the document).
        let lo = self.current_page.saturating_sub(1);
        let hi = (self.current_page + 1).min(self.page_count.saturating_sub(1));
        for i in 0..self.pages.len() {
            if i < lo || i > hi {
                self.pages[i] = None;
            }
        }
        for i in lo..=hi {
            if self.pages[i].is_none() {
                self.rasterize_page(ctx, i)?;
            }
        }
        Ok(())
    }

    /// Rasterize page `index` at the working resolution and upload as a texture.
    fn rasterize_page(&mut self, ctx: &egui::Context, index: usize) -> Result<()> {
        let Some(document) = &self.document else {
            return Ok(());
        };
        let page = document.pages().get(index as i32)?;
        let width_pts = page.width().value;
        let height_pts = page.height().value;
        let target = render_target_width(width_pts, height_pts, WORKING_WIDTH_PX as u32);
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target)
                .render_form_data(true)
                .render_annotations(true),
        )?;
        let img = upload_bitmap(ctx, bitmap, index, width_pts, height_pts)?;
        self.pages[index] = Some(img);
        Ok(())
    }

    /// If the current page is displayed far beyond its cached resolution,
    /// re-rasterize it at the display resolution (debounced). Called once per
    /// frame by the panel.
    pub fn refine_current_page(&mut self, ctx: &egui::Context) {
        if !self.is_loaded() || self.current_page >= self.page_count {
            return;
        }
        let Some(img) = self.pages.get(self.current_page).and_then(|p| p.as_ref()) else {
            return;
        };
        // Display width in screen pixels at the current zoom.
        let display_px = img.render_px_w as f32 * self.zoom;
        let is_zoom_high_enough = display_px > img.render_px_w as f32 * REFINE_ZOOM_THRESHOLD;
        let cooldown_elapsed = self
            .last_refine
            .map(|t| t.elapsed() >= REFINE_COOLDOWN)
            .unwrap_or(true);
        if !is_zoom_high_enough || !cooldown_elapsed {
            return;
        }
        let Some(document) = &self.document else {
            return;
        };
        let Ok(page) = document.pages().get(self.current_page as i32) else {
            return;
        };
        let width_pts = page.width().value;
        let height_pts = page.height().value;
        // Render at the display resolution, clamped to the hard cap (and to
        // egui's max texture side, enforced inside `render_target_width`).
        let target = render_target_width(
            width_pts,
            height_pts,
            (display_px as i32).clamp(WORKING_WIDTH_PX, MAX_RENDER_WIDTH_PX) as u32,
        );
        let bitmap = match page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target)
                .render_form_data(true)
                .render_annotations(true),
        ) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("[scriptviewer] refine render failed: {}", e);
                return;
            }
        };
        match upload_bitmap(ctx, bitmap, self.current_page, width_pts, height_pts) {
            Ok(img) => {
                self.pages[self.current_page] = Some(img);
                self.last_refine = Some(Instant::now());
                log::debug!(
                    "[scriptviewer] re-rasterized page {} at {}px",
                    self.current_page,
                    target
                );
            }
            Err(e) => log::warn!("[scriptviewer] refine upload failed: {}", e),
        }
    }

    /// Call after `current_page` changes to rasterize the new window lazily.
    pub fn page_changed(&mut self, ctx: &egui::Context) {
        if let Err(e) = self.ensure_pages_rendered(ctx) {
            self.error = Some(format!("Page rasterization failed: {}", e));
            log::warn!("[scriptviewer] {}", self.error.as_deref().unwrap_or(""));
        }
    }

    // ── Marker editing helpers (operate on the persisted data) ───────────────

    /// Index (into `data.markers`) of the first marker within `radius_px` of a
    /// screen position on the current page. `to_screen` maps page→screen.
    pub fn marker_at(
        &self,
        page_index: usize,
        screen_pos: egui::Pos2,
        radius_px: f32,
        to_screen: &dyn Fn(&CueMarker) -> egui::Pos2,
    ) -> Option<usize> {
        self.data.markers.iter().enumerate().find_map(|(idx, m)| {
            if m.page_index == page_index && to_screen(m).distance(screen_pos) <= radius_px {
                Some(idx)
            } else {
                None
            }
        })
    }

    pub fn add_marker(&mut self, marker: CueMarker) -> usize {
        self.data.markers.push(marker);
        self.data.markers.len() - 1
    }

    pub fn remove_marker(&mut self, index: usize) -> Option<CueMarker> {
        if index < self.data.markers.len() {
            Some(self.data.markers.remove(index))
        } else {
            None
        }
    }
}

/// Pick the render target width (px) for a page so that the *longer* rendered
/// dimension never exceeds egui's max texture side, while preferring `desired_px`.
/// Aspect ratio is preserved (rendering uses `set_target_width`).
fn render_target_width(width_pts: f32, height_pts: f32, desired_px: u32) -> i32 {
    let max_w_for_long_side = if height_pts > width_pts {
        MAX_TEXTURE_SIDE as f32 * (width_pts / height_pts)
    } else {
        MAX_TEXTURE_SIDE as f32
    };
    let target = (desired_px as f32).min(max_w_for_long_side).max(1.0);
    (target as u32).min(MAX_TEXTURE_SIDE) as i32
}

/// Upload a rendered pdfium bitmap as an egui GPU texture.
fn upload_bitmap(
    ctx: &egui::Context,
    bitmap: PdfBitmap<'_>,
    index: usize,
    width_pts: f32,
    height_pts: f32,
) -> Result<PageImage> {
    let w = bitmap.width() as usize;
    let h = bitmap.height() as usize;
    let rgba = bitmap.as_rgba_bytes();
    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
    let texture = ctx.load_texture(
        format!("scriptviewer_page_{}", index),
        image,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        },
    );
    Ok(PageImage {
        width_pts,
        height_pts,
        render_px_w: w as u32,
        texture,
    })
}

/// Locate and bind the PDFium native library. See the module docs for the
/// resolution order. Errors carry a human-readable hint so the UI can show the
/// operator how to install the library.
fn bind_pdfium_library() -> Result<PdfiumLibraryBindingsDyn> {
    // 1. Explicit env override: PDFIUM_LIBRARY_PATH=<path to libpdfium.so>
    if let Ok(path) = std::env::var("PDFIUM_LIBRARY_PATH") {
        if !path.is_empty() {
            let p = std::path::Path::new(&path);
            if p.exists() {
                return Pdfium::bind_to_library(p).map_err(|e| {
                    anyhow::anyhow!(
                        "Could not load PDFium from PDFIUM_LIBRARY_PATH '{}': {}",
                        path,
                        e
                    )
                });
            }
            log::warn!(
                "[scriptviewer] PDFIUM_LIBRARY_PATH set but '{}' does not exist — falling back",
                path
            );
        }
    }

    // 2. Bundled copy next to the executable / working directory.
    let lib_name = Pdfium::pdfium_platform_library_name();
    if let Some(found) = crate::paths::find_resource_file(std::path::Path::new(&lib_name)) {
        if let Ok(bindings) = Pdfium::bind_to_library(&found) {
            log::info!("[scriptviewer] Bound bundled PDFium at {}", found.display());
            return Ok(bindings);
        }
    }

    // 3. System library search (LD_LIBRARY_PATH, standard system paths, …).
    if let Ok(bindings) = Pdfium::bind_to_system_library() {
        return Ok(bindings);
    }

    anyhow::bail!(
        "PDFium library not found. Install libpdfium.so (e.g. via a distro \
         package or the bblanchon/pdfium-binaries release) on the system \
         library path, copy it next to the executable, or set \
         PDFIUM_LIBRARY_PATH to its location."
    )
}

/// Type alias to keep the binding return type readable.
type PdfiumLibraryBindingsDyn = Box<dyn PdfiumLibraryBindings>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_marker_round_trips_through_json() {
        let marker = CueMarker::new(2, 125.5, 733.25, 42);
        let json = serde_json::to_string(&marker).unwrap();
        let back: CueMarker = serde_json::from_str(&json).unwrap();
        assert_eq!(back, marker);
    }

    #[test]
    fn script_viewer_data_defaults_to_empty() {
        let data = ScriptViewerData::default();
        assert!(data.pdf_path.is_none());
        assert!(data.markers.is_empty());
    }

    #[test]
    fn script_viewer_data_loads_from_pre_feature_show_files() {
        // A show file fragment as older versions saved it — no script_viewer field.
        let json = r#"{"pdf_path": null, "markers": []}"#;
        let data: ScriptViewerData = serde_json::from_str(json).unwrap();
        assert!(data.pdf_path.is_none());
        assert!(data.markers.is_empty());

        // A fragment with markers.
        let json = r#"{"pdf_path": "script.pdf", "markers": [{"page_index": 0, "x": 1.0, "y": 2.0, "cue_id": 3}]}"#;
        let data: ScriptViewerData = serde_json::from_str(json).unwrap();
        assert_eq!(
            data.pdf_path.as_deref(),
            Some(std::path::Path::new("script.pdf"))
        );
        assert_eq!(data.markers.len(), 1);
        assert_eq!(data.markers[0].cue_id, 3);
    }

    #[test]
    fn set_pdf_path_strips_media_prefix_for_portability() {
        let mut data = ScriptViewerData::default();
        data.set_pdf_path(std::path::PathBuf::from("media/lorem.pdf"));
        assert_eq!(
            data.pdf_path.as_deref(),
            Some(std::path::Path::new("lorem.pdf"))
        );

        // Absolute / other paths are kept as-is.
        data.set_pdf_path(std::path::PathBuf::from("/tmp/other.pdf"));
        assert_eq!(
            data.pdf_path.as_deref(),
            Some(std::path::Path::new("/tmp/other.pdf"))
        );
    }

    /// End-to-end smoke test of PDF loading + rasterization. Requires a PDFium
    /// native library; skipped (passes trivially) when none is available so CI
    /// stays green. Point it at a library via `PDFIUM_LIBRARY_PATH` or by
    /// placing `libpdfium.so` on the library search path.
    #[test]
    fn loads_and_rasterizes_a_pdf() {
        // Discover the lorem.pdf fixture (repo root / working dir).
        let pdf = crate::paths::find_resource_file(std::path::Path::new("media/lorem.pdf"))
            .unwrap_or_else(|| std::path::PathBuf::from("media/lorem.pdf"));
        if !pdf.exists() {
            return;
        }

        // Confirm a PDFium library is reachable before attempting a load.
        let env_path = std::env::var("PDFIUM_LIBRARY_PATH")
            .ok()
            .filter(|p| !p.is_empty());
        let system_ok = Pdfium::bind_to_system_library().is_ok();
        let env_ok = env_path
            .as_deref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);
        if !system_ok && !env_ok {
            eprintln!("SKIP: no PDFium library available (set PDFIUM_LIBRARY_PATH to run)");
            return;
        }

        let ctx = egui::Context::default();
        let mut viewer = ScriptViewer::default();
        viewer
            .load_pdf(&pdf, &ctx)
            .expect("PDF should load and rasterize");

        assert!(viewer.is_loaded());
        assert!(viewer.page_count() >= 1);
        let img = viewer.page_image(0).expect("page 0 should be rasterized");
        assert!(img.width_pts > 0.0 && img.height_pts > 0.0);
        // Working-res render: width fits the cap, and the longer side stays
        // within egui's max texture side.
        assert!(img.render_px_w > 0);
        assert!(img.render_px_w <= WORKING_WIDTH_PX as u32);
        let long_px = img.render_px_w as f32 * (img.height_pts / img.width_pts).max(1.0);
        assert!(long_px <= MAX_TEXTURE_SIDE as f32);
    }
}
