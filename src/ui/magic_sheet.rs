//! Magic sheet panel — freeform fixture-layout canvas
//!
//! Edit mode: place and reposition shapes, assign fixtures, set colours.
//! Live mode: click/drag shapes to select fixtures and adjust intensity,
//!            kept in sync with the Channels panel via `app.ui_state.selected_fixtures`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use crate::app::EasyCueApp;
use crate::magic_sheet::ShapeKind;
use crate::fixtures::profiles::FixtureParameter;
use super::channels::update_command_from_fixture_selection;

/// Base shape dimensions in canvas-space logical pixels (before scale).
const BASE_W: f32 = 80.0;
const BASE_H: f32 = 60.0;

/// Entry point called by the tab viewer.
pub fn render_magic_sheet_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    let edit_mode = app.magic_sheet_state.edit_mode;

    // ── Toolbar ─────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let label = if edit_mode { "✏ Edit" } else { "▶ Live" };
        ui.toggle_value(&mut app.magic_sheet_state.edit_mode, label);

        if app.magic_sheet_state.edit_mode {
            ui.separator();

            // Shape palette — click to add a shape at canvas centre and immediately select it
            ui.label("Add:");
            for kind in crate::magic_sheet::ALL_SHAPE_KINDS {
                if ui.button(kind.to_string()).clicked() {
                    let canvas_centre = canvas_centre(ui, app);
                    let new_id = app.magic_sheet.add_shape(kind.clone(), [canvas_centre.x, canvas_centre.y]);
                    app.magic_sheet_state.selected_shape_ids.clear();
                    app.magic_sheet_state.selected_shape_ids.insert(new_id);
                }
            }

            ui.separator();

            // Copy
            let has_sel = !app.magic_sheet_state.selected_shape_ids.is_empty();
            if ui.add_enabled(has_sel, egui::Button::new("⎘ Copy")).clicked() {
                app.magic_sheet_state.clipboard = app.magic_sheet.shapes.iter()
                    .filter(|s| app.magic_sheet_state.selected_shape_ids.contains(&s.id))
                    .cloned()
                    .collect();
            }

            // Paste
            if !app.magic_sheet_state.clipboard.is_empty() && ui.button("⎙ Paste").clicked() {
                let offset = 20.0;
                let new_ids: Vec<u32> = app.magic_sheet_state.clipboard.iter().map(|s| {
                    app.magic_sheet.add_shape_at(s.kind.clone(), [s.pos[0] + offset, s.pos[1] + offset], s.scale, s.bg_color, s.outline_color, s.fixture_id)
                }).collect();
                app.magic_sheet_state.selected_shape_ids = new_ids.into_iter().collect();
            }

            ui.separator();

            // Alignment — shown when 2+ shapes are selected
            let sel_count = app.magic_sheet_state.selected_shape_ids.len();
            if sel_count >= 2 {
                ui.label("Align:");
                if ui.small_button("⬅").on_hover_text("Align left edges").clicked() {
                    align_shapes(app, Alignment::Left);
                }
                if ui.small_button("➡").on_hover_text("Align right edges").clicked() {
                    align_shapes(app, Alignment::Right);
                }
                if ui.small_button("⬆").on_hover_text("Align top edges").clicked() {
                    align_shapes(app, Alignment::Top);
                }
                if ui.small_button("⬇").on_hover_text("Align bottom edges").clicked() {
                    align_shapes(app, Alignment::Bottom);
                }
                if ui.small_button("↔").on_hover_text("Distribute horizontally").clicked() {
                    align_shapes(app, Alignment::DistributeH);
                }
                if ui.small_button("↕").on_hover_text("Distribute vertically").clicked() {
                    align_shapes(app, Alignment::DistributeV);
                }
                ui.separator();
            }

            // Delete selected shapes
            if has_sel && ui.button("🗑 Delete").clicked() {
                let to_delete: Vec<u32> = app.magic_sheet_state.selected_shape_ids.iter().copied().collect();
                for id in to_delete {
                    app.magic_sheet.remove_shape(id);
                }
                app.magic_sheet_state.selected_shape_ids.clear();
            }
        }

        // Reset view button (always visible)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("⟲ Reset View").clicked() {
                app.magic_sheet_state.canvas_offset = Vec2::ZERO;
                app.magic_sheet_state.canvas_zoom = 1.0;
            }
        });
    });

    ui.separator();

    // ── Properties side panel: always open in edit mode, closed in live mode ──
    if app.magic_sheet_state.edit_mode {
        egui::SidePanel::right("magic_sheet_props")
            .resizable(true)
            .min_width(180.0)
            .default_width(200.0)
            .show_inside(ui, |ui| {
                let sel_ids: Vec<u32> = app.magic_sheet_state.selected_shape_ids.iter().copied().collect();
                match sel_ids.len() {
                    0 => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("No Selection").color(egui::Color32::GRAY));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Click a shape to edit").small());
                        });
                    }
                    1 => render_shape_properties(ui, app, sel_ids[0]),
                    _ => render_multi_shape_properties(ui, app, &sel_ids),
                }
            });
    }

    // ── Canvas area ──────────────────────────────────────────────────────────
    let available = ui.available_size();
    let (canvas_rect, canvas_response) =
        ui.allocate_exact_size(available, Sense::click_and_drag());

    // ── Pan: middle-click drag, right-click drag, OR shift+left-drag ─────────
    let shift_held = ui.input(|i| i.modifiers.shift);

    if canvas_response.dragged_by(egui::PointerButton::Middle)
        || canvas_response.dragged_by(egui::PointerButton::Secondary)
        || (canvas_response.dragged_by(egui::PointerButton::Primary) && shift_held)
    {
        app.magic_sheet_state.canvas_offset += canvas_response.drag_delta();
    }

    // ── Zoom: scroll wheel when not shift-held; shift+scroll = pan both axes ──
    let (pan_delta, zoom_delta) = ui.input(|i| {
        if i.modifiers.shift {
            (i.smooth_scroll_delta, 0.0f32)
        } else {
            (Vec2::ZERO, i.smooth_scroll_delta.y)
        }
    });

    if canvas_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
        if zoom_delta != 0.0 {
            let zoom_factor = 1.0 + zoom_delta * 0.001;
            app.magic_sheet_state.canvas_zoom =
                (app.magic_sheet_state.canvas_zoom * zoom_factor).clamp(0.1, 5.0);
        }
        if pan_delta != Vec2::ZERO {
            app.magic_sheet_state.canvas_offset += pan_delta;
        }
    }

    // ── Background ───────────────────────────────────────────────────────────
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(8, 22, 38));
    draw_dot_grid(&painter, canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom);

    // ── Shapes ───────────────────────────────────────────────────────────────
    let shapes_snapshot: Vec<_> = app.magic_sheet.shapes.iter().map(|s| {
        let fixture_id = s.fixture_id;
        let kind = s.kind.clone();
        let pos = s.pos;
        let scale = s.scale;
        let bg = Color32::from_rgba_unmultiplied(s.bg_color[0], s.bg_color[1], s.bg_color[2], s.bg_color[3]);
        let outline = Color32::from_rgba_unmultiplied(s.outline_color[0], s.outline_color[1], s.outline_color[2], s.outline_color[3]);
        let id = s.id;
        let link_color = s.link_color;
        let link_intensity = s.link_intensity;
        (id, kind, pos, scale, bg, outline, fixture_id, link_color, link_intensity)
    }).collect();

    let mut drag_started_on_shape = false;

    for (shape_id, kind, pos, scale, bg_color, outline_color, fixture_id, link_color, link_intensity)
        in &shapes_snapshot
    {
        let shape_id = *shape_id;
        let scale = *scale;

        let screen_center = canvas_to_screen(canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom, *pos);
        let w = BASE_W * scale * app.magic_sheet_state.canvas_zoom;
        let h = BASE_H * scale * app.magic_sheet_state.canvas_zoom;

        let (label, fix_num, intensity, rgb) = fixture_info(app, *fixture_id);

        // ── Live linking: compute effective fill from fixture state ───────────
        let effective_bg = if !edit_mode && fixture_id.is_some() && (*link_color || *link_intensity) {
            let base = if *link_color { rgb.unwrap_or(*bg_color) } else { *bg_color };
            if *link_intensity {
                let [r, g, b, a] = base.to_srgba_unmultiplied();
                Color32::from_rgba_unmultiplied(
                    (r as f32 * intensity).round() as u8,
                    (g as f32 * intensity).round() as u8,
                    (b as f32 * intensity).round() as u8,
                    a,
                )
            } else {
                base
            }
        } else {
            *bg_color
        };

        let is_selected_shape = edit_mode
            && app.magic_sheet_state.selected_shape_ids.contains(&shape_id);
        let is_selected_fixture = !edit_mode && fixture_id
            .map(|fid| app.ui_state.selected_fixtures.contains(&fid))
            .unwrap_or(false);

        let shape_rect = Rect::from_center_size(screen_center, egui::vec2(w, h));
        let resp = ui.allocate_rect(shape_rect, Sense::click_and_drag());

        // ── Edit mode: click to select, drag to move ──────────────────────────
        if edit_mode && !shift_held && app.magic_sheet_state.drag_select_start.is_none() {
            if resp.drag_started() {
                drag_started_on_shape = true;
            }
            if resp.clicked() {
                let modifiers = ui.input(|i| i.modifiers);
                if modifiers.command || modifiers.ctrl {
                    if app.magic_sheet_state.selected_shape_ids.contains(&shape_id) {
                        app.magic_sheet_state.selected_shape_ids.remove(&shape_id);
                    } else {
                        app.magic_sheet_state.selected_shape_ids.insert(shape_id);
                    }
                } else {
                    app.magic_sheet_state.selected_shape_ids.clear();
                    app.magic_sheet_state.selected_shape_ids.insert(shape_id);
                }
            }
            if resp.dragged() && is_selected_shape {
                drag_started_on_shape = true;
                let delta = resp.drag_delta() / app.magic_sheet_state.canvas_zoom;
                let selected: Vec<u32> = app.magic_sheet_state.selected_shape_ids.iter().copied().collect();
                for sid in selected {
                    if let Some(s) = app.magic_sheet.get_shape_mut(sid) {
                        s.pos[0] += delta.x;
                        s.pos[1] += delta.y;
                    }
                }
            }
        } else if !edit_mode && !shift_held {
            // ── Live mode: click to select fixture ────────────────────────────
            if resp.clicked() {
                if let Some(fid) = fixture_id {
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.command || modifiers.ctrl {
                        if app.ui_state.selected_fixtures.contains(fid) {
                            app.ui_state.selected_fixtures.remove(fid);
                        } else {
                            app.ui_state.selected_fixtures.insert(*fid);
                        }
                    } else {
                        app.ui_state.selected_fixtures.clear();
                        app.ui_state.selected_fixtures.insert(*fid);
                    }
                    app.ui_state.last_selected_fixture = *fixture_id;
                    update_command_from_fixture_selection(app);
                }
            }

            // ── Live mode: drag adjusts intensity on all selected fixtures ────
            if resp.dragged() {
                if let Some(fid) = fixture_id {
                    let dy = resp.drag_delta().y;
                    if dy.abs() > 0.5 {
                        if !app.ui_state.selected_fixtures.contains(fid) {
                            app.ui_state.selected_fixtures.clear();
                            app.ui_state.selected_fixtures.insert(*fid);
                            app.ui_state.last_selected_fixture = *fixture_id;
                        }
                        let delta = (-dy / h).clamp(-1.0, 1.0);
                        adjust_selected_fixtures_intensity(app, delta);
                    }
                }
            }
        }

        // ── Draw shape ───────────────────────────────────────────────────────
        let highlight = is_selected_shape || is_selected_fixture;
        let border_color = if highlight { Color32::from_rgb(100, 180, 255) } else { *outline_color };
        let border_width = if highlight { 2.5 } else { 1.5 };

        draw_shape(&painter, kind, screen_center, w, h, effective_bg, Stroke::new(border_width, border_color));
        draw_shape_label(&painter, screen_center, w, h, &label, fix_num, intensity, rgb);
    }

    // ── Rubber-band selection (edit mode, no shift, drag on empty canvas) ────
    if edit_mode && !shift_held {
        // Start rubber band when drag begins on empty canvas
        if canvas_response.drag_started() && !drag_started_on_shape {
            app.magic_sheet_state.drag_select_start =
                ui.input(|i| i.pointer.press_origin());
        }

        if let Some(start) = app.magic_sheet_state.drag_select_start {
            let current = ui.input(|i| i.pointer.interact_pos().unwrap_or(start));
            let sel_rect = Rect::from_two_pos(start, current);

            // Draw the rubber band rect
            painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(50, 120, 220, 25));
            painter.rect_stroke(sel_rect, 0.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 200)), egui::epaint::StrokeKind::Outside);

            // On drag release: commit selection
            if canvas_response.drag_stopped() {
                let modifiers = ui.input(|i| i.modifiers);
                if !modifiers.command && !modifiers.ctrl {
                    app.magic_sheet_state.selected_shape_ids.clear();
                }
                for (shape_id, _kind, pos, scale, ..) in &shapes_snapshot {
                    let sc = canvas_to_screen(canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom, *pos);
                    let w = BASE_W * scale * app.magic_sheet_state.canvas_zoom;
                    let h = BASE_H * scale * app.magic_sheet_state.canvas_zoom;
                    let shape_rect = Rect::from_center_size(sc, egui::vec2(w, h));
                    if sel_rect.intersects(shape_rect) {
                        app.magic_sheet_state.selected_shape_ids.insert(*shape_id);
                    }
                }
                app.magic_sheet_state.drag_select_start = None;
            }
        }

        // Click on empty canvas (no shift, no rubber-band release): deselect all
        if canvas_response.clicked() && app.magic_sheet_state.drag_select_start.is_none() {
            app.magic_sheet_state.selected_shape_ids.clear();
        }
    }

    // ── Rubber-band fixture selection (live mode, no shift) ───────────────────
    if !edit_mode && !shift_held {
        if canvas_response.drag_started() && !ui.input(|i| {
            i.pointer.press_origin()
                .map(|p| shapes_snapshot.iter().any(|(_, _, pos, scale, ..)| {
                    let sc = canvas_to_screen(canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom, *pos);
                    let w = BASE_W * scale * app.magic_sheet_state.canvas_zoom;
                    let h = BASE_H * scale * app.magic_sheet_state.canvas_zoom;
                    Rect::from_center_size(sc, egui::vec2(w, h)).contains(p)
                }))
                .unwrap_or(false)
        }) {
            app.magic_sheet_state.drag_select_start = ui.input(|i| i.pointer.press_origin());
        }

        if let Some(start) = app.magic_sheet_state.drag_select_start {
            let current = ui.input(|i| i.pointer.interact_pos().unwrap_or(start));
            let sel_rect = Rect::from_two_pos(start, current);
            painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(50, 180, 120, 25));
            painter.rect_stroke(sel_rect, 0.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(80, 220, 150, 200)), egui::epaint::StrokeKind::Outside);

            if canvas_response.drag_stopped() {
                let modifiers = ui.input(|i| i.modifiers);
                if !modifiers.command && !modifiers.ctrl {
                    app.ui_state.selected_fixtures.clear();
                }
                for (_, _, pos, scale, _, _, fixture_id, ..) in &shapes_snapshot {
                    if let Some(fid) = fixture_id {
                        let sc = canvas_to_screen(canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom, *pos);
                        let w = BASE_W * scale * app.magic_sheet_state.canvas_zoom;
                        let h = BASE_H * scale * app.magic_sheet_state.canvas_zoom;
                        if sel_rect.intersects(Rect::from_center_size(sc, egui::vec2(w, h))) {
                            app.ui_state.selected_fixtures.insert(*fid);
                        }
                    }
                }
                if !app.ui_state.selected_fixtures.is_empty() {
                    update_command_from_fixture_selection(app);
                }
                app.magic_sheet_state.drag_select_start = None;
            }
        }

        // Click empty canvas in live mode: deselect fixtures
        if canvas_response.clicked() && app.magic_sheet_state.drag_select_start.is_none() {
            app.ui_state.selected_fixtures.clear();
            update_command_from_fixture_selection(app);
        }
    }

    // ── Sync canvas state back to show file (for persistence) ────────────────
    app.magic_sheet.canvas_offset = [
        app.magic_sheet_state.canvas_offset.x,
        app.magic_sheet_state.canvas_offset.y,
    ];
    app.magic_sheet.canvas_zoom = app.magic_sheet_state.canvas_zoom;

    // ── Empty-state hint ─────────────────────────────────────────────────────
    if app.magic_sheet.shapes.is_empty() {
        painter.text(
            canvas_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Magic Sheet — switch to Edit Mode and add shapes.",
            egui::FontId::proportional(14.0),
            Color32::from_gray(80),
        );
    }
}

