//! Properties panel - shows details of selected channel or cue

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the properties panel
pub fn render_properties_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // Determine what's selected
    let has_channels = !app.ui_state.selected_channels.is_empty();
    let has_cue = app.ui_state.selected_cue_index.is_some();
    
    if !has_channels && !has_cue {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("No Selection").color(egui::Color32::GRAY));
            ui.add_space(10.0);
            ui.label("Select a channel or cue to view properties");
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Tip: Ctrl/Cmd+Click to select multiple channels").italics().small());
        });
        return;
    }
    
    // Show channel properties if channels are selected
    if has_channels {
        if app.ui_state.selected_channels.len() == 1 {
            let channel = *app.ui_state.selected_channels.iter().next().unwrap();
            render_single_channel_properties(ui, app, channel);
        } else {
            render_multi_channel_properties(ui, app);
        }
        ui.separator();
    }
    
    // Show cue properties if a cue is selected
    if let Some(cue_idx) = app.ui_state.selected_cue_index {
        render_cue_properties(ui, app, cue_idx);
    }
}

/// Render properties for a single selected channel
fn render_single_channel_properties(ui: &mut Ui, app: &mut EasyCueApp, channel: u16) {
    ui.label(egui::RichText::new(format!("Channel {}", channel)).strong());
    
    if let Some(universe) = app.universes.first_mut() {
        let mut value = universe.get_channel(channel).unwrap_or(0);
        
        ui.add_space(6.0);
        
        egui::Grid::new("channel_props")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Value:");
                if ui.add(egui::DragValue::new(&mut value).range(0..=100)).changed() {
                    let _ = universe.set_channel(channel, value);
                    // Update base level when manually changed
                    app.ui_state.channel_base_levels.insert(channel, value);
                    app.ui_state.group_master = value;
                }
                ui.end_row();
            });
    }
}

/// Render properties for multiple selected channels
fn render_multi_channel_properties(ui: &mut Ui, app: &mut EasyCueApp) {
    let channel_count = app.ui_state.selected_channels.len();
    ui.label(egui::RichText::new(format!("{} Channels Selected", channel_count)).strong());
    
    if let Some(universe) = app.universes.first_mut() {
        // Get all selected channel values
        let mut channel_values: Vec<(u16, u8)> = app.ui_state.selected_channels
            .iter()
            .map(|&ch| (ch, universe.get_channel(ch).unwrap_or(0)))
            .collect();
        channel_values.sort_by_key(|(ch, _)| *ch);
        
        // Calculate statistics
        let max_value = channel_values.iter().map(|(_, v)| *v).max().unwrap_or(0);
        let min_value = channel_values.iter().map(|(_, v)| *v).min().unwrap_or(0);
        let avg_value = if !channel_values.is_empty() {
            channel_values.iter().map(|(_, v)| *v as u32).sum::<u32>() / channel_values.len() as u32
        } else {
            0
        } as u8;
        
        ui.add_space(6.0);
        
        egui::Grid::new("multi_channel_props")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Channels:");
                ui.label(format!("{}", channel_count));
                ui.end_row();
                
                ui.label("Range:");
                ui.label(format!("{}-{}", min_value, max_value));
                ui.end_row();
                
                ui.label("Average:");
                ui.label(format!("{}", avg_value));
                ui.end_row();
            });
        
        ui.add_space(10.0);
        
        // Master slider for proportional control using O_i = M * L_i formula
        ui.label(egui::RichText::new("Group Master (Proportional)").strong());
        ui.add_space(4.0);
        
        let mut master_value = app.ui_state.group_master;
        let slider_changed = ui.add(
            egui::Slider::new(&mut master_value, 0..=100)
                .text("M")
        ).changed();
        
        if slider_changed {
            app.ui_state.group_master = master_value;
            
            // Find the max base level for normalization
            let max_base = app.ui_state.channel_base_levels.values().copied().max().unwrap_or(100);
            
            if max_base > 0 {
                // Apply O_i = M * (L_i / L_max) formula to all selected channels
                for &ch in &app.ui_state.selected_channels {
                    if let Some(&base_level) = app.ui_state.channel_base_levels.get(&ch) {
                        // O_i = M * (L_i / L_max)
                        let output = ((master_value as f32) * (base_level as f32) / (max_base as f32)).round() as u8;
                        let _ = universe.set_channel(ch, output.min(100));
                    }
                }
            } else {
                // All base levels are 0, set all to master value
                for &ch in &app.ui_state.selected_channels {
                    let _ = universe.set_channel(ch, master_value);
                }
            }
        }
        
        // Show base levels for reference
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Base Levels (L_i):").small().italics());
        ui.horizontal_wrapped(|ui| {
            let mut sorted_channels: Vec<u16> = app.ui_state.selected_channels.iter().copied().collect();
            sorted_channels.sort();
            for &ch in &sorted_channels {
                if let Some(&base) = app.ui_state.channel_base_levels.get(&ch) {
                    ui.label(format!("{}:{}", ch, base));
                }
            }
        });
    }
}

