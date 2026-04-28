//! User interface components
//!
//! egui-based UI panels and widgets.

use egui::{Context, Ui};
use crate::app::EasyCueApp;

/// Render the main UI
pub fn render(ctx: &Context, app: &mut EasyCueApp) {
    // Top panel - menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Show").clicked() {
                    app.cue_list.clear();
                    app.playback.stop();
                    app.show_title = "New Show".to_string();
                    app.ui_state.show_title_input = "New Show".to_string();
                    app.ui_state.selected_cue_index = None;
                    app.ui_state.status_message = "New show created".to_string();
                    log::info!("New show created");
                    ui.close_menu();
                }
                if ui.button("Open… (Ctrl+O)").clicked() {
                    app.ui_state.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    app.ui_state.show_save_dialog = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut app.ui_state.show_fixture_panel, "Fixture Control");
                ui.checkbox(&mut app.ui_state.show_media_panel, "Media Panel");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    log::info!("Keyboard shortcuts: Space=GO, B=BACK, S=STOP, Ctrl+R=Record, Ctrl+S=Save, Ctrl+O=Open");
                }
                if ui.button("About").clicked() {
                    log::info!("EasyCue3 - Theatrical Lighting & Media Console");
                }
            });

            // Show title in menu bar
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(&app.show_title);
                ui.separator();
            });
        });
    });

    // Bottom panel - transport controls
    egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
        render_transport_panel(ui, app);
    });

    // Left panel - cue list (takes priority)
    egui::SidePanel::left("cue_list")
        .default_width(300.0)
        .show(ctx, |ui| {
            render_cue_list_panel(ui, app);
        });

    // Right panel - fixture control (optional)
    if app.ui_state.show_fixture_panel {
        egui::SidePanel::right("fixture_control")
            .default_width(250.0)
            .show(ctx, |ui| {
                render_fixture_panel(ui, app);
            });
    }

    // Center panel - media/workspace or cue editor
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.ui_state.show_media_panel {
            render_media_panel(ui, app);
        } else if app.ui_state.selected_cue_index.is_some() {
            render_cue_editor(ui, app);
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.heading("EasyCue3");
                ui.label("Theatrical Lighting & Media Console");
                ui.add_space(20.0);
                if app.cue_list.is_empty() {
                    ui.label("Press Ctrl+R to record your first cue ✨");
                } else {
                    ui.label("Press Space or GO to start your show ✨");
                    ui.add_space(10.0);
                    ui.label("Click a cue in the list to edit it");
                }
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);
                render_keyboard_shortcut_help(ui);
            });
        }
    });

    // Open show dialog
    if app.ui_state.show_open_dialog {
        render_open_dialog(ctx, app);
    }

    // Save show dialog
    if app.ui_state.show_save_dialog {
        render_save_dialog(ctx, app);
    }
}

/// Render keyboard shortcut reference
fn render_keyboard_shortcut_help(ui: &mut Ui) {
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing([20.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Keyboard Shortcuts").strong());
            ui.end_row();
            ui.label("Space");        ui.label("GO (advance to next cue)");  ui.end_row();
            ui.label("B");            ui.label("BACK (return to previous cue)"); ui.end_row();
            ui.label("S");            ui.label("STOP playback");                 ui.end_row();
            ui.label("Ctrl+R");       ui.label("Record new cue");                ui.end_row();
            ui.label("Ctrl+S");       ui.label("Save show");                     ui.end_row();
            ui.label("Ctrl+O");       ui.label("Open show");                     ui.end_row();
        });
}

