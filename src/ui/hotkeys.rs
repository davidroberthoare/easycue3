//! Hotkeys setup panel — assign cues to Ctrl+0…Ctrl+9.

use crate::app::EasyCueApp;
use crate::hotkeys::HotkeyMode;
use egui::Ui;
use egui_phosphor::regular as ph;

/// Render the hotkey assignment panel.
pub fn render_hotkeys_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "Assign cues to Ctrl+0…Ctrl+9. Trigger fires the cue without touching \
             the play head; Hold plays while the key is held; Latch toggles on the \
             first press and off on the second. Hold/Latch use the cue's fade \
             up/down times.",
        )
        .small()
        .color(egui::Color32::from_gray(160)),
    );
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    let cue_choices: Vec<(u32, String)> = app
        .cue_list
        .cues()
        .iter()
        .map(|c| (c.id, format!("{:.1} {}", c.number, c.label)))
        .collect();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("hotkeys_grid")
                .num_columns(4)
                .spacing([14.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Key");
                    ui.strong("Cue");
                    ui.strong("Trigger mode");
                    ui.strong("Test");
                    ui.end_row();

                    for idx in 0..10 {
                        ui.label(
                            egui::RichText::new(format!("Ctrl+{}", idx))
                                .monospace()
                                .strong(),
                        );

                        // ── Cue picker ──────────────────────────────────────
                        let selected_id = app.hotkeys.get(idx).map(|a| a.cue_id).unwrap_or(0);
                        let selected_label = cue_choices
                            .iter()
                            .find(|(id, _)| *id == selected_id)
                            .map(|(_, l)| l.clone())
                            .unwrap_or_else(|| "(unassigned)".to_string());
                        let mut new_id = selected_id;
                        egui::ComboBox::from_id_salt(("hk_cue", idx))
                            .selected_text(selected_label)
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut new_id, 0, "(unassigned)");
                                for (id, label) in &cue_choices {
                                    ui.selectable_value(&mut new_id, *id, label);
                                }
                            });
                        if new_id != selected_id {
                            if let Some(a) = app.hotkeys.get_mut(idx) {
                                a.cue_id = new_id;
                                // Adjust cues only make sense as one-shot triggers.
                                #[cfg(feature = "audio")]
                                if let Some(c) = app.cue_list.find_by_id(new_id) {
                                    if c.is_adjust() {
                                        a.mode = HotkeyMode::Trigger;
                                    }
                                }
                            }
                        }

                        // ── Trigger mode ────────────────────────────────────
                        let is_adjust = {
                            let cue = app.cue_list.find_by_id(selected_id);
                            #[cfg(feature = "audio")]
                            {
                                cue.map(|c| c.is_adjust()).unwrap_or(false)
                            }
                            #[cfg(not(feature = "audio"))]
                            {
                                let _ = cue;
                                false
                            }
                        };
                        let can_hold = !is_adjust && selected_id != 0;
                        let mut mode = app.hotkeys.get(idx).map(|a| a.mode).unwrap_or_default();
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(mode == HotkeyMode::Trigger, "Trigger")
                                .clicked()
                            {
                                mode = HotkeyMode::Trigger;
                            }
                            if ui
                                .add_enabled(
                                    can_hold,
                                    egui::SelectableLabel::new(mode == HotkeyMode::Hold, "Hold"),
                                )
                                .on_hover_text(
                                    "Held while pressed: plays while the key is down, \
                                     fades out on release (cue fade up/down times).",
                                )
                                .clicked()
                            {
                                mode = HotkeyMode::Hold;
                            }
                            if ui
                                .add_enabled(
                                    can_hold,
                                    egui::SelectableLabel::new(mode == HotkeyMode::Latch, "Latch"),
                                )
                                .on_hover_text(
                                    "First press starts the cue, second press stops it \
                                     (cue fade up/down times).",
                                )
                                .clicked()
                            {
                                mode = HotkeyMode::Latch;
                            }
                        });
                        if let Some(a) = app.hotkeys.get_mut(idx) {
                            a.mode = mode;
                        }

                        // ── Test / status ───────────────────────────────────
                        ui.horizontal(|ui| {
                            let engaged = app.hotkey_runtime.engaged[idx];
                            if engaged {
                                ui.colored_label(egui::Color32::from_rgb(255, 200, 0), ph::CIRCLE);
                            }
                            let assigned = selected_id != 0;
                            if ui
                                .add_enabled(assigned, egui::Button::new(ph::PLAY))
                                .on_hover_text("Test: fire the assigned cue once")
                                .clicked()
                            {
                                app.hotkey_trigger(selected_id);
                            }
                        });
                        ui.end_row();
                    }
                });
        });
}