/// Render properties for a selected cue
fn render_cue_properties(ui: &mut Ui, app: &mut EasyCueApp, cue_idx: usize) {
    // Get cue info for display (immutable borrow first)
    let (cue_number, channel_count) = {
        let Some(cue) = app.cue_list.get_cue(cue_idx) else {
            app.ui_state.selected_cue_index = None;
            return;
        };
        (cue.number, cue.channel_values.len())
    };
    
    // Get mutable reference for editing
    let Some(cue) = app.cue_list.get_cue_mut(cue_idx) else {
        app.ui_state.selected_cue_index = None;
        return;
    };
    
    ui.label(egui::RichText::new(format!("Cue {:.1}", cue_number)).strong());
    ui.add_space(6.0);
    
    egui::Grid::new("cue_props")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("Number:");
            ui.add(egui::DragValue::new(&mut cue.number)
                .range(0.0..=9999.0)
                .speed(0.1));
            ui.end_row();
            
            ui.label("Label:");
            ui.text_edit_singleline(&mut cue.label);
            ui.end_row();
            
            ui.label("Fade Up:");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut cue.fade_up)
                    .range(0.0..=300.0)
                    .speed(0.1));
                ui.label("sec");
            });
            ui.end_row();
            
            ui.label("Fade Down:");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut cue.fade_down)
                    .range(0.0..=300.0)
                    .speed(0.1));
                ui.label("sec");
            });
            ui.end_row();
            
            ui.label("Channels:");
            ui.label(format!("{}", channel_count));
            ui.end_row();
        });
    
    ui.add_space(10.0);
    
    ui.label("Notes:");
    ui.text_edit_multiline(&mut cue.notes);
    
    ui.add_space(10.0);
    
    // Show channel values in cue
    if !cue.channel_values.is_empty() {
        ui.collapsing("Channel Values", |ui| {
            let mut sorted_channels: Vec<(u16, u8)> = cue.channel_values.iter()
                .map(|(&ch, &val)| (ch, val))
                .collect();
            sorted_channels.sort_by_key(|(ch, _)| *ch);
            
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    egui::Grid::new("cue_channels")
                        .num_columns(4)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            for (i, (ch, val)) in sorted_channels.iter().enumerate() {
                                ui.label(format!("Ch {}", ch));
                                ui.label(format!("{}", val));
                                if (i + 1) % 2 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });
        });
    }
    
    // Store cue number for later use (after mutable borrow ends)
    let cue_num = cue.number;
    
    // Mutable borrow ends here naturally when cue goes out of scope
    
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Go to Cue").clicked() {
            if let Some(universe) = app.universes.first() {
                app.playback.go_to_cue(&app.cue_list, cue_idx, universe);
                app.ui_state.status_message = format!("Going to cue {:.1}", cue_num);
            }
        }
        
        if ui.button("Update from Live").clicked() {
            // Update cue from current universe state
            if let Some(cue_mut) = app.cue_list.get_cue_mut(cue_idx) {
                if let Some(universe) = app.universes.first() {
                    cue_mut.channel_values.clear();
                    for ch in 1u16..=512 {
                        if let Ok(val) = universe.get_channel(ch) {
                            if val > 0 {
                                cue_mut.set_channel(ch, val);
                            }
                        }
                    }
                    app.ui_state.status_message = format!("Updated cue {:.1}", cue_num);
                }
            }
        }
        
        if ui.button("Clear Selection").clicked() {
            app.ui_state.selected_cue_index = None;
        }
    });
}
