//! Fixture patching panel UI
//!
//! Interface for managing fixture patches (assigning fixtures to DMX addresses).

use crate::app::EasyCueApp;
use crate::fixtures::Patch;
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

    /// Open dialog to edit an existing patch
    pub fn open_edit_dialog(&mut self, patch: &Patch, profile_id: String) {
        self.show_patch_dialog = true;
        self.editing_patch_id = Some(patch.id);
        self.label_input = patch.label.clone();
        self.selected_profile_id = profile_id;
        self.address_input = patch.start_address.to_string();
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
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(40.0)) // ID
            .column(egui_extras::Column::initial(120.0).at_least(80.0)) // Label
            .column(egui_extras::Column::initial(150.0).at_least(100.0)) // Type
            .column(egui_extras::Column::exact(80.0)) // Address
            .column(egui_extras::Column::exact(80.0)) // Channels
            .column(egui_extras::Column::exact(100.0)) // Actions
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
                    ui.strong("Channels");
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
                let mut to_edit: Option<(Patch, String)> = None;

                for (patch, profile_name, profile_missing, channel_count) in patch_data {
                    let end_address = patch.start_address + channel_count - 1;

                    body.row(24.0, |mut row| {
                        // ID
                        row.col(|ui| {
                            ui.label(patch.id.to_string());
                        });

                        // Label
                        row.col(|ui| {
                            ui.label(&patch.label);
                        });

                        // Type (profile name)
                        row.col(|ui| {
                            if profile_missing {
                                ui.label(RichText::new(&profile_name).color(Color32::YELLOW));
                            } else {
                                ui.label(&profile_name);
                            }
                        });

                        // Address range
                        row.col(|ui| {
                            ui.label(format!("{}-{}", patch.start_address, end_address));
                        });

                        // Channel count
                        row.col(|ui| {
                            ui.label(format!("{} ch", channel_count));
                        });

                        // Actions
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("✏").clicked() {
                                    to_edit = Some((patch.clone(), patch.profile_id.clone()));
                                }

                                if ui.small_button("🗑").clicked() {
                                    to_remove = Some(patch.id);
                                }
                            });
                        });
                    });
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

                if let Some((patch, profile_id)) = to_edit {
                    state.open_edit_dialog(&patch, profile_id);
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
