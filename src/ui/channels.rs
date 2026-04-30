//! Instrument list panel - fixture-centric intensity control

use egui::{Ui, Sense, Vec2, Pos2, Color32, Stroke};
use crate::app::EasyCueApp;
use crate::fixtures::profiles::FixtureParameter;

/// Render the instrument list panel with per-fixture intensity controls
pub fn render_channels_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // View mode controls
    ui.horizontal(|ui| {
        ui.label("View:");
        
        // Toggle between fixture list and channel grid
        let mut show_unpatched = app.ui_state.show_unpatched_channels;
        if ui.checkbox(&mut show_unpatched, "Show DMX").changed() {
            app.ui_state.show_unpatched_channels = show_unpatched;
        }
        
        if !show_unpatched {
            if ui.button("Select All").clicked() {
                // Select all patched fixtures
                for patch in app.fixtures.patch_list().patches() {
                    app.ui_state.selected_fixtures.insert(patch.id);
                }
            }
            
            if !app.ui_state.selected_fixtures.is_empty() {
                if ui.button("Clear Selection").clicked() {
                    app.ui_state.selected_fixtures.clear();
                    app.ui_state.last_selected_fixture = None;
                }
            }
        }
    });
    
    ui.separator();
    
    if app.ui_state.show_unpatched_channels {
        // Show traditional channel grid for unpatched channels
        render_channel_grid(ui, app);
    } else {
        // Show instrument list for patched fixtures
        render_instrument_list(ui, app);
    }
}

/// Render the instrument list - fixture-centric view
fn render_instrument_list(ui: &mut Ui, app: &mut EasyCueApp) {
    let patches: Vec<_> = app.fixtures.patch_list().patches().to_vec();
    
    if patches.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label("No fixtures patched. Use the Patching panel to add fixtures.");
        });
        return;
    }
    
    // Calculate available height for scrollable area
    let footer_height = 40.0;
    let max_scroll_height = ui.available_height() - footer_height - 10.0;
    
    // Scrollable area for fixture list
    egui::ScrollArea::vertical()
        .id_salt("instrument_scroll")
        .auto_shrink([false, false])
        .max_height(max_scroll_height)
        .show(ui, |ui| {
            // Render each patched fixture
            for patch in &patches {
                let profile = match app.fixtures.get_profile(&patch.profile_id) {
                    Some(p) => p.clone(),
                    None => {
                        ui.label(format!("⚠ Fixture #{}: Unknown profile '{}'", patch.id, patch.profile_id));
                        continue;
                    }
                };
                
                render_fixture_row(ui, app, patch, &profile);
                ui.add_space(2.0);
            }
        });
    
    ui.separator();
    
    // Quick actions for selected fixtures
    if !app.ui_state.selected_fixtures.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} selected", app.ui_state.selected_fixtures.len()));
            ui.separator();
            
            // Quick intensity buttons for selection
            for &(label, val) in &[("0%", 0.0), ("25%", 0.25), ("50%", 0.5), ("75%", 0.75), ("FL", 1.0)] {
                if ui.button(label).clicked() {
                    set_selected_fixtures_intensity(app, val);
                }
            }
        });
    } else {
        ui.label("Click fixtures to select. Shift-click for range, Ctrl-click to toggle.");
    }
}

/// Set intensity for all selected fixtures
fn set_selected_fixtures_intensity(app: &mut EasyCueApp, intensity: f32) {
    let selected: Vec<usize> = app.ui_state.selected_fixtures.iter().copied().collect();
    
    if let Some(universe) = app.universes.first_mut() {
        for fixture_id in selected {
            let patch = match app.fixtures.patch_list().get_patch(fixture_id) {
                Some(p) => p.clone(),
                None => continue,
            };
            
            let profile = match app.fixtures.get_profile(&patch.profile_id) {
                Some(p) => p,
                None => continue,
            };
            
            // Route to appropriate intensity system
            if profile.has_intensity() {
                // iRGB: Direct intensity channel control
                if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Intensity) {
                    let channel = patch.start_address + offset;
                    let dmx_value = (intensity * 100.0).round() as u8;
                    let _ = universe.set_channel(channel, dmx_value);
                }
            } else if profile.is_rgb() {
                // RGB: Virtual intensity system
                let _ = app.virtual_intensity.set_intensity(
                    fixture_id,
                    intensity,
                    universe,
                    &patch,
                    profile,
                );
            }
        }
    }
    
    app.ui_state.status_message = format!("Set {} fixtures to {}%", 
        app.ui_state.selected_fixtures.len(), 
        (intensity * 100.0).round() as u8
    );
}

