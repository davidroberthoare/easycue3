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

use crate::app::EasyCueApp;
use crate::scriptviewer::{CueMarker, NewCueKind, PendingMarker};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// Hit radius (screen px) for marker selection/clicking.
const MARKER_RADIUS_PX: f32 = 10.0;

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
                app.script_viewer.zoom = (app.script_viewer.zoom * 0.8).max(0.1);
            }
            ui.label(format!("{:>3.0}%", app.script_viewer.zoom * 100.0));
            if ui.small_button("+").clicked() {
                app.script_viewer.zoom = (app.script_viewer.zoom * 1.25).min(8.0);
            }
            if ui
                .small_button("⟲ Fit")
                .on_hover_text("Reset zoom & pan")
                .clicked()
            {
                app.script_viewer.zoom = 1.0;
                app.script_viewer.pan = Vec2::ZERO;
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

    // ── Pan & zoom input (mirrors the magic sheet canvas) ────────────────────
    let shift_held = ui.input(|i| i.modifiers.shift);
    if canvas_response.dragged_by(egui::PointerButton::Middle)
        || canvas_response.dragged_by(egui::PointerButton::Secondary)
        || (canvas_response.dragged_by(egui::PointerButton::Primary) && shift_held)
    {
        app.script_viewer.pan += canvas_response.drag_delta();
    }

    let (pan_delta, zoom_delta) = ui.input(|i| {
        if i.modifiers.shift {
            (i.smooth_scroll_delta, 0.0f32)
        } else {
            (Vec2::ZERO, i.smooth_scroll_delta.y)
        }
    });
    if canvas_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
        if zoom_delta != 0.0 {
            app.script_viewer.zoom =
                (app.script_viewer.zoom * (1.0 + zoom_delta * 0.001)).clamp(0.1, 8.0);
        }
        if pan_delta != Vec2::ZERO {
            app.script_viewer.pan += pan_delta;
        }
    }

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

        // Double-click empty space → add-cue popup; single click → select.
        if canvas_response.double_clicked() && !shift_held {
            if let Some(pos) = canvas_response.interact_pointer_pos() {
                if hit_marker(app, current_page, pos, &to_screen).is_none() {
                    let (px, py) = to_page(pos);
                    app.script_viewer.pending_add = Some(PendingMarker {
                        page_index: current_page,
                        x: px.clamp(0.0, width_pts),
                        y: py.clamp(0.0, height_pts),
                    });
                }
            }
        } else if canvas_response.clicked_by(egui::PointerButton::Primary) && !shift_held {
            if let Some(pos) = canvas_response.interact_pointer_pos() {
                app.script_viewer.selected_marker = hit_marker(app, current_page, pos, &to_screen);
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

    // Colour by cue type using the user's cue colour settings.
    for (idx, marker, pos) in &markers {
        let (fill, text) = marker_color(app, marker.cue_id);
        let is_selected =
            app.script_viewer.edit_mode && app.script_viewer.selected_marker == Some(*idx);

        // Visible ring slightly smaller than the (invisible) hit radius.
        let r = if is_selected {
            MARKER_RADIUS_PX + 3.0
        } else {
            MARKER_RADIUS_PX
        };
        painter.circle_filled(*pos, r, Color32::from_black_alpha(140));
        painter.circle_filled(*pos, r - 2.0, fill);
        painter.circle_stroke(
            *pos,
            r - 2.0,
            Stroke::new(if is_selected { 2.5 } else { 1.5 }, text),
        );

        // Cue number label to the right, with a backdrop for legibility.
        let label = cue_short_label(app, marker.cue_id);
        let label_pos = Pos2::new(pos.x + r + 2.0, pos.y);
        let galley = painter.layout_no_wrap(label, egui::FontId::proportional(12.0), text);
        painter.rect_filled(
            Rect::from_min_size(
                label_pos + Vec2::new(-2.0, -galley.size().y / 2.0 - 1.0),
                galley.size() + Vec2::new(4.0, 2.0),
            ),
            2.0,
            Color32::from_black_alpha(120),
        );
        painter.galley(label_pos, galley, text);
    }
}

/// Fill + text colour for a marker, colour-coded by cue type (reuses the user's
/// cue colour settings). Missing cues render grey so the operator can spot them.
fn marker_color(app: &EasyCueApp, cue_id: u32) -> (Color32, Color32) {
    use crate::app::EasyCueApp as App;
    let base = match app.cue_list.find_by_id(cue_id) {
        None => Color32::from_rgb(90, 90, 100),
        Some(c) => match c.kind {
            crate::cue::CueKind::Lighting(_) => {
                App::color32_from_rgba(app.cue_colors.base_lighting)
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => App::color32_from_rgba(app.cue_colors.base_audio),
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(_) => App::color32_from_rgba(app.cue_colors.base_adjust),
        },
    };
    (base, Color32::WHITE)
}

/// "Q12.3" style short label for a marker, or a missing-cue hint.
fn cue_short_label(app: &EasyCueApp, cue_id: u32) -> String {
    match app.cue_list.find_by_id(cue_id) {
        Some(c) => format!("{:.1}", c.number),
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
            if ui.button("Deselect").clicked() {
                app.script_viewer.selected_marker = None;
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
            egui::ComboBox::from_id_salt("script_add_existing")
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
                if ui.button("Create & link").clicked() {
                    match kind {
                        NewCueKind::Lighting => {
                            created_id = app.add_cue_of_kind(kind);
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
                                    close = true;
                                }
                            }
                        }
                        #[cfg(feature = "audio")]
                        NewCueKind::Adjustment => {
                            created_id = app.add_cue_of_kind(kind);
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
        app.script_viewer.popup_existing_cue = None;
    }

    // Attach a marker to the chosen cue once the popup resolves.
    if let Some(cue_id) = created_id {
        let marker = CueMarker::new(pending.page_index, pending.x, pending.y, cue_id);
        let idx = app.script_viewer.add_marker(marker);
        app.script_viewer.selected_marker = Some(idx);
        app.ui_state.status_message = format!("Marker linked to cue {}", cue_id);
    }
}
