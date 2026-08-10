//! Script Viewer panel — PDF script display with cue markers.
//!
//! Renders the page rasterized by `crate::scriptviewer::ScriptViewer` as a
//! zoomable/panable canvas and handles both operating modes:
//!
//! - **Edit**: double-click empty space to add a marker (link an existing cue
//!   or create a new one), click to select, drag to move, Delete to remove.
//! - **Playback**: markers are read-only; clicking one fires the linked cue
//!   (identical to a GO from the cue list).
//!
//! Marker coordinates are stored in PDF point space; the screen↔page transform
//! is recomputed here on every frame from the current zoom/pan.

use crate::app::{EasyCueApp, TabKind};
use crate::scriptviewer::{CueMarker, NewCueKind, PendingMarker};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// Hit radius (screen px) for marker selection/clicking.
const MARKER_RADIUS_PX: f32 = 14.0;
/// Zoom clamp bounds (also used by the +/- buttons and scroll zoom).
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 8.0;

/// Entry point called by the tab viewer.
pub fn render_script_viewer_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // ── Lazy-load the PDF referenced by the show file, if any ────────────────
    // Loading needs the egui context for texture upload; the show file's
    // `pdf_path` is only restored here, so this runs once per document.
    let needs_load = {
        let sv = &app.script_viewer;
        sv.data.pdf_path.is_some() && !sv.is_loaded() && sv.error.is_none()
    };
    if needs_load {
        if let Err(e) = app.script_viewer.load_pdf_from_data(ui.ctx()) {
            app.script_viewer.error = Some(e.to_string());
        }
    }

    // Snapshot immutable state to avoid long-lived borrows of `app`.
    let (pdf_path, is_loaded, page_count, current_page) = {
        let sv = &app.script_viewer;
        (
            sv.data.pdf_path.clone(),
            sv.is_loaded(),
            sv.page_count(),
            sv.current_page,
        )
    };

    // ── Toolbar ──────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        if ui.button("Open PDF…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF Script", &["pdf"])
                .set_title("Open PDF Script")
                .pick_file()
            {
                match app.script_viewer.load_pdf(&path, ui.ctx()) {
                    Ok(_) => {
                        app.ui_state.status_message = format!("Loaded script: {}", path.display());
                    }
                    Err(e) => {
                        app.script_viewer.error = Some(e.to_string());
                        app.ui_state.status_message = format!("Script load failed: {}", e);
                    }
                }
            }
        }

        // Mode toggle.
        let edit_mode = app.script_viewer.edit_mode;
        let label = if edit_mode {
            "✏ Edit"
        } else {
            "▶ Playback"
        };
        if ui
            .toggle_value(&mut app.script_viewer.edit_mode, label)
            .changed()
        {
            app.ui_state.status_message = if app.script_viewer.edit_mode {
                "Edit mode: double-click to add a cue marker".to_string()
            } else {
                "Playback mode: click a marker to fire its cue".to_string()
            };
        }
        ui.separator();

        // Page navigation + zoom.
        ui.add_enabled_ui(is_loaded, |ui| {
            if ui
                .small_button("◀")
                .on_hover_text("Previous page")
                .clicked()
            {
                app.script_viewer.current_page = app.script_viewer.current_page.saturating_sub(1);
            }
            ui.label(format!(
                "Page {} / {}",
                if page_count > 0 { current_page + 1 } else { 0 },
                page_count
            ));
            if ui.small_button("▶").on_hover_text("Next page").clicked() {
                let next = (app.script_viewer.current_page + 1).min(page_count.saturating_sub(1));
                app.script_viewer.current_page = next;
            }
            ui.separator();
            if ui.small_button("−").clicked() {
                app.script_viewer.zoom = (app.script_viewer.zoom * 0.8).max(MIN_ZOOM);
            }
            ui.label(format!("{:>3.0}%", app.script_viewer.zoom * 100.0));
            if ui.small_button("+").clicked() {
                app.script_viewer.zoom = (app.script_viewer.zoom * 1.25).min(MAX_ZOOM);
            }
            if ui
                .small_button("⟲ Fit")
                .on_hover_text("Fit whole page in view")
                .clicked()
            {
                app.script_viewer.pending_fit = true;
            }
        });

        // Filename, right-aligned.
        if let Some(p) = &pdf_path {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )
                    .small()
                    .color(Color32::from_gray(140)),
                );
            });
        }
    });

    // ── Marker editor strip (edit mode with a selection) ─────────────────────
    if app.script_viewer.edit_mode && app.script_viewer.selected_marker.is_some() {
        render_marker_editor_strip(ui, app);
        ui.separator();
    }

    // ── Error / empty-state message ──────────────────────────────────────────
    if let Some(err) = app.script_viewer.error.clone() {
        ui.colored_label(Color32::from_rgb(255, 120, 120), format!("⚠ {}", err));
        ui.separator();
    } else if !is_loaded {
        ui.colored_label(
            Color32::from_gray(120),
            "No script loaded — click \"Open PDF…\" to load a PDF.",
        );
        ui.separator();
    }

    // ── Canvas ───────────────────────────────────────────────────────────────
    let available = ui.available_size();
    let (canvas_rect, canvas_response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    // Keep page content steady when the canvas shifts vertically on screen — the
    // marker-editor strip appears above it the moment a marker is selected, which
    // (without compensation) would shove the just-placed dot below the click point.
    // The compensation keeps the page still, so the click lands at the dot's centre.
    app.script_viewer.compensate_canvas_shift(canvas_rect.top());

    if !is_loaded {
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(8, 22, 38));
        painter.text(
            canvas_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Script Viewer",
            egui::FontId::proportional(20.0),
            Color32::from_gray(80),
        );
        return;
    }

    // Keep the page window rasterized and refine resolution on deep zoom.
    app.script_viewer.page_changed(ui.ctx());
    app.script_viewer.refine_current_page(ui.ctx());

    // Copy the page image metadata out (TextureId is Copy) so the rest of the
    // frame can mutate `app.script_viewer` (zoom/pan/selection) freely.
    let (w_px, width_pts, height_pts, texture_id) = {
        let p = app.script_viewer.page_image(current_page);
        match p {
            Some(p) => (p.render_px_w, p.width_pts, p.height_pts, p.texture.id()),
            None => return,
        }
    };

    // ── Apply the Fit button: fit the whole page into the canvas, centred ────
    if app.script_viewer.pending_fit {
        let scale = (canvas_rect.width() / width_pts).min(canvas_rect.height() / height_pts);
        app.script_viewer.zoom = (scale * width_pts / w_px as f32).clamp(MIN_ZOOM, MAX_ZOOM);
        let disp_w = w_px as f32 * app.script_viewer.zoom;
        let disp_h = disp_w * (height_pts / width_pts);
        app.script_viewer.pan = egui::vec2(
            (canvas_rect.width() - disp_w) * 0.5,
            (canvas_rect.height() - disp_h) * 0.5,
        );
        app.script_viewer.pending_fit = false;
    }

    // ── Apply a pending "bring marker into view" focus ───────────────────────
    // Set when a cue fires elsewhere. Only jumps when the marker is NOT already
    // visible on screen; on the same page it recentres only when panned away.
    if let Some((fpage, fx, fy)) = app.script_viewer.pending_focus {
        if fpage != app.script_viewer.current_page {
            // Marker is on another page — must jump there. `page_changed` above
            // already ran for the old page, so it rasterizes next frame and the
            // focus is re-evaluated then.
            app.script_viewer.current_page = fpage;
        } else {
            let scale_here = w_px as f32 * app.script_viewer.zoom / width_pts;
            let screen_pos = Pos2::new(
                canvas_rect.min.x + app.script_viewer.pan.x + fx * scale_here,
                canvas_rect.min.y + app.script_viewer.pan.y + fy * scale_here,
            );
            if canvas_rect.contains(screen_pos) {
                app.script_viewer.pending_focus = None; // already visible — no jump
            } else {
                app.script_viewer.pan = egui::vec2(
                    canvas_rect.width() * 0.5 - fx * scale_here,
                    canvas_rect.height() * 0.5 - fy * scale_here,
                );
                app.script_viewer.pending_focus = None;
            }
        }
    }

    // ── Pan & zoom input ─────────────────────────────────────────────────────
    // Plain scroll (vertical or horizontal) pans the view; Ctrl/Cmd+scroll
    // zooms. Note: egui routes Ctrl+scroll into `zoom_delta()` (a multiplicative
    // factor), NOT into `smooth_scroll_delta`, so that's what we read here.
    let shift_held = ui.input(|i| i.modifiers.shift);
    if canvas_response.dragged_by(egui::PointerButton::Middle)
        || canvas_response.dragged_by(egui::PointerButton::Secondary)
        || (canvas_response.dragged_by(egui::PointerButton::Primary) && shift_held)
    {
        app.script_viewer.pan += canvas_response.drag_delta();
    }

    let (scroll_delta, ctrl_zoom) = ui.input(|i| {
        if i.modifiers.ctrl || i.modifiers.command {
            (Vec2::ZERO, i.zoom_delta())
        } else {
            (i.smooth_scroll_delta, 1.0)
        }
    });
    let pointer_over_canvas =
        canvas_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()));
    // Don't let the wheel pan/zoom the page while the add-cue popup is open or a
    // dropdown/menu (combo box, colour picker, …) is showing — the scroll should
    // go to that control, not to the PDF behind it.
    let popup_active =
        app.script_viewer.pending_add.is_some() || ui.memory(|m| m.any_popup_open());
    if pointer_over_canvas && !popup_active {
        if ctrl_zoom != 1.0 {
            app.script_viewer.zoom = (app.script_viewer.zoom * ctrl_zoom).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        if scroll_delta != Vec2::ZERO {
            app.script_viewer.pan += scroll_delta;
        }
    }

    // ── Keyboard page navigation (only when this panel is the active one) ────
    // Left/Right and PageUp/PageDown step through pages.
    let panel_active = app.ui_state.active_pane == Some(TabKind::ScriptViewer);
    let text_focused = ui.memory(|m| m.focused().is_some());
    if panel_active && !text_focused && !popup_active {
        let (left, right, page_up, page_down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
            )
        });
        if right || page_down {
            let next = (app.script_viewer.current_page + 1).min(page_count.saturating_sub(1));
            app.script_viewer.current_page = next;
        }
        if left || page_up {
            app.script_viewer.current_page = app.script_viewer.current_page.saturating_sub(1);
        }
    }

    // ── Draw the page (after all view mutations so the frame is consistent) ──
    let zoom = app.script_viewer.zoom;
    let pan = app.script_viewer.pan;
    let w = w_px as f32 * zoom;
    let h = w * (height_pts / width_pts);
    let page_rect = Rect::from_min_size(canvas_rect.min + pan, Vec2::new(w, h));

    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(8, 22, 38));
    painter.image(
        texture_id,
        page_rect,
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    painter.rect_stroke(
        page_rect,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(60, 90, 130)),
        egui::epaint::StrokeKind::Outside,
    );

    // ── Coordinate transforms (PDF points ↔ screen) — closures capture only
    //    copied locals, never `app`, so mutation elsewhere is unconstrained.
    let zoom = app.script_viewer.zoom;
    let pan = app.script_viewer.pan;
    let scale = w_px as f32 * zoom / width_pts;
    let to_screen = |x: f32, y: f32| {
        Pos2::new(
            canvas_rect.min.x + pan.x + x * scale,
            canvas_rect.min.y + pan.y + y * scale,
        )
    };
    let to_page = |pos: Pos2| {
        (
            (pos.x - canvas_rect.min.x - pan.x) / scale,
            (pos.y - canvas_rect.min.y - pan.y) / scale,
        )
    };

    // ── Interactions ─────────────────────────────────────────────────────────
    let edit_mode = app.script_viewer.edit_mode;

    if edit_mode {
        // Drag an existing marker (primary button).
        if canvas_response.drag_started_by(egui::PointerButton::Primary) && !shift_held {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                app.script_viewer.drag_marker = hit_marker(app, current_page, origin, &to_screen);
            }
        }
        if let Some(midx) = app.script_viewer.drag_marker {
            if canvas_response.dragged_by(egui::PointerButton::Primary) {
                if let Some(ptr) = canvas_response.interact_pointer_pos() {
                    let (px, py) = to_page(ptr);
                    if let Some(m) = app.script_viewer.data.markers.get_mut(midx) {
                        m.x = px.clamp(0.0, width_pts);
                        m.y = py.clamp(0.0, height_pts);
                    }
                }
            }
            if canvas_response.drag_stopped() {
                app.script_viewer.drag_marker = None;
            }
        }

        // Double-click empty space → add-cue popup.
        if canvas_response.double_clicked() && !shift_held {
            if let Some(pos) = canvas_response.interact_pointer_pos() {
                if hit_marker(app, current_page, pos, &to_screen).is_none() {
                    let (px, py) = to_page(pos);
                    app.script_viewer.pending_add = Some(PendingMarker {
                        page_index: current_page,
                        x: px.clamp(0.0, width_pts),
                        y: py.clamp(0.0, height_pts),
                    });
                    // Remembered action: focus the existing-cue combo if the last
                    // popup used it, otherwise the "Create & link" button so a
                    // repeat double-click is a keyboard-only flow.
                    app.script_viewer.popup_focus =
                        Some(if app.script_viewer.popup_last_was_link {
                            crate::scriptviewer::PopupFocusTarget::ExistingCombo
                        } else {
                            crate::scriptviewer::PopupFocusTarget::CreateButton
                        });
                }
            }
        }
        // Single click: select the marker (and its cue in the cue list), or
        // click the background to deselect everything.
        if canvas_response.clicked_by(egui::PointerButton::Primary) && !shift_held {
            if let Some(pos) = canvas_response.interact_pointer_pos() {
                match hit_marker(app, current_page, pos, &to_screen) {
                    Some(idx) => {
                        app.script_viewer.selected_marker = Some(idx);
                        if let Some(cue_id) =
                            app.script_viewer.data.markers.get(idx).map(|m| m.cue_id)
                        {
                            app.select_cue(cue_id);
                        }
                    }
                    None => {
                        app.script_viewer.selected_marker = None;
                        app.ui_state.selected_cue_id = None;
                        app.ui_state.selected_lighting_cue_id = None;
                        app.ui_state.selected_audio_cue_id = None;
                    }
                }
            }
        }

        // Delete selected marker while the canvas is hovered.
        let delete_pressed = ui.input(|i| i.key_pressed(egui::Key::Delete))
            && canvas_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()));
        if delete_pressed {
            if let Some(midx) = app.script_viewer.selected_marker {
                app.script_viewer.remove_marker(midx);
                app.script_viewer.selected_marker = None;
                app.ui_state.status_message = "Marker deleted".to_string();
            }
        }
    } else if canvas_response.clicked_by(egui::PointerButton::Primary) {
        // Playback: click a marker → fire its cue (GO behaviour).
        if let Some(pos) = canvas_response.interact_pointer_pos() {
            let cue_id = hit_marker(app, current_page, pos, &to_screen)
                .and_then(|i| app.script_viewer.data.markers.get(i))
                .map(|m| m.cue_id);
            if let Some(cue_id) = cue_id {
                app.fire_cue_by_id(cue_id);
            }
        }
    }

    // ── Draw markers ─────────────────────────────────────────────────────────
    draw_markers(&painter, app, current_page, &to_screen);

    // ── Add-cue popup ────────────────────────────────────────────────────────
    if app.script_viewer.pending_add.is_some() {
        render_add_cue_popup(ui.ctx(), app);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Marker drawing
// ─────────────────────────────────────────────────────────────────────────────

/// Index of the marker on `current_page` within hit radius of `pos`, if any.
fn hit_marker(
    app: &EasyCueApp,
    current_page: usize,
    pos: Pos2,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) -> Option<usize> {
    app.script_viewer
        .marker_at(current_page, pos, MARKER_RADIUS_PX, &|m: &CueMarker| {
            to_screen(m.x, m.y)
        })
}
fn draw_markers(
    painter: &egui::Painter,
    app: &EasyCueApp,
    current_page: usize,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) {
    // The marker whose cue is selected in the cue list (highlights regardless of
    // which panel initiated the selection, so all three views stay linked).
    let selected_cue = app.ui_state.selected_cue_id;

    // Collect this page's markers with their screen positions.
    let markers: Vec<(usize, &CueMarker, Pos2)> = app
        .script_viewer
        .data
        .markers
        .iter()
        .enumerate()
        .filter(|(_, m)| m.page_index == current_page)
        .map(|(idx, m)| (idx, m, to_screen(m.x, m.y)))
        .collect();

    // Colour by live cue status (fading/active/on-deck) falling back to the
    // kind's base colour, matching the cue list. Missing cues render grey.
    for (idx, marker, pos) in &markers {
        let (fill, text) = marker_color(app, marker.cue_id);
        let is_selected = app.script_viewer.edit_mode
            && (app.script_viewer.selected_marker == Some(*idx)
                || selected_cue == Some(marker.cue_id));

        // Visible ring slightly smaller than the (invisible) hit radius.
        let r = if is_selected {
            MARKER_RADIUS_PX + 4.0
        } else {
            MARKER_RADIUS_PX
        };
        painter.circle_filled(*pos, r, Color32::from_black_alpha(150));
        painter.circle_filled(*pos, r - 3.0, fill);
        painter.circle_stroke(
            *pos,
            r - 3.0,
            Stroke::new(if is_selected { 2.5 } else { 1.5 }, text),
        );

        // "2.0: Label" text to the right, vertically centred on the dot.
        let label = cue_short_label(app, marker.cue_id);
        let galley = painter.layout_no_wrap(label, egui::FontId::proportional(12.0), text);
        let pad = Vec2::new(5.0, 2.5);
        let label_tl = Pos2::new(pos.x + r + 3.0, pos.y - galley.size().y / 2.0);
        painter.rect_filled(
            Rect::from_min_size(label_tl - pad, galley.size() + pad * 2.0),
            3.0,
            Color32::from_black_alpha(120),
        );
        painter.galley(label_tl, galley, text);
    }
}

/// Fill + text colour for a marker, driven by the linked cue's live status:
/// fading → `status_fading`, active → `status_active`, on-deck → `status_on_deck`,
/// otherwise the cue kind's base colour — mirroring the Cue list rows. Missing
/// cues render grey so the operator can spot them.
fn marker_color(app: &EasyCueApp, cue_id: u32) -> (Color32, Color32) {
    use crate::app::EasyCueApp as App;

    let abs_idx = app.cue_list.cues().iter().position(|c| c.id == cue_id);
    let Some(abs_idx) = abs_idx else {
        return (Color32::from_rgb(90, 90, 100), Color32::WHITE);
    };
    let cue = app.cue_list.get_cue(abs_idx).expect("index just found");

    let is_lighting = cue.is_lighting();
    #[cfg(feature = "audio")]
    let is_audio = cue.is_audio();
    #[cfg(feature = "audio")]
    let is_adjust = cue.is_adjust();

    // Lighting: active while it's the play-head cue; fading while a fade runs.
    let lx_active_id = app.playback.current_cue_id();
    let is_lx_active = lx_active_id == Some(cue_id) && is_lighting;
    let is_lx_fading = is_lx_active && app.playback.fade_progress().is_some();

    // Audio: active while its stream plays; fading during in/out fades.
    #[cfg(feature = "audio")]
    let is_audio_active = is_audio && app.audio_playback.active_cue_ids().contains(&cue_id);
    #[cfg(not(feature = "audio"))]
    let is_audio_active = false;
    #[cfg(feature = "audio")]
    let is_audio_fading = is_audio_active
        && matches!(
            app.audio_playback.stream_state(cue_id),
            Some(
                crate::audio::AudioCueState::FadingIn { .. }
                    | crate::audio::AudioCueState::FadingOut { .. }
            )
        );
    #[cfg(not(feature = "audio"))]
    let is_audio_fading = false;

    // Adjust: active while its targeted stream has a per-route fade in progress.
    #[cfg(feature = "audio")]
    let is_adjust_active = if is_adjust && app.cue_list.current_index() == Some(abs_idx) {
        cue.adjust_data()
            .and_then(|d| {
                let target_id = d
                    .target_audio_cue
                    .and_then(|n| {
                        app.cue_list
                            .cues()
                            .iter()
                            .find(|c| (c.number - n).abs() < 0.005)
                            .map(|c| c.id)
                    })
                    .unwrap_or(0);
                app.audio_playback.volume_adjust_progress(target_id)
            })
            .is_some()
    } else {
        false
    };
    #[cfg(not(feature = "audio"))]
    let is_adjust_active = false;

    let is_active = is_lx_active || is_audio_active || is_adjust_active;
    let is_fading = is_lx_fading || is_audio_fading || is_adjust_active;
    let is_on_deck = app.cue_list.next_any_index() == Some(abs_idx);

    let base = match cue.kind {
        crate::cue::CueKind::Lighting(_) => App::color32_from_rgba(app.cue_colors.base_lighting),
        #[cfg(feature = "audio")]
        crate::cue::CueKind::Audio(_) => App::color32_from_rgba(app.cue_colors.base_audio),
        #[cfg(feature = "audio")]
        crate::cue::CueKind::Adjust(_) => App::color32_from_rgba(app.cue_colors.base_adjust),
    };
    let fill = if is_fading {
        App::color32_from_rgba(app.cue_colors.status_fading)
    } else if is_active {
        App::color32_from_rgba(app.cue_colors.status_active)
    } else if is_on_deck {
        App::color32_from_rgba(app.cue_colors.status_on_deck)
    } else {
        base
    };
    (fill, Color32::WHITE)
}

/// "2.0: Label" text for a marker (label omitted when empty), or a missing-cue
/// hint.
fn cue_short_label(app: &EasyCueApp, cue_id: u32) -> String {
    match app.cue_list.find_by_id(cue_id) {
        Some(c) => {
            if c.label.is_empty() {
                format!("{:.1}", c.number)
            } else {
                format!("{:.1}: {}", c.number, c.label)
            }
        }
        None => "?".to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Marker editor strip (edit mode, one marker selected)
// ─────────────────────────────────────────────────────────────────────────────

fn render_marker_editor_strip(ui: &mut Ui, app: &mut EasyCueApp) {
    let Some(midx) = app.script_viewer.selected_marker else {
        return;
    };
    let Some(marker) = app.script_viewer.data.markers.get(midx).copied() else {
        app.script_viewer.selected_marker = None;
        return;
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Marker:").strong());

        // Reassign the linked cue via a combo of every cue in the list.
        let cue_choices: Vec<(u32, String)> = app
            .cue_list
            .cues()
            .iter()
            .map(|c| (c.id, format!("{:.1} {}", c.number, c.label)))
            .collect();
        let current_label = cue_choices
            .iter()
            .find(|(id, _)| *id == marker.cue_id)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| "(cue missing)".to_string());
        let mut new_cue_id = marker.cue_id;
        egui::ComboBox::from_id_salt("script_marker_cue_combo")
            .selected_text(current_label)
            .width(180.0)
            .show_ui(ui, |ui| {
                for (id, label) in &cue_choices {
                    ui.selectable_value(&mut new_cue_id, *id, label);
                }
            });
        if new_cue_id != marker.cue_id {
            app.script_viewer.data.markers[midx].cue_id = new_cue_id;
            app.ui_state.status_message = "Marker relinked".to_string();
        }

        ui.separator();
        ui.label(
            egui::RichText::new(format!(
                "page {} · ({:.0}, {:.0}) pt",
                marker.page_index + 1,
                marker.x,
                marker.y
            ))
            .small()
            .color(Color32::from_gray(150)),
        );
        ui.label(
            egui::RichText::new("drag to move · Delete to remove")
                .small()
                .italics()
                .color(Color32::from_gray(120)),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Delete").clicked() {
                app.script_viewer.remove_marker(midx);
                app.script_viewer.selected_marker = None;
                app.ui_state.status_message = "Marker deleted".to_string();
            }
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Add-cue popup (double-click on empty page space)
// ─────────────────────────────────────────────────────────────────────────────

fn render_add_cue_popup(ctx: &egui::Context, app: &mut EasyCueApp) {
    let Some(pending) = app.script_viewer.pending_add else {
        return;
    };

    let mut close = false;
    let mut created_id: Option<u32> = None;
    // True when the resolved cue was freshly created here (vs. linked to an
    // existing one). New cues get selected + their label focused so the
    // operator can start typing a name right away.
    let mut created_new = false;

    // Consumed on this frame so the focus request only applies once on open.
    let focus_target = app.script_viewer.popup_focus.take();

    egui::Window::new("Add Cue Marker")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "Page {} at ({:.0}, {:.0}) pt",
                pending.page_index + 1,
                pending.x,
                pending.y
            ));
            ui.add_space(6.0);

            // ── Link an existing cue ───────────────────────────────────────
            let cue_choices: Vec<(u32, String)> = app
                .cue_list
                .cues()
                .iter()
                .map(|c| (c.id, format!("{:.1} {}", c.number, c.label)))
                .collect();
            let existing_label = app
                .script_viewer
                .popup_existing_cue
                .and_then(|id| cue_choices.iter().find(|(cid, _)| *cid == id))
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| "(select cue)".to_string());
            let existing_combo = egui::ComboBox::from_id_salt("script_add_existing")
                .selected_text(existing_label)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (id, label) in &cue_choices {
                        ui.selectable_value(
                            &mut app.script_viewer.popup_existing_cue,
                            Some(*id),
                            label,
                        );
                    }
                });
            if focus_target == Some(crate::scriptviewer::PopupFocusTarget::ExistingCombo) {
                existing_combo.response.request_focus();
            }
            if ui
                .add_enabled(
                    app.script_viewer.popup_existing_cue.is_some(),
                    egui::Button::new("Link existing cue"),
                )
                .clicked()
            {
                if let Some(cid) = app.script_viewer.popup_existing_cue {
                    created_id = Some(cid);
                    close = true;
                }
            }
            ui.separator();

            // ── Create a new cue inline ────────────────────────────────────
            ui.label(
                egui::RichText::new("…or create a new cue:")
                    .small()
                    .color(Color32::from_gray(150)),
            );
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut app.script_viewer.popup_new_kind,
                    NewCueKind::Lighting,
                    "Lighting",
                );
                #[cfg(feature = "audio")]
                ui.selectable_value(
                    &mut app.script_viewer.popup_new_kind,
                    NewCueKind::Sound,
                    "Sound",
                );
                #[cfg(feature = "audio")]
                ui.selectable_value(
                    &mut app.script_viewer.popup_new_kind,
                    NewCueKind::Adjustment,
                    "Adjustment",
                );
            });

            let kind = app.script_viewer.popup_new_kind;
            if kind != NewCueKind::Lighting {
                let hint = if kind == NewCueKind::Sound {
                    "Sound cues need an audio file — you'll pick one on create."
                } else {
                    "Adjustment cues target the most recent sound cue."
                };
                ui.label(
                    egui::RichText::new(hint)
                        .small()
                        .italics()
                        .color(Color32::from_gray(140)),
                );
            }

            ui.horizontal(|ui| {
                let create_btn = ui.button("Create & link");
                if focus_target == Some(crate::scriptviewer::PopupFocusTarget::CreateButton) {
                    create_btn.request_focus();
                }
                if create_btn.clicked() {
                    match kind {
                        NewCueKind::Lighting => {
                            created_id = app.add_cue_of_kind(kind);
                            created_new = created_id.is_some();
                            close = created_id.is_some();
                        }
                        #[cfg(feature = "audio")]
                        NewCueKind::Sound => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter(
                                    "Audio Files",
                                    &["mp3", "wav", "flac", "ogg", "aac", "m4a"],
                                )
                                .set_title("Select Audio File")
                                .pick_file()
                            {
                                let id = app.add_cue_of_kind(kind);
                                if let Some(id) = id {
                                    if let Some(idx) =
                                        app.cue_list.cues().iter().position(|c| c.id == id)
                                    {
                                        if let Some(c) = app.cue_list.get_cue_mut(idx) {
                                            if let Some(d) = c.audio_data_mut() {
                                                d.set_path(path);
                                            }
                                        }
                                    }
                                    created_id = Some(id);
                                    created_new = true;
                                    close = true;
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        NewCueKind::Adjustment => {
                            created_id = app.add_cue_of_kind(kind);
                            created_new = created_id.is_some();
                            close = created_id.is_some();
                        }
                        #[cfg(not(feature = "audio"))]
                        NewCueKind::Sound | NewCueKind::Adjustment => {}
                    }
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });

    if close {
        app.script_viewer.pending_add = None;
        // Remember what the operator did so the next popup can pre-select and
        // pre-focus the same control. `popup_existing_cue` is intentionally
        // kept so a repeat "link existing" flow shows the last linked cue.
        if let Some(cid) = created_id {
            if created_new {
                app.script_viewer.popup_last_was_link = false;
            } else {
                app.script_viewer.popup_last_was_link = true;
                app.script_viewer.popup_existing_cue = Some(cid);
            }
        }
    }

    // Attach a marker to the chosen cue once the popup resolves.
    if let Some(cue_id) = created_id {
        let marker = CueMarker::new(pending.page_index, pending.x, pending.y, cue_id);
        let idx = app.script_viewer.add_marker(marker);
        app.script_viewer.selected_marker = Some(idx);
        // A freshly created cue gets selected and its label field focused so
        // the operator can name it immediately (mirrors the "+LX" cue button).
        if created_new {
            app.select_cue(cue_id);
            app.ui_state.focus_cue_edit = Some((cue_id, crate::app::CueEditField::Label));
        }
        app.ui_state.status_message = format!("Marker linked to cue {}", cue_id);
    }
}