/// Render a single fixture row with intensity control
fn render_fixture_row(
    ui: &mut Ui,
    app: &mut EasyCueApp,
    patch: &crate::fixtures::Patch,
    profile: &crate::fixtures::FixtureProfile,
) {
    let fixture_id = patch.id;
    let is_selected = app.ui_state.selected_fixtures.contains(&fixture_id);
    
    // Get current intensity
    let current_intensity = if let Some(universe) = app.universes.first() {
        if profile.has_intensity() {
            // iRGB: Read from intensity channel
            if let Some(offset) = profile.get_parameter_offset(&FixtureParameter::Intensity) {
                let channel = patch.start_address + offset;
                universe.get_channel(channel).unwrap_or(0) as f32 / 100.0
            } else {
                0.0
            }
        } else if profile.is_rgb() {
            // RGB: Get virtual intensity
            app.virtual_intensity.get_intensity(fixture_id).unwrap_or_else(|| {
                // Calculate from universe if not cached
                if let Some(universe) = app.universes.first() {
                    app.virtual_intensity.calculate_intensity(fixture_id, universe, patch, profile)
                } else {
                    0.0
                }
            })
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    // Row background
    let row_height = 40.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        Sense::click_and_drag()
    );
    
    // Handle selection
    if response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        
        if modifiers.shift {
            // Shift-click: add range to selection
            if let Some(last_id) = app.ui_state.last_selected_fixture {
                let patches = app.fixtures.patch_list().patches();
                let start_idx = patches.iter().position(|p| p.id == last_id);
                let end_idx = patches.iter().position(|p| p.id == fixture_id);
                
                if let (Some(start), Some(end)) = (start_idx, end_idx) {
                    let (start, end) = if start <= end { (start, end) } else { (end, start) };
                    for patch in &patches[start..=end] {
                        app.ui_state.selected_fixtures.insert(patch.id);
                    }
                }
            }
            app.ui_state.last_selected_fixture = Some(fixture_id);
        } else if modifiers.command || modifiers.ctrl {
            // Ctrl/Cmd-click: toggle selection
            if is_selected {
                app.ui_state.selected_fixtures.remove(&fixture_id);
            } else {
                app.ui_state.selected_fixtures.insert(fixture_id);
            }
            app.ui_state.last_selected_fixture = Some(fixture_id);
        } else {
            // Regular click: replace selection
            app.ui_state.selected_fixtures.clear();
            app.ui_state.selected_fixtures.insert(fixture_id);
            app.ui_state.last_selected_fixture = Some(fixture_id);
        }
    }
    
    // Handle intensity drag
    if response.dragged() {
        let drag_delta = response.drag_delta();
        let change_y = (-drag_delta.y / 2.0) / 100.0; // Convert to intensity delta
        let change_x = (drag_delta.x / 2.0) / 100.0;
        let total_change = change_y + change_x;
        
        if total_change.abs() > 0.001 {
            let new_intensity = (current_intensity + total_change).clamp(0.0, 1.0);
            
            // Always select the dragged fixture
            if !is_selected {
                app.ui_state.selected_fixtures.clear();
                app.ui_state.selected_fixtures.insert(fixture_id);
                app.ui_state.last_selected_fixture = Some(fixture_id);
            }
            
            // Apply to all selected fixtures
            set_selected_fixtures_intensity(app, new_intensity);
        }
    }
    
    // Draw background
    let bg_color = if is_selected {
        Color32::from_rgb(50, 70, 90)
    } else if current_intensity > 0.0 {
        Color32::from_rgb(35, 35, 35)
    } else {
        Color32::from_rgb(25, 25, 25)
    };
    
    ui.painter().rect_filled(rect, 2.0, bg_color);
    
    // Draw border
    let border_color = if is_selected {
        Color32::from_rgb(100, 150, 200)
    } else {
        Color32::from_rgb(50, 50, 50)
    };
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, border_color), egui::epaint::StrokeKind::Middle);
    
    // Draw fixture info
    let text_color = if current_intensity > 0.0 {
        Color32::WHITE
    } else {
        Color32::GRAY
    };
    
    let text_pos = Pos2::new(rect.min.x + 10.0, rect.min.y + 8.0);
    
    // Line 1: [#ID] Label (Type)
    let line1 = format!("[#{}] {} ({})", 
        fixture_id, 
        patch.label, 
        profile.name
    );
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_TOP,
        line1,
        egui::FontId::proportional(14.0),
        text_color,
    );
    
    // Line 2: Intensity value
    let intensity_pct = (current_intensity * 100.0).round() as u8;
    let line2 = format!("Intensity: {}%", intensity_pct);
    let line2_pos = Pos2::new(rect.min.x + 10.0, rect.min.y + 24.0);
    
    let intensity_color = if intensity_pct == 0 {
        Color32::from_rgb(100, 100, 100)
    } else if intensity_pct == 100 {
        Color32::from_rgb(255, 100, 100)
    } else if intensity_pct >= 75 {
        Color32::from_rgb(255, 255, 100)
    } else if intensity_pct >= 50 {
        Color32::from_rgb(150, 255, 150)
    } else {
        Color32::from_rgb(150, 200, 255)
    };
    
    ui.painter().text(
        line2_pos,
        egui::Align2::LEFT_TOP,
        line2,
        egui::FontId::monospace(12.0),
        intensity_color,
    );
}

