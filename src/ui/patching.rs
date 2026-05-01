//! Fixture patching panel UI
//!
//! Interface for managing fixture patches (assigning fixtures to DMX addresses).

use crate::app::EasyCueApp;
use egui::{Color32, RichText, ScrollArea};

/// State for the patching panel
#[derive(Default)]
pub struct PatchingPanelState {
    /// Show add/edit patch dialog
    pub show_patch_dialog: bool,
    /// ID of patch being edited (None for new patch)
    pub editing_patch_id: Option<usize>,
    /// Input fields for patch dialog
    pub label_input: String,
    pub selected_profile_id: String,
    pub address_input: String,
    pub error_message: String,
}

impl PatchingPanelState {
    /// Open dialog to add a new patch
    pub fn open_add_dialog(&mut self, default_profile: Option<String>) {
        self.show_patch_dialog = true;
        self.editing_patch_id = None;
        self.label_input.clear();
        self.selected_profile_id = default_profile.unwrap_or_default();
        self.address_input.clear();
        self.error_message.clear();
    }

    /// Close the patch dialog
    pub fn close_dialog(&mut self) {
        self.show_patch_dialog = false;
        self.error_message.clear();
    }
}

/// Render the patching panel
pub fn render_patching_panel(ui: &mut egui::Ui, app: &mut EasyCueApp, state: &mut PatchingPanelState) {
    ui.heading("Fixture Patch");

    ui.horizontal(|ui| {
        if ui.button("➕ Add Fixture").clicked() {
            // Default to first profile if available
            let default_profile = app.fixtures.profile_ids().first().cloned();
            state.open_add_dialog(default_profile);
        }

        ui.separator();

        ui.label(format!(
            "{} fixtures patched",
            app.fixtures.patch_list().len()
        ));
    });

    ui.separator();

    // Patch table
    ScrollArea::vertical().show(ui, |ui| {
        let profile_options: Vec<(String, String, u16)> = app
            .fixtures
            .profile_ids()
            .into_iter()
            .filter_map(|profile_id| {
                app.fixtures
                    .get_profile(&profile_id)
                    .map(|profile| (profile_id, profile.name.clone(), profile.channel_count))
            })
            .collect();

        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(40.0)) // ID
            .column(egui_extras::Column::initial(120.0).at_least(80.0)) // Label
            .column(egui_extras::Column::initial(150.0).at_least(100.0)) // Type
            .column(egui_extras::Column::exact(140.0)) // Address
            .column(egui_extras::Column::exact(70.0)) // Actions
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("#");
                });
                header.col(|ui| {
                    ui.strong("Label");
                });
                header.col(|ui| {
                    ui.strong("Type");
                });
                header.col(|ui| {
                    ui.strong("Address");
                });
                header.col(|ui| {
                    ui.strong("Actions");
                });
            })
            .body(|mut body| {
                // Collect patch data with profile info to avoid borrow issues
                let patch_data: Vec<_> = app
                    .fixtures
                    .patch_list()
                    .patches()
                    .iter()
                    .map(|patch| {
                        let profile = app.fixtures.get_profile(&patch.profile_id);
                        let channel_count = profile.map(|p| p.channel_count).unwrap_or(1);
                        let profile_name = profile
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| format!("⚠ {}", patch.profile_id));
                        let profile_missing = profile.is_none();
                        (
                            patch.clone(),
                            profile_name,
                            profile_missing,
                            channel_count,
                        )
                    })
                    .collect();

                // Track patches to remove (can't remove during iteration)
                let mut to_remove: Option<usize> = None;
                let mut label_updates: Vec<(usize, String)> = Vec::new();
                let mut profile_updates: Vec<(usize, String, u16)> = Vec::new();
                let mut address_updates: Vec<(usize, u16, u16)> = Vec::new();

                for (patch, profile_name, profile_missing, channel_count) in patch_data {
                    body.row(24.0, |mut row| {
                        // ID
                        row.col(|ui| {
                            ui.label(patch.id.to_string());
                        });

                        // Label (inline editable)
                        row.col(|ui| {
                            let mut new_label = patch.label.clone();
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut new_label)
                                    .desired_width(ui.available_width())
                            );
                            if response.changed() {
                                label_updates.push((patch.id, new_label));
                            }
                        });

                        // Type (inline dropdown)
                        row.col(|ui| {
                            let selected_text = if profile_missing {
                                format!("⚠ {}", patch.profile_id)
                            } else {
                                profile_name.clone()
                            };

                            egui::ComboBox::from_id_salt(format!("patch_type_{}", patch.id))
                                .selected_text(selected_text)
                                .show_ui(ui, |ui| {
                                    for (option_profile_id, option_name, option_channels) in &profile_options {
                                        let is_selected = patch.profile_id == *option_profile_id;
                                        if ui.selectable_label(is_selected, option_name).clicked() {
                                            profile_updates.push((
                                                patch.id,
                                                option_profile_id.clone(),
                                                *option_channels,
                                            ));
                                        }
                                    }
                                });
                        });

                        // Address start + computed end
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                let mut start_address = patch.start_address;
                                let response = ui.add(
                                    egui::DragValue::new(&mut start_address)
                                        .range(1..=512)
                                        .speed(1.0)
                                );

                                let end_address = (u32::from(start_address) + u32::from(channel_count) - 1) as u16;
                                ui.label(format!("-{}", end_address));

                                if response.changed() {
                                    address_updates.push((patch.id, start_address, channel_count));
                                }
                            });
                        });

                        // Actions
                        row.col(|ui| {
                            if ui.small_button("🗑").clicked() {
                                to_remove = Some(patch.id);
                            }
                        });
                    });
                }

                for (id, new_label) in label_updates {
                    if let Some(target_patch) = app.fixtures.patch_list_mut().get_patch_mut(id) {
                        target_patch.label = new_label;
                    }
                }

                for (id, new_profile_id, new_channel_count) in profile_updates {
                    let current_start = app
                        .fixtures
                        .patch_list()
                        .get_patch(id)
                        .map(|p| p.start_address);

                    if let Some(start_address) = current_start {
                        match app
                            .fixtures
                            .patch_list_mut()
                            .update_patch_address(id, start_address, new_channel_count)
                        {
                            Ok(()) => {
                                if let Some(target_patch) = app.fixtures.patch_list_mut().get_patch_mut(id) {
                                    target_patch.profile_id = new_profile_id;
                                }
                            }
                            Err(e) => {
                                app.ui_state.status_message = format!("Error: {}", e);
                            }
                        }
                    }
                }

                for (id, new_start_address, channel_count) in address_updates {
                    if let Err(e) = app
                        .fixtures
                        .patch_list_mut()
                        .update_patch_address(id, new_start_address, channel_count)
                    {
                        app.ui_state.status_message = format!("Error: {}", e);
                    }
                }

                // Process actions after table rendering
                if let Some(id) = to_remove {
                    if let Err(e) = app.fixtures.remove_patch(id) {
                        log::error!("Failed to remove patch: {}", e);
                        app.ui_state.status_message = format!("Error: {}", e);
                    } else {
                        app.ui_state.status_message = "Removed fixture".to_string();
                    }
                }
            });
    });

    // Render patch dialog if open
    if state.show_patch_dialog {
        render_patch_dialog(ui, app, state);
    }
}

