//! Effects panel — the show-level effect library editor.
//!
//! Lists all effects, edits the selected one live (running effects pick up
//! parameter changes immediately), and offers manual start/stop on the current
//! fixture selection for programming. During playback, channel readouts show
//! the un-modulated base look; only the DMX output carries the effect.

use crate::app::EasyCueApp;
use crate::effects::{Effect, EffectTarget, Waveform};
use egui::Ui;
use egui_phosphor::regular as ph;

/// Ramp time for manual (panel-button) start/stop, in seconds.
const MANUAL_RAMP: f32 = 0.5;

pub fn render_effects_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // ── Toolbar ──────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        if ui.button(format!("{} New Effect", ph::PLUS)).clicked() {
            let count = app.effect_list.len();
            let mut effect = Effect::new();
            effect.label = format!("Effect {}", count + 1);
            let id = app.effect_list.add(effect);
            app.ui_state.selected_effect_id = Some(id);
        }

        let selected = app.ui_state.selected_effect_id;
        if ui
            .add_enabled(
                selected.is_some(),
                egui::Button::new(format!("{} Delete", ph::TRASH)),
            )
            .clicked()
        {
            if let Some(id) = selected {
                app.effect_engine.stop(id, 0.0);
                app.effect_list.remove(id);
                app.ui_state.selected_effect_id = None;
                if app.ui_state.cue_props_effect_choice == Some(id) {
                    app.ui_state.cue_props_effect_choice = None;
                }
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    app.effect_engine.is_active(),
                    egui::Button::new(format!("{} Stop All", ph::STOP)),
                )
                .clicked()
            {
                app.effect_engine.stop_all(MANUAL_RAMP);
            }
        });
    });

    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        render_effect_list(ui, app);

        if let Some(id) = app.ui_state.selected_effect_id {
            if app.effect_list.find(id).is_some() {
                ui.separator();
                render_effect_editor(ui, app, id);
                ui.separator();
                render_manual_controls(ui, app, id);
            } else {
                app.ui_state.selected_effect_id = None;
            }
        }

        if app.effect_list.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("No effects yet").color(egui::Color32::GRAY));
                ui.label(
                    egui::RichText::new("Create one, then start it on a fixture selection\nor attach it to a cue in Cue Properties.")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
        }
    });
}

fn render_effect_list(ui: &mut Ui, app: &mut EasyCueApp) {
    // Snapshot display rows first so selection clicks can mutate freely after.
    let rows: Vec<(u32, String, bool)> = app
        .effect_list
        .effects()
        .iter()
        .map(|e| {
            (
                e.id,
                format!(
                    "{}  —  {} · {} · {:.2} Hz",
                    if e.label.is_empty() {
                        format!("Effect {}", e.id)
                    } else {
                        e.label.clone()
                    },
                    e.waveform.label(),
                    e.target.label(),
                    e.rate,
                ),
                app.effect_engine.is_running(e.id),
            )
        })
        .collect();

    for (id, text, running) in rows {
        ui.horizontal(|ui| {
            let dot = if running {
                egui::RichText::new("●").color(egui::Color32::from_rgb(45, 200, 45))
            } else {
                egui::RichText::new("○").color(egui::Color32::GRAY)
            };
            ui.label(dot);
            let selected = app.ui_state.selected_effect_id == Some(id);
            if ui.selectable_label(selected, text).clicked() {
                app.ui_state.selected_effect_id = Some(id);
            }
        });
    }
}

