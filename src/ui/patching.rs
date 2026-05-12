//! Fixture patching panel UI
//!
//! Interface for managing fixture patches (assigning fixtures to DMX addresses).

use crate::app::EasyCueApp;
use egui::{Color32, RichText, ScrollArea};
use egui_phosphor::regular as ph;

/// State for the patching panel
#[derive(Default)]
pub struct PatchingPanelState {
    pub show_patch_dialog: bool,
    pub editing_patch_id: Option<usize>,
    pub label_input: String,
    pub selected_profile_id: String,
    pub address_input: String,
    pub fixture_number_input: String,
    pub quantity: u32,
    pub error_message: String,
    pub show_clear_confirm: bool,
    pub show_one_to_one_dialog: bool,
    pub one_to_one_count_input: String,
    pub one_to_one_error: String,
    /// Universe number for the add-fixture dialog (1-based, default 1).
    pub universe_input: u16,
}

impl PatchingPanelState {
    pub fn open_add_dialog(&mut self, default_profile: Option<String>, next_fixture_id: usize) {
        self.show_patch_dialog = true;
        self.editing_patch_id = None;
        self.label_input.clear();
        self.selected_profile_id = default_profile.unwrap_or_default();
        self.address_input.clear();
        self.fixture_number_input = next_fixture_id.to_string();
        self.quantity = 1;
        self.error_message.clear();
        if self.universe_input == 0 { self.universe_input = 1; }
    }

    pub fn close_dialog(&mut self) {
        self.show_patch_dialog = false;
        self.error_message.clear();
    }
}

