//! Magic sheet panel — freeform fixture-layout canvas
//!
//! Edit mode: place and reposition shapes, assign fixtures, set colours.
//! Live mode: click/drag shapes to select fixtures and adjust intensity,
//!            kept in sync with the Channels panel via `app.ui_state.selected_fixtures`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use crate::app::EasyCueApp;
use crate::magic_sheet::ShapeKind;
use crate::fixtures::profiles::FixtureParameter;

/// Base shape dimensions in canvas-space logical pixels (before scale).
const BASE_W: f32 = 80.0;
const BASE_H: f32 = 60.0;

/// Entry point called by the tab viewer.
pub fn render_magic_sheet_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // ── Toolbar ─────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let label = if app.magic_sheet_state.edit_mode { "✏ Edit" } else { "▶ Live" };
        ui.toggle_value(&mut app.magic_sheet_state.edit_mode, label);

        if app.magic_sheet_state.edit_mode {
            ui.separator();

            // Shape palette — click to add a shape at canvas centre
            ui.label("Add:");
            for kind in crate::magic_sheet::ALL_SHAPE_KINDS {
                if ui.button(kind.to_string()).clicked() {
                    let canvas_centre = canvas_centre(ui, app);
                    app.magic_sheet.add_shape(kind.clone(), [canvas_centre.x, canvas_centre.y]);
                }
            }

            ui.separator();

            // Delete selected shape
            if let Some(sel_id) = app.magic_sheet_state.selected_shape_id {
                if ui.button("🗑 Delete").clicked() {
                    app.magic_sheet.remove_shape(sel_id);
                    app.magic_sheet_state.selected_shape_id = None;
                }
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

    // ── Canvas area ──────────────────────────────────────────────────────────
    let available = ui.available_size();
    let (canvas_rect, canvas_response) =
        ui.allocate_exact_size(available, Sense::click_and_drag());

    // ── Pan: middle-click drag or right-click drag ───────────────────────────
    if canvas_response.dragged_by(egui::PointerButton::Middle)
        || canvas_response.dragged_by(egui::PointerButton::Secondary)
    {
        app.magic_sheet_state.canvas_offset += canvas_response.drag_delta();
    }

    // ── Zoom: scroll wheel ────────────────────────────────────────────────────
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && canvas_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
        let zoom_factor = 1.0 + scroll_delta * 0.001;
        app.magic_sheet_state.canvas_zoom = (app.magic_sheet_state.canvas_zoom * zoom_factor).clamp(0.1, 5.0);
    }

    // ── Background ───────────────────────────────────────────────────────────
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(8, 22, 38));

    // Dot-grid to give spatial reference
    draw_dot_grid(&painter, canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom);

    // ── Shapes ───────────────────────────────────────────────────────────────
    // Collect shape data so we can iterate without holding borrows into app.
    let shapes_snapshot: Vec<_> = app.magic_sheet.shapes.iter().map(|s| {
        let fixture_id = s.fixture_id;
        let kind = s.kind.clone();
        let pos = s.pos;
        let scale = s.scale;
        let bg = Color32::from_rgba_unmultiplied(s.bg_color[0], s.bg_color[1], s.bg_color[2], s.bg_color[3]);
        let outline = Color32::from_rgba_unmultiplied(s.outline_color[0], s.outline_color[1], s.outline_color[2], s.outline_color[3]);
        let id = s.id;
        (id, kind, pos, scale, bg, outline, fixture_id)
    }).collect();

    for (shape_id, kind, pos, scale, bg_color, outline_color, fixture_id) in &shapes_snapshot {
        let shape_id = *shape_id;
        let scale = *scale;

        // Canvas → screen coordinates
        let screen_center = canvas_to_screen(canvas_rect, app.magic_sheet_state.canvas_offset, app.magic_sheet_state.canvas_zoom, *pos);
        let w = BASE_W * scale * app.magic_sheet_state.canvas_zoom;
        let h = BASE_H * scale * app.magic_sheet_state.canvas_zoom;

        // Fixture info (read before painter borrow)
        let (label, fix_num, intensity, rgb) = fixture_info(app, *fixture_id);

        let is_selected_shape = app.magic_sheet_state.edit_mode
            && app.magic_sheet_state.selected_shape_id == Some(shape_id);
        let is_selected_fixture = fixture_id
            .map(|fid| app.ui_state.selected_fixtures.contains(&fid))
            .unwrap_or(false);

        // Interaction rect (screen space)
        let shape_rect = Rect::from_center_size(screen_center, egui::vec2(w, h));

        // Allocate a response for interaction only if the rect is inside the canvas
        let resp = ui.allocate_rect(shape_rect, Sense::click_and_drag());

        // ── Edit mode: drag to reposition ────────────────────────────────────
        if app.magic_sheet_state.edit_mode {
            if resp.clicked() {
                app.magic_sheet_state.selected_shape_id = Some(shape_id);
            }
            if resp.dragged() {
                let delta = resp.drag_delta() / app.magic_sheet_state.canvas_zoom;
                if let Some(s) = app.magic_sheet.get_shape_mut(shape_id) {
                    s.pos[0] += delta.x;
                    s.pos[1] += delta.y;
                }
            }
        } else {
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
                }
            }

            // ── Live mode: vertical drag adjusts intensity ────────────────────
            if resp.dragged() {
                if let Some(fid) = fixture_id {
                    let dy = resp.drag_delta().y;
                    if dy.abs() > 0.5 {
                        if !app.ui_state.selected_fixtures.contains(fid) {
                            app.ui_state.selected_fixtures.clear();
                            app.ui_state.selected_fixtures.insert(*fid);
                            app.ui_state.last_selected_fixture = *fixture_id;
                        }
                        let current = intensity;
                        let delta = (-dy / h).clamp(-1.0, 1.0);
                        let new_intensity = (current + delta).clamp(0.0, 1.0);
                        set_fixture_intensity(app, *fid, new_intensity);
                    }
                }
            }
        }

        // ── Draw shape ───────────────────────────────────────────────────────
        let highlight = is_selected_shape || is_selected_fixture;
        let border_color = if highlight {
            Color32::from_rgb(100, 180, 255)
        } else {
            *outline_color
        };
        let border_width = if highlight { 2.5 } else { 1.5 };

        draw_shape(
            &painter,
            kind,
            screen_center,
            w,
            h,
            *bg_color,
            Stroke::new(border_width, border_color),
        );

        // ── Draw fixture info text ────────────────────────────────────────────
        draw_shape_label(
            &painter,
            screen_center,
            w,
            h,
            &label,
            fix_num,
            intensity,
            rgb,
        );
    }

    // Click on empty canvas in edit mode: deselect shape
    if app.magic_sheet_state.edit_mode
        && canvas_response.clicked()
        && !canvas_response.drag_started()
    {
        app.magic_sheet_state.selected_shape_id = None;
    }

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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn canvas_to_screen(canvas_rect: Rect, offset: Vec2, zoom: f32, pos: [f32; 2]) -> Pos2 {
    let cx = canvas_rect.min.x + offset.x + pos[0] * zoom;
    let cy = canvas_rect.min.y + offset.y + pos[1] * zoom;
    Pos2::new(cx, cy)
}