/// Render the traditional channel grid (for unpatched channels)
fn render_channel_grid(ui: &mut Ui, app: &mut EasyCueApp) {
    // Reserve space for footer (~40px) + separator + padding
    let footer_height = 40.0;
    let max_scroll_height = ui.available_height() - footer_height - 10.0;
    
    // Scrollable area for channel display - continuous grid with grouping by 5s
    // Use virtual scrolling to only render visible rows for performance
    let available_width = ui.available_width() - 10.0; // Margin for scrollbar and safety
    let box_width = 50.0;
    let group_size = 5;
    let group_spacing = 10.0; // Extra space between groups of 5
    let channel_spacing = 2.0; // Space between individual channels
    
    // Calculate width of one group of 5 channels (including internal spacing)
    // Group = 5 boxes + 4 spaces between them
    let group_width = (box_width * group_size as f32) + (channel_spacing * (group_size - 1) as f32);
    
    // Calculate how many COMPLETE groups can fit across
    // For N groups: width = N * group_width + (N-1) * group_spacing
    let mut groups_per_row = 1;
    loop {
        let width_needed = (groups_per_row + 1) as f32 * group_width 
                         + groups_per_row as f32 * group_spacing; // N+1 groups need N gaps
        if width_needed <= available_width {
            groups_per_row += 1;
            if groups_per_row >= 20 { // reasonable upper limit
                break;
            }
        } else {
            break;
        }
    }
    
    let channels_per_row = groups_per_row * group_size;
    let row_height = 55.0 + 2.0; // Box height + vertical spacing
    let total_rows = (512 + channels_per_row - 1) / channels_per_row; // Ceiling division
    
    // Use show_rows for virtual scrolling - only renders visible rows!
    egui::ScrollArea::vertical()
        .id_salt("channels_scroll")
        .auto_shrink([false, false])
        .max_height(max_scroll_height)
        .show_rows(ui, row_height, total_rows, |ui, row_range| {
            for row_idx in row_range {
                let channel_start = (row_idx * channels_per_row + 1) as u16;
                let channel_end = ((row_idx + 1) * channels_per_row).min(512) as u16;
                
                ui.horizontal(|ui| {
                    for ch in channel_start..=channel_end {
                        render_channel_box(ui, app, ch);
                        
                        // Add spacing between channels and extra spacing between groups
                        if ch < channel_end {
                            if (ch - channel_start + 1) % group_size as u16 == 0 {
                                ui.add_space(group_spacing); // Extra space after each group of 5
                            } else {
                                ui.add_space(channel_spacing); // Normal spacing between channels
                            }
                        }
                    }
                });
                
                ui.add_space(2.0); // Vertical spacing between rows
            }
        });
    
    ui.separator();
    
    // Quick actions and selection controls
    ui.horizontal_wrapped(|ui| {
        if ui.button("All @ Full").clicked() {
            if let Some(universe) = app.universes.first_mut() {
                for ch in 1..=512 {
                    let _ = universe.set_channel(ch, 100);
                }
            }
            app.ui_state.status_message = "All channels at full".to_string();
        }
        
        if ui.button("All @ 0").clicked() {
            if let Some(universe) = app.universes.first_mut() {
                for ch in 1..=512 {
                    let _ = universe.set_channel(ch, 0);
                }
            }
            app.ui_state.status_message = "All channels cleared".to_string();
        }
        
        if !app.ui_state.selected_channels.is_empty() {
            ui.separator();
            let selected_list: Vec<u16> = app.ui_state.selected_channels.iter().copied().collect();
            if selected_list.len() == 1 {
                ui.label(format!("Ch {}", selected_list[0]));
            } else {
                ui.label(format!("{} channels", selected_list.len()));
            }
            
            // Quick value buttons for selection
            ui.separator();
            for &(label, val) in &[("0%", 0), ("25%", 25), ("50%", 50), ("75%", 75), ("FL", 100)] {
                if ui.button(label).clicked() {
                    if let Some(universe) = app.universes.first_mut() {
                        // Set all selected channels to exactly this level
                        for &ch in &app.ui_state.selected_channels {
                            let _ = universe.set_channel(ch, val);
                            // Update base level to match new value
                            app.ui_state.channel_base_levels.insert(ch, val);
                        }
                        // Update master to match the new level
                        app.ui_state.group_master = val;
                    }
                }
            }
            
            ui.separator();
            if ui.button("Clear").clicked() {
                app.ui_state.selected_channels.clear();
                app.ui_state.channel_base_levels.clear();
                app.ui_state.group_master = 100;
                app.ui_state.last_selected_channel = None;
                rebuild_command_from_selection(app);
            }
        }
    });
}

