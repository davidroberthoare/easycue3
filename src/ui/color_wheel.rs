//! Circular HSV colour-wheel widget.
//!
//! Hue varies by angle (red at 3 o'clock, clockwise), saturation by radius
//! (white at centre, full colour at rim).  Value is fixed at 1.0 — use the
//! fixture's intensity or virtual-intensity slider to control brightness.

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2, pos2};

pub struct ColorWheel {
    /// Hue, 0.0–1.0 (red=0, clockwise).
    pub hue: f32,
    /// Saturation, 0.0–1.0.
    pub saturation: f32,
    texture: Option<egui::TextureHandle>,
    last_size: Vec2,
}

impl Default for ColorWheel {
    fn default() -> Self {
        Self { hue: 0.0, saturation: 0.0, texture: None, last_size: Vec2::ZERO }
    }
}

impl ColorWheel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sync from the fixture's current colour (internal 0–100 range).
    /// No-op when all channels are zero so the wheel doesn't snap to red when
    /// the fixture is dark.
    pub fn set_from_srgb_100(&mut self, r: u8, g: u8, b: u8) {
        if r == 0 && g == 0 && b == 0 {
            return;
        }
        let (h, s, _v) = rgb_to_hsv(r as f32 / 100.0, g as f32 / 100.0, b as f32 / 100.0);
        self.hue = h;
        self.saturation = s;
    }

    /// Fully-saturated sRGB (0.0–1.0) for the current hue + saturation.
    /// Apply the fixture's intensity after this to get actual channel values.
    pub fn selected_color(&self) -> (f32, f32, f32) {
        hsv_to_rgb(self.hue, self.saturation, 1.0)
    }

    /// Draw the wheel.  `size` is the diameter in logical pixels.
    /// Returns `true` when the user moves the selection.
    pub fn show(&mut self, ui: &mut Ui, size: f32) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());

        if self.last_size != rect.size() || self.texture.is_none() {
            self.texture = Some(self.build_texture(ui.ctx(), rect.size()));
            self.last_size = rect.size();
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            if let Some(tex) = &self.texture {
                painter.image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            // Crosshair at current selection
            let center = rect.center();
            let radius = rect.width().min(rect.height()) * 0.5;
            let angle = self.hue * std::f32::consts::TAU;
            let dist = self.saturation * radius;
            let sel = pos2(
                center.x + angle.cos() * dist,
                center.y + angle.sin() * dist,
            );
            painter.circle_stroke(sel, 8.5, Stroke::new(3.0, Color32::BLACK));
            painter.circle_stroke(sel, 7.0, Stroke::new(1.5, Color32::WHITE));
        }

        // Handle input
        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let center = rect.center();
                let radius = rect.width().min(rect.height()) * 0.5;
                let dx = pos.x - center.x;
                let dy = pos.y - center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= radius {
                    self.hue = (dy.atan2(dx) / std::f32::consts::TAU).rem_euclid(1.0);
                    self.saturation = (dist / radius).clamp(0.0, 1.0);
                    return true;
                }
            }
        }

        false
    }

    fn build_texture(&self, ctx: &egui::Context, size: Vec2) -> egui::TextureHandle {
        let w = (size.x as usize).max(1);
        let h = (size.y as usize).max(1);
        let mut pixels = vec![Color32::TRANSPARENT; w * h];
        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        let radius = cx.min(cy);

        for row in 0..h {
            for col in 0..w {
                let dx = col as f32 - cx;
                let dy = row as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > radius {
                    continue;
                }
                let hue = (dy.atan2(dx) / std::f32::consts::TAU).rem_euclid(1.0);
                let sat = dist / radius;
                let (r, g, b) = hsv_to_rgb(hue, sat, 1.0);
                pixels[row * w + col] = Color32::from_rgb(
                    (r * 255.0).round() as u8,
                    (g * 255.0).round() as u8,
                    (b * 255.0).round() as u8,
                );
            }
        }

        ctx.load_texture(
            "color_wheel",
            egui::ColorImage { size: [w, h], pixels },
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                ..Default::default()
            },
        )
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s < 1e-6 {
        return (v, v, v);
    }
    let h6 = h * 6.0;
    let i = h6.floor() as i32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let v = max;
    let s = if max < 1e-6 { 0.0 } else { delta / max };
    let h = if delta < 1e-6 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    ((h / 6.0).rem_euclid(1.0), s, v)
}