fn render_effect_editor(ui: &mut Ui, app: &mut EasyCueApp, id: u32) {
    let Some(effect) = app.effect_list.find_mut(id) else {
        return;
    };

    egui::Grid::new("effect_editor")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Label:");
            ui.add(egui::TextEdit::singleline(&mut effect.label).desired_width(160.0));
            ui.end_row();

            ui.label("Target:");
            egui::ComboBox::from_id_salt("effect_target")
                .selected_text(effect.target.label())
                .show_ui(ui, |ui| {
                    for target in EffectTarget::ALL {
                        ui.selectable_value(&mut effect.target, target, target.label());
                    }
                });
            ui.end_row();

            ui.label("Waveform:");
            egui::ComboBox::from_id_salt("effect_waveform")
                .selected_text(effect.waveform.label())
                .show_ui(ui, |ui| {
                    for waveform in Waveform::ALL {
                        ui.selectable_value(&mut effect.waveform, waveform, waveform.label());
                    }
                });
            ui.end_row();

            ui.label("Rate:");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut effect.rate)
                        .speed(0.05)
                        .range(0.05..=10.0)
                        .suffix(" Hz"),
                );
                ui.label(
                    egui::RichText::new(format!("= {:.0} BPM", effect.rate * 60.0))
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.end_row();

            ui.label("Size:");
            let size_hint = match effect.target {
                EffectTarget::Hue => "Hue swing: 100% = ±180° — a sawtooth at 100% cycles the full rainbow",
                EffectTarget::Saturation => "Saturation swing: toward white on the low half, toward the pure hue on the high half",
                _ => "Peak deviation from the base level, in percentage points",
            };
            ui.add(
                egui::DragValue::new(&mut effect.size)
                    .speed(1.0)
                    .range(0.0..=100.0)
                    .suffix("%"),
            )
            .on_hover_text(size_hint);
            ui.end_row();

            ui.label("Phase spread:");
            ui.add(
                egui::DragValue::new(&mut effect.phase_spread)
                    .speed(5.0)
                    .range(0.0..=360.0)
                    .suffix("°"),
            )
            .on_hover_text("Offsets fixtures across the selection — 360° makes a full wave/chase");
            ui.end_row();

            if effect.waveform == Waveform::Random {
                ui.label("Smoothing:");
                ui.add(egui::Slider::new(&mut effect.smoothing, 0.0..=100.0).suffix("%"))
                    .on_hover_text("0% snaps to each new random level (flicker), 100% glides between them (fire/water)");
                ui.end_row();
            }
        });
}

fn render_manual_controls(ui: &mut Ui, app: &mut EasyCueApp, id: u32) {
    let selection_count = app.ui_state.selected_fixtures.len();
    let running = app.effect_engine.is_running(id);

    ui.horizontal(|ui| {
        let start_label = format!("{} Start on Selection ({})", ph::PLAY, selection_count);
        if ui
            .add_enabled(selection_count > 0, egui::Button::new(start_label))
            .on_hover_text(
                "Runs the effect live on the selected fixtures (for programming/preview)",
            )
            .clicked()
        {
            let mut fixture_ids: Vec<usize> =
                app.ui_state.selected_fixtures.iter().copied().collect();
            fixture_ids.sort_unstable();
            let resolved = app.resolve_effect_fixtures(&fixture_ids);
            app.effect_engine
                .start(id, fixture_ids, resolved, MANUAL_RAMP);
        }

        if ui
            .add_enabled(running, egui::Button::new(format!("{} Stop", ph::STOP)))
            .clicked()
        {
            app.effect_engine.stop(id, MANUAL_RAMP);
        }
    });

    let active: Vec<String> = app
        .effect_engine
        .running()
        .iter()
        .map(|r| {
            let name = app
                .effect_list
                .find(r.effect_id())
                .map(|e| {
                    if e.label.is_empty() {
                        format!("Effect {}", e.id)
                    } else {
                        e.label.clone()
                    }
                })
                .unwrap_or_else(|| format!("Effect {}", r.effect_id()));
            format!(
                "{} on {} fixture{}{}",
                name,
                r.fixture_ids().len(),
                if r.fixture_ids().len() == 1 { "" } else { "s" },
                if r.is_stopping() { " (stopping)" } else { "" },
            )
        })
        .collect();

    if !active.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Running: {}", active.join(", "))).small());
        ui.label(
            egui::RichText::new(
                "Modulated channels show live values in cyan — recording still captures the base look.",
            )
            .small()
            .italics()
            .color(egui::Color32::GRAY),
        );
    }
}
