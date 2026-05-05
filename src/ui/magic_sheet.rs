//! Magic sheet panel — freeform fixture-layout canvas
//!
//! Edit mode: place and reposition shapes, assign fixtures, set colours.
//! Live mode: click/drag shapes to select fixtures and adjust intensity,
//!            kept in sync with the Channels panel via `app.ui_state.selected_fixtures`.

use egui::Ui;
use crate::app::EasyCueApp;

/// Entry point called by the tab viewer.
pub fn render_magic_sheet_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // ── Toolbar ─────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let label = if app.magic_sheet_state.edit_mode { "Edit Mode  " } else { "Live Mode  " };
        ui.toggle_value(&mut app.magic_sheet_state.edit_mode, label);

        if app.magic_sheet_state.edit_mode {
            ui.separator();
            ui.label(egui::RichText::new("(shape placement coming in next build)").italics().small());
        }
    });

    ui.separator();

    // ── Canvas placeholder ───────────────────────────────────────────────────
    let n = app.magic_sheet.shapes.len();
    ui.centered_and_justified(|ui| {
        if n == 0 {
            ui.label(
                egui::RichText::new(
                    "Magic Sheet — no shapes yet.\nSwitch to Edit Mode to add fixtures."
                )
                .color(egui::Color32::GRAY)
                .italics(),
            );
        } else {
            ui.label(
                egui::RichText::new(format!("{} shape(s) defined — rendering coming soon.", n))
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        }
    });
}