// ── Properties panel — single shape ──────────────────────────────────────────

fn render_shape_properties(ui: &mut Ui, app: &mut EasyCueApp, shape_id: u32) {
    ui.heading("Shape Properties");
    ui.separator();

    let shape = match app.magic_sheet.shapes.iter().find(|s| s.id == shape_id) {
        Some(s) => s,
        None => { ui.label("(shape not found)"); return; }
    };

    let mut fixture_id = shape.fixture_id;
    let mut scale = shape.scale;
    let mut bg = Color32::from_rgba_unmultiplied(shape.bg_color[0], shape.bg_color[1], shape.bg_color[2], shape.bg_color[3]);
    let mut outline = Color32::from_rgba_unmultiplied(shape.outline_color[0], shape.outline_color[1], shape.outline_color[2], shape.outline_color[3]);
    let mut link_color = shape.link_color;
    let mut link_intensity = shape.link_intensity;

    egui::Grid::new("shape_props_grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Fixture:");
            let patches: Vec<_> = app.fixtures.patch_list().patches().to_vec();
            let selected_label = fixture_id
                .and_then(|fid| patches.iter().find(|p| p.id == fid))
                .map(|p| format!("#{} {}", p.id, p.label))
                .unwrap_or_else(|| "(none)".to_string());

            egui::ComboBox::from_id_salt("shape_fixture_combo")
                .selected_text(&selected_label)
                .width(130.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut fixture_id, None, "(none)").clicked() {}
                    for patch in &patches {
                        let item_label = format!("#{} {}", patch.id, patch.label);
                        ui.selectable_value(&mut fixture_id, Some(patch.id), item_label);
                    }
                });
            ui.end_row();

            ui.label("Scale:");
            ui.add(egui::DragValue::new(&mut scale)
                .range(0.25f32..=4.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.end_row();

            ui.label("Fill:");
            egui::color_picker::color_edit_button_srgba(
                ui, &mut bg, egui::color_picker::Alpha::Opaque,
            );
            ui.end_row();

            ui.label("Outline:");
            egui::color_picker::color_edit_button_srgba(
                ui, &mut outline, egui::color_picker::Alpha::Opaque,
            );
            ui.end_row();

            ui.separator(); ui.separator(); ui.end_row();
            ui.strong("Live Linking"); ui.end_row();

            ui.label("Link fill → color:");
            ui.checkbox(&mut link_color, "");
            ui.end_row();

            ui.label("Link fill → intensity:");
            ui.checkbox(&mut link_intensity, "");
            ui.end_row();
        });

    if let Some(s) = app.magic_sheet.get_shape_mut(shape_id) {
        s.fixture_id = fixture_id;
        s.scale = scale;
        let [r, g, b, a] = bg.to_srgba_unmultiplied();
        s.bg_color = [r, g, b, a];
        let [r, g, b, a] = outline.to_srgba_unmultiplied();
        s.outline_color = [r, g, b, a];
        s.link_color = link_color;
        s.link_intensity = link_intensity;
    }
}