/// Render the patching panel
pub fn render_patching_panel(ui: &mut egui::Ui, app: &mut EasyCueApp, state: &mut PatchingPanelState) {
    ui.heading("Fixture Patch");

    ui.horizontal(|ui| {
        if ui.button(format!("{} Add Fixture", ph::PLUS)).clicked() {
            let default_profile = app.fixtures.profile_ids().first().cloned();
            let next_id = app.fixtures.next_available_fixture_id();
            state.open_add_dialog(default_profile, next_id);
        }

        ui.separator();

        if ui
            .add(egui::Button::new(egui::RichText::new(format!("{} 1-to-1", ph::ARROWS_HORIZONTAL)).color(egui::Color32::RED)))
            .clicked()
        {
            state.show_one_to_one_dialog = true;
            state.one_to_one_count_input.clear();
            state.one_to_one_error.clear();
        }

        if ui
            .add(egui::Button::new(egui::RichText::new(format!("{} Clear Patch", ph::TRASH)).color(egui::Color32::RED)))
            .clicked()
        {
            state.show_clear_confirm = true;
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
            .column(egui_extras::Column::exact(55.0)) // Universe
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
                    ui.strong("Univ.");
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

                let mut to_remove: Option<usize> = None;
                let mut label_updates: Vec<(usize, String)> = Vec::new();
                let mut profile_updates: Vec<(usize, String, u16)> = Vec::new();
                let mut address_updates: Vec<(usize, u16, u16)> = Vec::new();
                let mut id_updates: Vec<(usize, usize)> = Vec::new();
                let mut universe_updates: Vec<(usize, u16)> = Vec::new();

                for (patch, profile_name, profile_missing, channel_count) in patch_data {
                    body.row(24.0, |mut row| {
                        // ID (editable)
                        row.col(|ui| {
                            let mut id_val = patch.id as i32;
                            let resp = ui.add(
                                egui::DragValue::new(&mut id_val).range(1..=9999).speed(1.0)
                            );
                            if resp.changed() && id_val >= 1 {
                                id_updates.push((patch.id, id_val as usize));
                            }
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

                        // Universe (1-based)
                        row.col(|ui| {
                            let mut uni = patch.universe.max(1) as i32;
                            let resp = ui.add(
                                egui::DragValue::new(&mut uni).range(1..=16).speed(1.0)
                            );
                            if resp.changed() && uni >= 1 {
                                universe_updates.push((patch.id, uni as u16));
                            }
                        });

                        // Actions
                        row.col(|ui| {
                            if ui.small_button(ph::TRASH).clicked() {
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
                        .update_patch_address(id, new_start_address, channel_count)
                    {
                        app.ui_state.status_message = format!("Error: {}", e);
                    }
                }

                for (old_id, new_id) in id_updates {
                    // Drop VirtualIntensity state for the old ID so it reinitialises cleanly.
                    app.virtual_intensity.remove_fixture(old_id);
                    if let Err(e) = app.fixtures.rename_fixture_id(old_id, new_id) {
                        app.ui_state.status_message = format!("Error: {}", e);
                    }
                }

                for (id, new_universe) in universe_updates {
                    if let Some(patch) = app.fixtures.patch_list_mut().get_patch_mut(id) {
                        patch.universe = new_universe;
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

    // Clear patch confirmation dialog
    if state.show_clear_confirm {
        egui::Window::new("Clear Patch")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.set_min_width(300.0);
                ui.label(RichText::new("This will delete all patched fixtures.").color(egui::Color32::RED));
                ui.label("This cannot be undone.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        state.show_clear_confirm = false;
                    }
                    ui.add_space(10.0);
                    if ui
                        .add(egui::Button::new("Clear All Fixtures").fill(egui::Color32::DARK_RED))
                        .clicked()
                    {
                        app.fixtures.patch_list_mut().clear();
                        app.virtual_intensity.clear();
                        app.ui_state.status_message = "Patch cleared".to_string();
                        state.show_clear_confirm = false;
                    }
                });
            });
    }

    // 1-to-1 dialog
    if state.show_one_to_one_dialog {
        egui::Window::new("1-to-1 Patch")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.set_min_width(300.0);
                ui.label(RichText::new("This will delete all patched fixtures...").color(egui::Color32::RED));
                ui.label("...and create simple dimmer channels.");
                ui.label("This cannot be undone.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("How many channels to create:");
                    ui.text_edit_singleline(&mut state.one_to_one_count_input);
                });
                if !state.one_to_one_error.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&state.one_to_one_error).color(egui::Color32::RED));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        state.show_one_to_one_dialog = false;
                        state.one_to_one_error.clear();
                    }
                    ui.add_space(10.0);
                    if ui.button("Create").clicked() {
                        match state.one_to_one_count_input.trim().parse::<u16>() {
                            Ok(count) if count >= 1 && count <= 512 => {
                                app.fixtures.patch_list_mut().clear();
                                app.virtual_intensity.clear();
                                let mut errors: Vec<String> = Vec::new();
                                for i in 0..count {
                                    let fid = (i + 1) as usize;
                                    let addr = i + 1;
                                    let label = format!("Ch{}", fid);
                                    match app.fixtures.add_patch_with_id(
                                        fid,
                                        label,
                                        "generic_dimmer".to_string(),
                                        addr,
                                        1, // 1-to-1 always universe 1
                                    ) {
                                        Ok(_) => {}
                                        Err(e) => errors.push(e.to_string()),
                                    }
                                }
                                if errors.is_empty() {
                                    app.ui_state.status_message =
                                        format!("Created {count} dimmer channels");
                                    state.show_one_to_one_dialog = false;
                                    state.one_to_one_error.clear();
                                } else {
                                    state.one_to_one_error = errors.join("; ");
                                }
                            }
                            Ok(_) => {
                                state.one_to_one_error =
                                    "Enter a number between 1 and 512".to_string();
                            }
                            Err(_) => {
                                state.one_to_one_error = "Enter a valid number".to_string();
                            }
                        }
                    }
                });
            });
    }
}