/// Render a single channel box with interactive value
fn render_channel_box(
    ui: &mut Ui,
    app: &mut EasyCueApp,
    channel: u16,
) {
    // Get current value
    let universe = if let Some(u) = app.universes.first_mut() {
        u
    } else {
        return;
    };
    
    let value = universe.get_channel(channel).unwrap_or(0);
    let is_selected = app.ui_state.selected_channels.contains(&channel);
    let is_active = value > 0;
    
    // Box dimensions
    let box_size = Vec2::new(50.0, 55.0);
    let (rect, response) = ui.allocate_exact_size(box_size, Sense::click_and_drag());
    
    // Handle drag for value change
    if response.dragged() {
        let drag_delta = response.drag_delta();
        let change_y = (-drag_delta.y / 2.0) as i32;
        let change_x = (drag_delta.x / 2.0) as i32;
        let total_change = change_y + change_x;
        
        if total_change != 0 {
            if is_selected && app.ui_state.selected_channels.len() > 1 {
                // Multi-channel proportional update using O_i = M * L_i formula
                // Adjust master value M
                let new_master = (app.ui_state.group_master as i32 + total_change).clamp(0, 100) as u8;
                app.ui_state.group_master = new_master;
                
                // Find the max base level to normalize
                let max_base = app.ui_state.channel_base_levels.values().copied().max().unwrap_or(100);
                
                if max_base > 0 {
                    // Apply O_i = M * (L_i / L_max) to all selected channels
                    for &ch in &app.ui_state.selected_channels {
                        if let Some(&base_level) = app.ui_state.channel_base_levels.get(&ch) {
                            let output = ((new_master as f32) * (base_level as f32) / (max_base as f32)).round() as u8;
                            let _ = universe.set_channel(ch, output.min(100));
                        }
                    }
                } else {
                    // All base levels are 0, set all to master value
                    for &ch in &app.ui_state.selected_channels {
                        let _ = universe.set_channel(ch, new_master);
                    }
                }
            } else {
                // Single channel update
                let new_val = (value as i32 + total_change).clamp(0, 100) as u8;
                let _ = universe.set_channel(channel, new_val);
                
                // Always select the dragged channel and update base levels
                app.ui_state.selected_channels.clear();
                app.ui_state.selected_channels.insert(channel);
                app.ui_state.channel_base_levels.clear();
                app.ui_state.channel_base_levels.insert(channel, new_val);
                app.ui_state.group_master = new_val;
                app.ui_state.last_selected_channel = Some(channel);
            }
        }
    }
    
    // Handle click to select channels and update command line
    if response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        
        if modifiers.shift {
            // Shift+click: add range to selection
            if let Some(last_ch) = app.ui_state.last_selected_channel {
                let start = last_ch.min(channel);
                let end = last_ch.max(channel);
                
                // Add all channels in range to selection
                for ch in start..=end {
                    app.ui_state.selected_channels.insert(ch);
                    let base_level = universe.get_channel(ch).unwrap_or(0);
                    app.ui_state.channel_base_levels.insert(ch, base_level);
                }
                
                app.ui_state.last_selected_channel = Some(channel);
                app.ui_state.status_message = format!("Ch {}-{}", start, end);
            } else {
                // No previous selection, treat as regular click
                app.ui_state.selected_channels.clear();
                app.ui_state.selected_channels.insert(channel);
                app.ui_state.channel_base_levels.clear();
                let base_level = universe.get_channel(channel).unwrap_or(0);
                app.ui_state.channel_base_levels.insert(channel, base_level);
                app.ui_state.group_master = base_level;
                app.ui_state.last_selected_channel = Some(channel);
                app.ui_state.status_message = format!("Ch {}", channel);
            }
        } else if modifiers.command || modifiers.ctrl {
            // Ctrl/Cmd+click: toggle selection
            if is_selected {
                app.ui_state.selected_channels.remove(&channel);
                app.ui_state.channel_base_levels.remove(&channel);
                app.ui_state.status_message = format!("Ch {} removed", channel);
            } else {
                app.ui_state.selected_channels.insert(channel);
                let base_level = universe.get_channel(channel).unwrap_or(0);
                app.ui_state.channel_base_levels.insert(channel, base_level);
                app.ui_state.status_message = format!("Ch {} added", channel);
            }
            
            // Update master to current max if we have selections
            if !app.ui_state.selected_channels.is_empty() {
                app.ui_state.group_master = app.ui_state.selected_channels
                    .iter()
                    .filter_map(|&ch| universe.get_channel(ch).ok())
                    .max()
                    .unwrap_or(100);
            }
            app.ui_state.last_selected_channel = Some(channel);
        } else {
            // Regular click: replace selection with only this channel
            app.ui_state.selected_channels.clear();
            app.ui_state.selected_channels.insert(channel);
            app.ui_state.channel_base_levels.clear();
            let base_level = universe.get_channel(channel).unwrap_or(0);
            app.ui_state.channel_base_levels.insert(channel, base_level);
            app.ui_state.group_master = base_level;
            app.ui_state.last_selected_channel = Some(channel);
            app.ui_state.status_message = format!("Ch {}", channel);
        }
        
        // Rebuild command line from current selection
        rebuild_command_from_selection(app);
    }
    
    // Draw the box (simplified - just filled rect with stroke)
    let bg_color = if is_selected {
        Color32::from_rgb(60, 80, 100)
    } else if is_active {
        Color32::from_rgb(40, 40, 40)
    } else {
        Color32::from_rgb(25, 25, 25)
    };
    
    let border_color = if is_selected {
        Color32::from_rgb(100, 150, 200)
    } else if is_active {
        Color32::from_rgb(80, 80, 80)
    } else {
        Color32::from_rgb(50, 50, 50)
    };
    
    // Draw background and border
    ui.painter().rect_filled(rect, 2.0, bg_color);
    
    // Draw border lines manually
    let pts = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    ui.painter().add(egui::Shape::line(
        pts.to_vec(),
        Stroke::new(1.0, border_color),
    ));
    
    // Draw channel number at top
    let ch_text = format!("{}", channel);
    let ch_font = egui::FontId::monospace(10.0);
    let ch_galley = ui.painter().layout_no_wrap(
        ch_text,
        ch_font,
        Color32::GRAY,
    );
    let ch_pos = Pos2::new(
        rect.center().x - ch_galley.rect.width() / 2.0,
        rect.min.y + 5.0,
    );
    ui.painter().galley(ch_pos, ch_galley, Color32::GRAY);
    
    // Draw value in center (color-coded by intensity level)
    let value_text = if value == 100 {
        "FL".to_string()
    } else {
        format!("{}", value)
    };
    
    let value_color = if value == 0 {
        Color32::from_rgb(60, 60, 60)
    } else if value == 100 {
        Color32::from_rgb(255, 100, 100)
    } else if value >= 79 {
        Color32::from_rgb(255, 255, 100)
    } else if value >= 51 {
        Color32::from_rgb(150, 255, 150)
    } else if value >= 26 {
        Color32::from_rgb(100, 200, 255)
    } else {
        Color32::from_rgb(200, 150, 255)
    };
    
    let value_font = egui::FontId::proportional(18.0);
    let value_galley = ui.painter().layout_no_wrap(
        value_text,
        value_font,
        value_color,
    );
    let value_pos = Pos2::new(
        rect.center().x - value_galley.rect.width() / 2.0,
        rect.center().y - value_galley.rect.height() / 2.0 + 2.0,
    );
    ui.painter().galley(value_pos, value_galley, value_color);
}