// ── Properties panel — multi-shape ───────────────────────────────────────────

fn render_multi_shape_properties(ui: &mut Ui, app: &mut EasyCueApp, shape_ids: &[u32]) {
    ui.heading(format!("{} Shapes Selected", shape_ids.len()));
    ui.separator();

    // Read shared values from the first selected shape as defaults
    let first = match app.magic_sheet.shapes.iter().find(|s| s.id == shape_ids[0]) {
        Some(s) => s.clone(),
        None => return,
    };
    let mut scale = first.scale;
    let mut bg = Color32::from_rgba_unmultiplied(first.bg_color[0], first.bg_color[1], first.bg_color[2], first.bg_color[3]);
    let mut outline = Color32::from_rgba_unmultiplied(first.outline_color[0], first.outline_color[1], first.outline_color[2], first.outline_color[3]);
    let mut link_color = first.link_color;
    let mut link_intensity = first.link_intensity;

    let mut scale_changed = false;
    let mut bg_changed = false;
    let mut outline_changed = false;
    let mut link_color_changed = false;
    let mut link_intensity_changed = false;

    egui::Grid::new("multi_shape_props_grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Scale (all):");
            scale_changed = ui.add(egui::DragValue::new(&mut scale)
                .range(0.25f32..=4.0)
                .speed(0.01)
                .fixed_decimals(2)).changed();
            ui.end_row();

            ui.label("Fill (all):");
            bg_changed = egui::color_picker::color_edit_button_srgba(
                ui, &mut bg, egui::color_picker::Alpha::Opaque,
            ).changed();
            ui.end_row();

            ui.label("Outline (all):");
            outline_changed = egui::color_picker::color_edit_button_srgba(
                ui, &mut outline, egui::color_picker::Alpha::Opaque,
            ).changed();
            ui.end_row();

            ui.separator(); ui.separator(); ui.end_row();
            ui.strong("Live Linking"); ui.end_row();

            ui.label("Link fill → color:");
            link_color_changed = ui.checkbox(&mut link_color, "").changed();
            ui.end_row();

            ui.label("Link fill → intensity:");
            link_intensity_changed = ui.checkbox(&mut link_intensity, "").changed();
            ui.end_row();
        });

    // Apply changes to all selected shapes
    let ids = shape_ids.to_vec();
    for sid in ids {
        if let Some(s) = app.magic_sheet.get_shape_mut(sid) {
            if scale_changed { s.scale = scale; }
            if bg_changed {
                let [r, g, b, a] = bg.to_srgba_unmultiplied();
                s.bg_color = [r, g, b, a];
            }
            if outline_changed {
                let [r, g, b, a] = outline.to_srgba_unmultiplied();
                s.outline_color = [r, g, b, a];
            }
            if link_color_changed { s.link_color = link_color; }
            if link_intensity_changed { s.link_intensity = link_intensity; }
        }
    }
}