/// Render the add/edit patch dialog
fn render_patch_dialog(ui: &mut egui::Ui, app: &mut EasyCueApp, state: &mut PatchingPanelState) {
    let is_editing = state.editing_patch_id.is_some();
    let title = if is_editing { "Edit Fixture" } else { "Add Fixture" };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.set_min_width(400.0);

            egui::Grid::new("add_fixture_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    // Fixture number
                    ui.label("Fixture #:");
                    ui.text_edit_singleline(&mut state.fixture_number_input);
                    ui.end_row();

                    // Label (optional)
                    ui.label("Label (optional):");
                    ui.text_edit_singleline(&mut state.label_input);
                    ui.end_row();

                    // Profile selector
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
                    ui.end_row();

                    // Channel count info
                    if let Some(profile) = app.fixtures.get_profile(&state.selected_profile_id) {
                        ui.label("");
                        ui.label(RichText::new(format!("{} channels", profile.channel_count)).color(Color32::GRAY));
                        ui.end_row();
                    }

                    // DMX start address
                    ui.label("DMX Address:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut state.address_input);
                        ui.label(RichText::new("(1–512)").color(Color32::GRAY).small());
                    });
                    ui.end_row();

                    // Universe
                    ui.label("Universe:");
                    ui.horizontal(|ui| {
                        if state.universe_input == 0 { state.universe_input = 1; }
                        let mut uni = state.universe_input as i32;
                        ui.add(egui::DragValue::new(&mut uni).range(1..=16).speed(1.0));
                        state.universe_input = uni as u16;
                        ui.label(RichText::new("(1 = default)").color(Color32::GRAY).small());
                    });
                    ui.end_row();

                    // Quantity (only shown for new patches)
                    if !is_editing {
                        ui.label("Quantity:");
                        let mut qty = state.quantity.max(1) as i32;
                        if ui.add(egui::DragValue::new(&mut qty).range(1..=50)).changed() {
                            state.quantity = qty as u32;
                        }
                        ui.end_row();

                        // Preview end address
                        if let Some(profile) = app.fixtures.get_profile(&state.selected_profile_id) {
                            if let Ok(addr) = state.address_input.trim().parse::<u16>() {
                                let end = addr + profile.channel_count * state.quantity as u16 - 1;
                                ui.label("");
                                let color = if end > 512 { Color32::RED } else { Color32::GRAY };
                                ui.label(RichText::new(format!("uses DMX {addr}–{end}")).color(color).small());
                                ui.end_row();
                            }
                        }
                    }
                });

            if !state.error_message.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(&state.error_message).color(Color32::RED));
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    state.close_dialog();
                }

                ui.add_space(10.0);

                let can_submit = !state.selected_profile_id.is_empty()
                    && !state.address_input.is_empty()
                    && !state.fixture_number_input.trim().is_empty();

                let btn_label: String = if is_editing {
                    "Update".into()
                } else if state.quantity > 1 {
                    format!("Add {}", state.quantity)
                } else {
                    "Add".into()
                };

                if ui.add_enabled(can_submit, egui::Button::new(btn_label)).clicked() {
                    let addr_result = state.address_input.trim().parse::<u16>();
                    let fnum_result = state.fixture_number_input.trim().parse::<usize>();
                    match (addr_result, fnum_result) {
                        (Ok(address), Ok(fixture_num)) => {
                            if address < 1 || address > 512 {
                                state.error_message = "DMX address must be 1–512".to_string();
                            } else if fixture_num < 1 {
                                state.error_message = "Fixture number must be ≥ 1".to_string();
                            } else if is_editing {
                                state.error_message = "Edit not yet implemented".to_string();
                            } else {
                                let qty = state.quantity.max(1) as usize;
                                let ch_count = app.fixtures
                                    .get_profile(&state.selected_profile_id)
                                    .map(|p| p.channel_count as usize)
                                    .unwrap_or(1);
                                let universe = state.universe_input.max(1);
                                let mut last_id = 0usize;
                                let mut errors: Vec<String> = Vec::new();
                                for i in 0..qty {
                                    let fid = fixture_num + i;
                                    let addr = address + (i * ch_count) as u16;
                                    let label = if state.label_input.is_empty() {
                                        String::new()
                                    } else if qty == 1 {
                                        state.label_input.clone()
                                    } else {
                                        format!("{} {}", state.label_input, i + 1)
                                    };
                                    match app.fixtures.add_patch_with_id(
                                        fid, label,
                                        state.selected_profile_id.clone(), addr, universe,
                                    ) {
                                        Ok(id) => { last_id = id; }
                                        Err(e) => { errors.push(format!("#{fid}: {e}")); }
                                    }
                                }
                                if errors.is_empty() {
                                    app.ui_state.status_message = if qty == 1 {
                                        format!("Added fixture #{} at {}", last_id, address)
                                    } else {
                                        format!("Added {} fixtures starting at #{}", qty, fixture_num)
                                    };
                                    state.close_dialog();
                                } else {
                                    state.error_message = errors.join("; ");
                                }
                            }
                        }
                        (Err(_), _) => {
                            state.error_message = "Invalid DMX address".to_string();
                        }
                        (_, Err(_)) => {
                            state.error_message = "Fixture number must be a positive integer".to_string();
                        }
                    }
                }
            });
        });
}