/// Logical canvas position at the centre of the visible canvas area.
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
                Pos2::new(center.x,         center.y - h / 2.0), // top
                Pos2::new(center.x + w / 2.0, center.y),          // right
                Pos2::new(center.x,         center.y + h / 2.0), // bottom
                Pos2::new(center.x - w / 2.0, center.y),          // left
            ];
            painter.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
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

    // Fixture number (top-left)
    if let Some(num) = fix_num {
        painter.text(
            Pos2::new(center.x - w / 2.0 + 4.0, center.y - h / 2.0 + 3.0),
            egui::Align2::LEFT_TOP,
            format!("#{}", num),
            font_sm.clone(),
            small_color,
        );
    }

    // Colour swatch (top-right, only for RGB fixtures)
    if let Some(color) = rgb {
        let swatch_r = 6.0f32.min(w / 6.0).min(h / 4.0);
        let swatch_pos = Pos2::new(center.x + w / 2.0 - swatch_r - 3.0, center.y - h / 2.0 + swatch_r + 3.0);
        painter.circle_filled(swatch_pos, swatch_r, color);
        painter.circle_stroke(swatch_pos, swatch_r, Stroke::new(0.5, Color32::from_gray(80)));
    }

    // Label (centre)
    let label_display = if label.len() > 12 { &label[..12] } else { label };
    painter.text(
        Pos2::new(center.x, center.y - 5.0),
        egui::Align2::CENTER_CENTER,
        label_display,
        font_md,
        text_color,
    );

    // Intensity (bottom-centre)
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

/// Read a fixture's label, id, intensity (0–1), and RGB colour from app state.
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