// ── Alignment helper ──────────────────────────────────────────────────────────

enum Alignment { Left, Right, Top, Bottom, DistributeH, DistributeV }

fn align_shapes(app: &mut EasyCueApp, alignment: Alignment) {
    let ids: Vec<u32> = app.magic_sheet_state.selected_shape_ids.iter().copied().collect();
    if ids.len() < 2 { return; }

    struct Info { id: u32, cx: f32, cy: f32, hw: f32, hh: f32 }
    let mut infos: Vec<Info> = ids.iter().filter_map(|&id| {
        app.magic_sheet.shapes.iter().find(|s| s.id == id).map(|s| Info {
            id,
            cx: s.pos[0],
            cy: s.pos[1],
            hw: BASE_W * s.scale / 2.0,
            hh: BASE_H * s.scale / 2.0,
        })
    }).collect();

    let min_left   = infos.iter().map(|i| i.cx - i.hw).fold(f32::MAX, f32::min);
    let max_right  = infos.iter().map(|i| i.cx + i.hw).fold(f32::MIN, f32::max);
    let min_top    = infos.iter().map(|i| i.cy - i.hh).fold(f32::MAX, f32::min);
    let max_bottom = infos.iter().map(|i| i.cy + i.hh).fold(f32::MIN, f32::max);

    match alignment {
        Alignment::DistributeH => {
            infos.sort_by(|a, b| a.cx.partial_cmp(&b.cx).unwrap());
            let n = infos.len();
            let span = max_right - min_left;
            let total_shape_w: f32 = infos.iter().map(|i| i.hw * 2.0).sum();
            let gap = if n > 1 { (span - total_shape_w) / (n as f32 - 1.0) } else { 0.0 };
            let mut x = min_left;
            for info in &infos {
                x += info.hw;
                if let Some(s) = app.magic_sheet.get_shape_mut(info.id) {
                    s.pos[0] = x;
                }
                x += info.hw + gap;
            }
        }
        Alignment::DistributeV => {
            infos.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
            let n = infos.len();
            let span = max_bottom - min_top;
            let total_shape_h: f32 = infos.iter().map(|i| i.hh * 2.0).sum();
            let gap = if n > 1 { (span - total_shape_h) / (n as f32 - 1.0) } else { 0.0 };
            let mut y = min_top;
            for info in &infos {
                y += info.hh;
                if let Some(s) = app.magic_sheet.get_shape_mut(info.id) {
                    s.pos[1] = y;
                }
                y += info.hh + gap;
            }
        }
        _ => {
            for info in &infos {
                if let Some(s) = app.magic_sheet.get_shape_mut(info.id) {
                    match alignment {
                        Alignment::Left   => s.pos[0] = min_left + info.hw,
                        Alignment::Right  => s.pos[0] = max_right - info.hw,
                        Alignment::Top    => s.pos[1] = min_top + info.hh,
                        Alignment::Bottom => s.pos[1] = max_bottom - info.hh,
                        _ => {}
                    }
                }
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn canvas_to_screen(canvas_rect: Rect, offset: Vec2, zoom: f32, pos: [f32; 2]) -> Pos2 {
    let cx = canvas_rect.min.x + offset.x + pos[0] * zoom;
    let cy = canvas_rect.min.y + offset.y + pos[1] * zoom;
    Pos2::new(cx, cy)
}

fn canvas_centre(ui: &Ui, app: &EasyCueApp) -> Pos2 {
    let available = ui.available_rect_before_wrap();
    let cx = (available.width() / 2.0 - app.magic_sheet_state.canvas_offset.x)
        / app.magic_sheet_state.canvas_zoom;
    let cy = (available.height() / 2.0 - app.magic_sheet_state.canvas_offset.y)
        / app.magic_sheet_state.canvas_zoom;
    Pos2::new(cx, cy)
}

fn draw_dot_grid(painter: &egui::Painter, canvas_rect: Rect, offset: Vec2, zoom: f32) {
    let spacing = (40.0 * zoom).max(10.0);
    let dot_r = (1.0 * zoom).clamp(0.8, 2.5);
    let color = Color32::from_gray(35);

    let start_x = canvas_rect.min.x + offset.x.rem_euclid(spacing);
    let start_y = canvas_rect.min.y + offset.y.rem_euclid(spacing);

    let mut x = start_x;
    while x < canvas_rect.max.x {
        let mut y = start_y;
        while y < canvas_rect.max.y {
            painter.circle_filled(Pos2::new(x, y), dot_r, color);
            y += spacing;
        }
        x += spacing;
    }
}

fn draw_shape(
    painter: &egui::Painter,
    kind: &ShapeKind,
    center: Pos2,
    w: f32,
    h: f32,
    fill: Color32,
    stroke: Stroke,
) {
    match kind {
        ShapeKind::Rectangle => {
            let rect = Rect::from_center_size(center, egui::vec2(w, h));
            painter.rect_filled(rect, 4.0, fill);
            painter.rect_stroke(rect, 4.0, stroke, egui::epaint::StrokeKind::Inside);
        }
        ShapeKind::Circle => {
            let r = (w.min(h)) / 2.0;
            painter.circle_filled(center, r, fill);
            painter.circle_stroke(center, r, stroke);
        }
        ShapeKind::Diamond => {
            let pts = vec![
                Pos2::new(center.x,           center.y - h / 2.0),
                Pos2::new(center.x + w / 2.0, center.y           ),
                Pos2::new(center.x,           center.y + h / 2.0),
                Pos2::new(center.x - w / 2.0, center.y           ),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
    }
}

fn draw_shape_label(
    painter: &egui::Painter,
    center: Pos2,
    w: f32,
    h: f32,
    label: &str,
    fix_num: Option<usize>,
    intensity: f32,
    rgb: Option<Color32>,
) {
    let text_color = Color32::from_gray(220);
    let small_color = Color32::from_gray(160);
    let font_sm = egui::FontId::proportional(10.0);
    let font_md = egui::FontId::proportional(13.0);

    if let Some(num) = fix_num {
        painter.text(
            Pos2::new(center.x - w / 2.0 + 4.0, center.y - h / 2.0 + 3.0),
            egui::Align2::LEFT_TOP,
            format!("#{}", num),
            font_sm.clone(),
            small_color,
        );
    }

    if let Some(color) = rgb {
        let swatch_r = 6.0f32.min(w / 6.0).min(h / 4.0);
        let swatch_pos = Pos2::new(center.x + w / 2.0 - swatch_r - 3.0, center.y - h / 2.0 + swatch_r + 3.0);
        painter.circle_filled(swatch_pos, swatch_r, color);
        painter.circle_stroke(swatch_pos, swatch_r, Stroke::new(0.5, Color32::from_gray(80)));
    }

    let label_display = if label.len() > 12 { &label[..12] } else { label };
    painter.text(
        Pos2::new(center.x, center.y - 5.0),
        egui::Align2::CENTER_CENTER,
        label_display,
        font_md,
        text_color,
    );

    let int_str = if intensity > 0.0 {
        format!("{}%", (intensity * 100.0).round() as u8)
    } else {
        "0%".to_string()
    };
    let int_color = if intensity > 0.0 {
        Color32::from_rgb(180, 220, 120)
    } else {
        Color32::from_gray(100)
    };
    painter.text(
        Pos2::new(center.x, center.y + h / 2.0 - 10.0),
        egui::Align2::CENTER_BOTTOM,
        int_str,
        font_sm,
        int_color,
    );
}

fn fixture_info(
    app: &EasyCueApp,
    fixture_id: Option<usize>,
) -> (String, Option<usize>, f32, Option<Color32>) {
    let fid = match fixture_id {
        Some(id) => id,
        None => return ("(unassigned)".to_string(), None, 0.0, None),
    };

    let patch = match app.fixtures.patch_list().get_patch(fid) {
        Some(p) => p,
        None => return (format!("#{}", fid), Some(fid), 0.0, None),
    };
    let profile = match app.fixtures.get_profile(&patch.profile_id) {
        Some(p) => p,
        None => return (patch.label.clone(), Some(fid), 0.0, None),
    };
    let universe = match app.universes.first() {
        Some(u) => u,
        None => return (patch.label.clone(), Some(fid), 0.0, None),
    };

    let intensity = if profile.has_intensity() {
        profile.get_parameter_offset(&FixtureParameter::Intensity)
            .map(|off| universe.get_channel(patch.start_address + off).unwrap_or(0) as f32 / 100.0)
            .unwrap_or(0.0)
    } else if profile.is_rgb() {
        app.virtual_intensity.get_intensity(fid).unwrap_or_else(|| {
            app.virtual_intensity.calculate_intensity(fid, universe, patch, profile)
        })
    } else {
        0.0
    };

    let rgb = if profile.is_rgb() {
        let r = profile.get_parameter_offset(&FixtureParameter::Red)
            .map(|o| universe.get_channel(patch.start_address + o).unwrap_or(0)).unwrap_or(0);
        let g = profile.get_parameter_offset(&FixtureParameter::Green)
            .map(|o| universe.get_channel(patch.start_address + o).unwrap_or(0)).unwrap_or(0);
        let b = profile.get_parameter_offset(&FixtureParameter::Blue)
            .map(|o| universe.get_channel(patch.start_address + o).unwrap_or(0)).unwrap_or(0);
        Some(Color32::from_rgb(
            ((r as f32 / 100.0) * 255.0) as u8,
            ((g as f32 / 100.0) * 255.0) as u8,
            ((b as f32 / 100.0) * 255.0) as u8,
        ))
    } else {
        None
    };

    (patch.label.clone(), Some(fid), intensity, rgb)
}

/// Apply an additive intensity delta to every currently selected fixture.
fn adjust_selected_fixtures_intensity(app: &mut EasyCueApp, delta: f32) {
    let selected: Vec<usize> = app.ui_state.selected_fixtures.iter().copied().collect();
    for fid in selected {
        let patch = match app.fixtures.patch_list().get_patch(fid) {
            Some(p) => p.clone(),
            None => continue,
        };
        let profile = match app.fixtures.get_profile(&patch.profile_id).cloned() {
            Some(p) => p,
            None => continue,
        };
        let current = if let Some(universe) = app.universes.first() {
            if profile.has_intensity() {
                profile.get_parameter_offset(&crate::fixtures::profiles::FixtureParameter::Intensity)
                    .map(|off| universe.get_channel(patch.start_address + off).unwrap_or(0) as f32 / 100.0)
                    .unwrap_or(0.0)
            } else if profile.is_rgb() {
                app.virtual_intensity.get_intensity(fid).unwrap_or_else(|| {
                    app.virtual_intensity.calculate_intensity(fid, universe, &patch, &profile)
                })
            } else {
                0.0
            }
        } else {
            0.0
        };
        let new_intensity = (current + delta).clamp(0.0, 1.0);
        if let Some(universe) = app.universes.first_mut() {
            if profile.has_intensity() {
                if let Some(offset) = profile.get_parameter_offset(&crate::fixtures::profiles::FixtureParameter::Intensity) {
                    let _ = universe.set_channel(patch.start_address + offset, (new_intensity * 100.0).round() as u8);
                }
            } else if profile.is_rgb() {
                let _ = app.virtual_intensity.set_intensity(fid, new_intensity, universe, &patch, &profile);
            }
        }
    }
}

#[allow(dead_code)]
fn set_fixture_intensity(app: &mut EasyCueApp, fixture_id: usize, intensity: f32) {
    let patch = match app.fixtures.patch_list().get_patch(fixture_id) {
        Some(p) => p.clone(),
        None => return,
    };
    let profile = match app.fixtures.get_profile(&patch.profile_id).cloned() {
        Some(p) => p,
        None => return,
    };
    if let Some(universe) = app.universes.first_mut() {
        if profile.has_intensity() {
            if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Intensity) {
                let _ = universe.set_channel(patch.start_address + offset, (intensity * 100.0).round() as u8);
            }
        } else if profile.is_rgb() {
            let _ = app.virtual_intensity.set_intensity(fixture_id, intensity, universe, &patch, &profile);
        }
    }
}
