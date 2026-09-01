//! Submasters panel: horizontal row of theatrical submaster faders.

use crate::app::EasyCueApp;
use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::regular as ph;

const COLUMN_WIDTH: f32 = 76.0;

/// Render the submaster faders and their edit-only record/name controls.
pub fn render_submasters_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    if app.show_mode {
        app.submaster_state.edit_mode = false;
    }
    let mut edit_mode = app.submaster_state.edit_mode;
    ui.horizontal(|ui| {
        let label = if edit_mode { "▶ Live" } else { "✏ Edit" };
        if !app.show_mode && ui.toggle_value(&mut edit_mode, label).changed() {
            app.submaster_state.edit_mode = edit_mode;
        }
    });
    ui.separator();

    if app.submasters.is_empty() && !edit_mode {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label("No subs defined. Click 'Edit' to add some.");
        });
        return;
    }

    let slider_height = (ui.available_height() - 78.0).max(80.0);
    let column_height = slider_height + 78.0;
    let mut record_index = None;
    let mut add_requested = false;

    egui::ScrollArea::horizontal()
        .id_salt("submasters_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, submaster) in app.submasters.iter_mut().enumerate() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(COLUMN_WIDTH, column_height),
                        Layout::top_down(Align::Center),
                        |ui| {
                            if edit_mode {
                                ui.add(
                                    egui::TextEdit::singleline(&mut submaster.name)
                                        .desired_width(COLUMN_WIDTH)
                                        .horizontal_align(egui::Align::Center),
                                );
                            } else {
                                ui.add_sized(
                                    [COLUMN_WIDTH, 20.0],
                                    egui::Label::new(
                                        RichText::new(&submaster.name).strong().size(12.0),
                                    )
                                    .truncate(),
                                );
                            }

                            let mut level = submaster.level;
                            if ui
                                .add_sized(
                                    [COLUMN_WIDTH - 18.0, slider_height],
                                    egui::Slider::new(&mut level, 0..=100)
                                        .vertical()
                                        .show_value(false),
                                )
                                .changed()
                            {
                                submaster.level = level;
                            }

                            ui.label(format!("{}%", submaster.level));
                            if edit_mode {
                                if ui
                                    .small_button(
                                        RichText::new(format!("{} Record", ph::CIRCLE))
                                            .color(egui::Color32::from_rgb(245, 80, 80)),
                                    )
                                    .on_hover_text(
                                        "Capture the current live stage levels (without effects)",
                                    )
                                    .clicked()
                                {
                                    record_index = Some(index);
                                }
                            }
                        },
                    );
                }

                if edit_mode {
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::vec2(COLUMN_WIDTH, column_height),
                        Layout::top_down(Align::Center),
                        |ui| {
                            ui.add_space(20.0);
                            if ui
                                .add_sized(
                                    [COLUMN_WIDTH - 12.0, slider_height],
                                    egui::Button::new(RichText::new("+").size(26.0)),
                                )
                                .on_hover_text("Add a submaster")
                                .clicked()
                            {
                                add_requested = true;
                            }
                            ui.label(RichText::new("Add a submaster").small());
                        },
                    );
                }
            });
        });

    if let Some(index) = record_index {
        app.record_submaster(index);
    }
    if add_requested {
        app.add_submaster();
    }
}
