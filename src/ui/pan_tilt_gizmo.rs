//! 2-D pan/tilt position gizmo widget.
//!
//! Renders a square pad with a draggable dot.
//! X axis = pan  (0 = full left, 100 = full right).
//! Y axis = tilt (0 = full up,   100 = full down).
//!
//! Values are in the internal 0–100 range, matching the rest of the DMX layer.

use egui::{Color32, Sense, Stroke, Ui, Vec2, pos2};

/// Stateless 2-D pan/tilt gizmo.  Pan and tilt map directly to raw channel
/// values (0–100) so no intermediate conversion state is required.
pub struct PanTiltGizmo;

impl Default for PanTiltGizmo {
    fn default() -> Self {
        Self
    }
}

impl PanTiltGizmo {
    pub fn new() -> Self {
        Self
    }

    /// Draw the gizmo.  `pan` and `tilt` are current internal values (0–100).
    /// Returns `Some((new_pan, new_tilt))` when the user clicks or drags.
    pub fn show(&self, ui: &mut Ui, pan: u8, tilt: u8, size: f32) -> Option<(u8, u8)> {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // Background
            painter.rect_filled(rect, 3.0, Color32::from_gray(28));
            painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(70)), egui::epaint::StrokeKind::Inside);

            // Subtle centre crosshairs
            let cx = rect.left() + rect.width() * 0.5;
            let cy = rect.top() + rect.height() * 0.5;
            let guide = Color32::from_gray(48);
            painter.line_segment(
                [pos2(cx, rect.top()), pos2(cx, rect.bottom())],
                Stroke::new(1.0, guide),
            );
            painter.line_segment(
                [pos2(rect.left(), cy), pos2(rect.right(), cy)],
                Stroke::new(1.0, guide),
            );

            // Corner pip marks
            let m = 4.5_f32;
            let corner_col = Color32::from_gray(60);
            for (x, y) in [
                (rect.left() + m, rect.top() + m),
                (rect.right() - m, rect.top() + m),
                (rect.left() + m, rect.bottom() - m),
                (rect.right() - m, rect.bottom() - m),
            ] {
                painter.circle_filled(pos2(x, y), 1.5, corner_col);
            }

            // Dot
            let dot_x = rect.left() + (pan as f32 / 100.0) * rect.width();
            let dot_y = rect.top() + (tilt as f32 / 100.0) * rect.height();
            let dot = pos2(
                dot_x.clamp(rect.left(), rect.right()),
                dot_y.clamp(rect.top(), rect.bottom()),
            );

            // Shadow
            painter.circle_stroke(
                dot,
                9.5,
                Stroke::new(3.0, Color32::from_rgba_premultiplied(0, 0, 0, 180)),
            );
            // Outer ring
            painter.circle_stroke(dot, 8.0, Stroke::new(1.5, Color32::WHITE));
            // Core fill
            painter.circle_filled(dot, 5.0, Color32::from_rgb(80, 160, 255));
        }

        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let new_pan = ((pos.x - rect.left()) / rect.width() * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u8;
                let new_tilt = ((pos.y - rect.top()) / rect.height() * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u8;
                return Some((new_pan, new_tilt));
            }
        }

        None
    }
}