/// Render the cue list panel
fn render_cue_list_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    ui.heading("Cue List");
    ui.separator();

    let selected = app.ui_state.selected_cue_index;
    let current = app.cue_list.current_index();

    egui::ScrollArea::vertical()
        .id_salt("cue_list_scroll")
        .show(ui, |ui| {
        if app.cue_list.is_empty() {
            ui.label("No cues");
            ui.label("Record your first cue to begin (Ctrl+R)");
        } else {
            let cue_count = app.cue_list.len();
            let mut clicked_index: Option<usize> = None;

            for idx in 0..cue_count {
                if let Some(cue) = app.cue_list.get_cue(idx) {
                    let is_current  = Some(idx) == current;
                    let is_selected = Some(idx) == selected;

                    let bg_color = if is_current && is_selected {
                        egui::Color32::from_rgb(80, 120, 160)
                    } else if is_current {
                        egui::Color32::from_rgb(60, 60, 100)
                    } else if is_selected {
                        egui::Color32::from_rgb(60, 80, 60)
                    } else {
                        egui::Color32::from_gray(40)
                    };

                    let label_text = format!(
                        "{:.1}  {}",
                        cue.number,
                        if cue.label.is_empty() { "(untitled)" } else { &cue.label }
                    );
                    let fade_text = format!("{:.1}s", cue.fade_up);
                    let ch_count  = cue.channel_values.len();

                    let response = egui::Frame::new()
                        .fill(bg_color)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(&label_text);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(format!("{}ch", ch_count));
                                        ui.label(&fade_text);
                                    },
                                );
                            });
                        })
                        .response;

                    if response.interact(egui::Sense::click()).clicked() {
                        clicked_index = Some(idx);
                    }

                    ui.add_space(2.0);
                }
            }

            if let Some(idx) = clicked_index {
                if selected == Some(idx) {
                    // Toggle off if already selected
                    app.ui_state.selected_cue_index = None;
                } else {
                    app.ui_state.selected_cue_index = Some(idx);
                }
            }
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Record (Ctrl+R)").clicked() {
            let idx = app.record_cue();
            app.ui_state.selected_cue_index = Some(idx);
        }
        if ui.button("Delete").clicked() {
            if let Some(sel_idx) = app.ui_state.selected_cue_index {
                if app.cue_list.remove_cue(sel_idx).is_ok() {
                    app.ui_state.selected_cue_index = None;
                    app.ui_state.status_message = "Cue deleted".to_string();
                }
            } else {
                app.ui_state.status_message = "Select a cue first".to_string();
            }
        }
    });
}

/// Render the cue editor panel (shown in the center when a cue is selected)
fn render_cue_editor(ui: &mut Ui, app: &mut EasyCueApp) {
    let Some(idx) = app.ui_state.selected_cue_index else {
        return;
    };
    let Some(cue) = app.cue_list.get_cue_mut(idx) else {
        app.ui_state.selected_cue_index = None;
        return;
    };

    ui.heading(format!("Edit Cue {:.1}", cue.number));
    ui.separator();

    egui::Grid::new("cue_editor_grid")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Label:");
            ui.text_edit_singleline(&mut cue.label);
            ui.end_row();

            ui.label("Fade Up (s):");
            ui.add(egui::DragValue::new(&mut cue.fade_up)
                .range(0.0..=300.0)
                .speed(0.1));
            ui.end_row();

            ui.label("Fade Down (s):");
            ui.add(egui::DragValue::new(&mut cue.fade_down)
                .range(0.0..=300.0)
                .speed(0.1));
            ui.end_row();

            ui.label("Notes:");
            ui.text_edit_multiline(&mut cue.notes);
            ui.end_row();
        });

    ui.separator();
    ui.label(format!("Channel values: {} channels stored", cue.channel_values.len()));

    if !cue.channel_values.is_empty() {
        ui.add_space(4.0);
        // Show non-zero channels (up to 20)
        let mut sorted_channels: Vec<(u16, u8)> = cue.channel_values.iter()
            .map(|(&ch, &val)| (ch, val))
            .collect();
        sorted_channels.sort_by_key(|(ch, _)| *ch);

        egui::ScrollArea::vertical()
            .id_salt("channel_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("channel_grid")
                    .num_columns(4)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        for (i, (ch, val)) in sorted_channels.iter().enumerate() {
                            ui.label(format!("Ch {}", ch));
                            ui.label(format!("{}", val));
                            if (i + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    ui.separator();
    if ui.button("Close Editor").clicked() {
        app.ui_state.selected_cue_index = None;
    }
}

/// Render transport controls (GO/BACK/STOP)
fn render_transport_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);

        // Status indicator
        let state_text = match app.playback.state() {
            crate::cue::CueState::Stopped => "⏹ Stopped".to_string(),
            crate::cue::CueState::Fading { progress } => {
                format!("⏵ Fading {:.0}%", progress * 100.0)
            }
            crate::cue::CueState::Active => "⏸ Active".to_string(),
        };
        ui.label(&state_text);

        // Current cue indicator
        if let Some(idx) = app.cue_list.current_index() {
            if let Some(cue) = app.cue_list.get_cue(idx) {
                ui.separator();
                ui.label(format!("Cue {:.1}: {}", cue.number, cue.label));
            }
        }

        // Status message
        if !app.ui_state.status_message.is_empty() {
            ui.separator();
            ui.label(
                egui::RichText::new(&app.ui_state.status_message)
                    .color(egui::Color32::from_rgb(180, 180, 100)),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);

            if ui.button("⏹ STOP (S)").clicked() {
                app.playback.stop();
            }

            if ui.button("⏮ BACK (B)").clicked() {
                app.playback.back(&mut app.cue_list);
            }

            // Big GO button
            let go_button = egui::Button::new("⏵ GO (Space)")
                .fill(egui::Color32::from_rgb(50, 100, 50))
                .min_size(egui::vec2(100.0, 30.0));

            if ui.add(go_button).clicked() {
                app.playback.go(&mut app.cue_list);
            }
        });
    });
}