/// Render the add/edit patch dialog
fn render_patch_dialog(ui: &mut egui::Ui, app: &mut EasyCueApp, state: &mut PatchingPanelState) {
    let is_editing = state.editing_patch_id.is_some();
    let title = if is_editing {
        "Edit Fixture"
    } else {
        "Add Fixture"
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.set_min_width(400.0);

            // Label input
            ui.horizontal(|ui| {
                ui.label("Label:");
                ui.text_edit_singleline(&mut state.label_input);
            });

            ui.add_space(8.0);

            // Profile selector
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("profile_selector")
                    .selected_text(if state.selected_profile_id.is_empty() {
                        "Select profile..."
                    } else {
                        app.fixtures
                            .get_profile(&state.selected_profile_id)
                            .map(|p| p.name.as_str())
                            .unwrap_or(&state.selected_profile_id)
                    })
                    .show_ui(ui, |ui| {
                        let profile_ids = app.fixtures.profile_ids();
                        for profile_id in profile_ids {
                            if let Some(profile) = app.fixtures.get_profile(&profile_id) {
                                let selected = state.selected_profile_id == profile_id;
                                if ui.selectable_label(selected, &profile.name).clicked() {
                                    state.selected_profile_id = profile_id.clone();
                                }
                            }
                        }
                    });
            });

            // Show channel count for selected profile
            if let Some(profile) = app.fixtures.get_profile(&state.selected_profile_id) {
                ui.label(
                    RichText::new(format!("({} channels)", profile.channel_count))
                        .color(Color32::GRAY),
                );
            }

            ui.add_space(8.0);

            // Address input
            ui.horizontal(|ui| {
                ui.label("DMX Address:");
                ui.text_edit_singleline(&mut state.address_input);
                ui.label("(1-512)");
            });

            ui.add_space(8.0);

            // Show error message if any
            if !state.error_message.is_empty() {
                ui.label(RichText::new(&state.error_message).color(Color32::RED));
                ui.add_space(8.0);
            }

            // Buttons
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    state.close_dialog();
                }

                ui.add_space(10.0);

                let can_submit = !state.label_input.is_empty()
                    && !state.selected_profile_id.is_empty()
                    && !state.address_input.is_empty();

                if ui
                    .add_enabled(can_submit, egui::Button::new(if is_editing { "Update" } else { "Add" }))
                    .clicked()
                {
                    // Parse address
                    match state.address_input.parse::<u16>() {
                        Ok(address) if address >= 1 && address <= 512 => {
                            // Attempt to add or update patch
                            let result = if is_editing {
                                // TODO: Implement patch update
                                state.error_message =
                                    "Editing not yet implemented".to_string();
                                Err(anyhow::anyhow!("Not implemented"))
                            } else {
                                app.fixtures.add_patch(
                                    state.label_input.clone(),
                                    state.selected_profile_id.clone(),
                                    address,
                                )
                            };

                            match result {
                                Ok(id) => {
                                    app.ui_state.status_message = format!(
                                        "Added fixture #{}: {} at address {}",
                                        id, state.label_input, address
                                    );
                                    state.close_dialog();
                                }
                                Err(e) => {
                                    state.error_message = e.to_string();
                                }
                            }
                        }
                        Ok(_) => {
                            state.error_message = "Address must be between 1 and 512".to_string();
                        }
                        Err(_) => {
                            state.error_message = "Invalid address format".to_string();
                        }
                    }
                }
            });
        });
}