/// Rebuild the command line from the current channel selection
fn rebuild_command_from_selection(app: &mut EasyCueApp) {
    if app.ui_state.selected_channels.is_empty() {
        app.ui_state.command_input.clear();
        return;
    }
    
    // Sort channels
    let mut channels: Vec<u16> = app.ui_state.selected_channels.iter().copied().collect();
    channels.sort_unstable();
    
    // Build compact representation with ranges
    let mut result = Vec::new();
    let mut range_start = channels[0];
    let mut range_end = channels[0];
    
    for i in 1..channels.len() {
        if channels[i] == range_end + 1 {
            // Continue the range
            range_end = channels[i];
        } else {
            // End of range, add to result
            if range_start == range_end {
                result.push(format!("{}", range_start));
            } else if range_end == range_start + 1 {
                result.push(format!("{}", range_start));
                result.push(format!("{}", range_end));
            } else {
                result.push(format!("{}thru{}", range_start, range_end));
            }
            range_start = channels[i];
            range_end = channels[i];
        }
    }
    
    // Add final range
    if range_start == range_end {
        result.push(format!("{}", range_start));
    } else if range_end == range_start + 1 {
        result.push(format!("{}", range_start));
        result.push(format!("{}", range_end));
    } else {
        result.push(format!("{}thru{}", range_start, range_end));
    }
    
    app.ui_state.command_input = result.join("+");
}