/// Render the open-show dialog
fn render_open_dialog(ctx: &Context, app: &mut EasyCueApp) {
    let mut open = app.ui_state.show_open_dialog;
    egui::Window::new("Open Show")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .min_width(380.0)
        .show(ctx, |ui| {
            ui.label("Show file path:");
            ui.text_edit_singleline(&mut app.ui_state.file_path_input);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    let path = std::path::PathBuf::from(&app.ui_state.file_path_input);
                    match app.load_show(&path) {
                        Ok(_) => {
                            app.ui_state.show_open_dialog = false;
                        }
                        Err(e) => {
                            app.ui_state.status_message = format!("Error: {}", e);
                            log::error!("Failed to load show: {}", e);
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.ui_state.show_open_dialog = false;
                }
            });
        });
    app.ui_state.show_open_dialog = open;
}

/// Render the save-show dialog
fn render_save_dialog(ctx: &Context, app: &mut EasyCueApp) {
    let mut open = app.ui_state.show_save_dialog;
    egui::Window::new("Save Show")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .min_width(380.0)
        .show(ctx, |ui| {
            ui.label("Show title:");
            ui.text_edit_singleline(&mut app.ui_state.show_title_input);
            ui.label("Save path:");
            ui.text_edit_singleline(&mut app.ui_state.file_path_input);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    let path = std::path::PathBuf::from(&app.ui_state.file_path_input);
                    let title = app.ui_state.show_title_input.clone();
                    match app.save_show(&path, &title) {
                        Ok(_) => {
                            app.show_title = title;
                            app.ui_state.status_message =
                                format!("Saved to {:?}", path);
                            app.ui_state.show_save_dialog = false;
                        }
                        Err(e) => {
                            app.ui_state.status_message = format!("Error: {}", e);
                            log::error!("Failed to save show: {}", e);
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.ui_state.show_save_dialog = false;
                }
            });
        });
    app.ui_state.show_save_dialog = open;
}

/// Render fixture control panel
fn render_fixture_panel(ui: &mut Ui, _app: &mut EasyCueApp) {
    ui.heading("Fixture Control");
    ui.separator();

    ui.label("Fixture selection and control");
    ui.label("Coming in Phase 4");

    // TODO: Fixture selection UI
    // TODO: Parameter sliders
}

/// Render media panel
fn render_media_panel(ui: &mut Ui, _app: &mut EasyCueApp) {
    ui.heading("Media Panel");
    ui.separator();

    ui.label("Audio/Video/Image playback");
    ui.label("Coming in Phase 5");

    // TODO: Media file browser
    // TODO: Playback controls
}
