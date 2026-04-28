//! Lighting channels panel - manual DMX control

use egui::{Ui, Sense, Vec2, Pos2, Color32, Stroke};
use crate::app::EasyCueApp;

/// Render the lighting channels panel with per-channel sliders
pub fn render_channels_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // Channel display controls
    ui.horizontal(|ui| {
        ui.label("View:");
        if ui.button("All (1-512)").clicked() {
            // Future: Set view range
        }
        if ui.button("Active Only").clicked() {
            // Future: Filter to non-zero channels
        }
        if ui.button("Groups").clicked() {
            // Future: Show channel groups
        }
    });
    
    ui.separator();
    
    // Calculate available height for scrollable area
    // Reserve space for footer (~40px) + separator + padding
    let footer_height = 40.0;
    let max_scroll_height = ui.available_height() - footer_height - 10.0;
    
    // Scrollable area for channel display - continuous grid with grouping by 5s
    egui::ScrollArea::vertical()
        .id_salt("channels_scroll")
        .auto_shrink([false, false])
        .max_height(max_scroll_height)
        .show(ui, |ui| {
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
            
            // Layout all 512 channels in the responsive grid
            let mut channel = 1u16;
            while channel <= 512 {
                ui.horizontal(|ui| {
                    let row_end = (channel + channels_per_row as u16 - 1).min(512);
                    
                    for ch in channel..=row_end {
                        render_channel_box(ui, app, ch);
                        
                        // Add spacing between channels and extra spacing between groups
                        if ch < row_end {
                            if (ch - channel + 1) % group_size as u16 == 0 {
                                ui.add_space(group_spacing); // Extra space after each group of 5
                            } else {
                                ui.add_space(channel_spacing); // Normal spacing between channels
                            }
                        }
                    }
                });
                
                ui.add_space(2.0); // Vertical spacing between rows
                channel += channels_per_row as u16;
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
                // Update base level and master if this is the only selected channel
                if is_selected {
                    app.ui_state.channel_base_levels.insert(channel, new_val);
                    app.ui_state.group_master = new_val;
                }
            }
        }
    }
    
    // Handle click to select/deselect
    if response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        if modifiers.shift {
            // Shift+click: select range from last selected to this channel
            if let Some(last_ch) = app.ui_state.last_selected_channel {
                let start = last_ch.min(channel);
                let end = last_ch.max(channel);
                
                // Add all channels in range to selection
                for ch in start..=end {
                    app.ui_state.selected_channels.insert(ch);
                    let base_level = universe.get_channel(ch).unwrap_or(0);
                    app.ui_state.channel_base_levels.insert(ch, base_level);
                }
                
                // Update master to current max
                app.ui_state.group_master = app.ui_state.selected_channels
                    .iter()
                    .filter_map(|&ch| universe.get_channel(ch).ok())
                    .max()
                    .unwrap_or(100);
                    
                app.ui_state.last_selected_channel = Some(channel);
            } else {
                // No previous selection, treat as regular click
                app.ui_state.selected_channels.clear();
                app.ui_state.selected_channels.insert(channel);
                app.ui_state.channel_base_levels.clear();
                let base_level = universe.get_channel(channel).unwrap_or(0);
                app.ui_state.channel_base_levels.insert(channel, base_level);
                app.ui_state.group_master = base_level;
                app.ui_state.last_selected_channel = Some(channel);
            }
        } else if modifiers.command || modifiers.ctrl {
            // Ctrl/Cmd+click: toggle selection
            if is_selected {
                app.ui_state.selected_channels.remove(&channel);
                app.ui_state.channel_base_levels.remove(&channel);
            } else {
                app.ui_state.selected_channels.insert(channel);
                // Store base level for proportional scaling
                let base_level = universe.get_channel(channel).unwrap_or(0);
                app.ui_state.channel_base_levels.insert(channel, base_level);
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
            // Regular click: select only this channel
            app.ui_state.selected_channels.clear();
            app.ui_state.selected_channels.insert(channel);
            // Store base levels for all selected channels
            app.ui_state.channel_base_levels.clear();
            let base_level = universe.get_channel(channel).unwrap_or(0);
            app.ui_state.channel_base_levels.insert(channel, base_level);
            app.ui_state.group_master = base_level;
            app.ui_state.last_selected_channel = Some(channel);
        }
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
