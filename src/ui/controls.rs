//! Controls panel - transport controls and command line

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the controls panel with GO/BACK buttons and command entry
pub fn render_controls_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // Transport controls (GO/BACK/STOP)
    ui.label(egui::RichText::new("Transport").strong());
    ui.add_space(4.0);
    
    // Large GO button
    let go_button = egui::Button::new("⏵ GO")
        .fill(egui::Color32::from_rgb(50, 120, 50))
        .min_size(egui::vec2(ui.available_width(), 50.0));
    
    if ui.add(go_button).clicked() {
        if let Some(universe) = app.universes.first() {
            app.playback.go(&mut app.cue_list, universe);
            app.ui_state.status_message = "GO".to_string();
        }
    }
    
    ui.add_space(4.0);
    
    // BACK and STOP buttons
    ui.horizontal(|ui| {
        let back_button = egui::Button::new("⏮ BACK")
            .min_size(egui::vec2(ui.available_width() / 2.0 - 5.0, 35.0));
        
        if ui.add(back_button).clicked() {
            if let Some(universe) = app.universes.first() {
                app.playback.back(&mut app.cue_list, universe);
                app.ui_state.status_message = "BACK".to_string();
            }
        }
        
        let stop_button = egui::Button::new("⏹ STOP")
            .fill(egui::Color32::from_rgb(120, 50, 50))
            .min_size(egui::vec2(ui.available_width(), 35.0));
        
        if ui.add(stop_button).clicked() {
            app.playback.stop();
            app.ui_state.status_message = "STOP".to_string();
        }
    });
    
    ui.add_space(10.0);
    ui.separator();
    
    // Playback status
    ui.label(egui::RichText::new("Status").strong());
    ui.add_space(4.0);
    
    let state_text = match app.playback.state() {
        crate::cue::CueState::Stopped => "⏹ Stopped",
        crate::cue::CueState::Fading { progress } => {
            ui.add(egui::ProgressBar::new(progress).text(format!("{:.0}%", progress * 100.0)));
            "⏵ Fading"
        }
        crate::cue::CueState::Active => "⏸ Active",
    };
    
    ui.label(state_text);
    
    // Current cue display
    if let Some(idx) = app.cue_list.current_index() {
        if let Some(cue) = app.cue_list.get_cue(idx) {
            ui.add_space(4.0);
            ui.label(format!("Current: {:.1}", cue.number));
            ui.label(egui::RichText::new(&cue.label).italics());
        }
    } else {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("No cue active").color(egui::Color32::GRAY));
    }
    
    ui.add_space(10.0);
    ui.separator();
    
    // Quick actions
    ui.label(egui::RichText::new("Quick Actions").strong());
    ui.add_space(4.0);
    
    if ui.button("Record Cue (Ctrl+R)").clicked() {
        let idx = app.record_cue();
        app.ui_state.selected_cue_index = Some(idx);
    }
    
    ui.add_space(2.0);
    
    if ui.button("Blackout").clicked() {
        if let Some(universe) = app.universes.first_mut() {
            for ch in 1..=512 {
                let _ = universe.set_channel(ch, 0);
            }
            app.ui_state.status_message = "Blackout".to_string();
        }
    }
    
    ui.add_space(10.0);
    ui.separator();
    
    // Command line entry
    ui.label(egui::RichText::new("Command Line").strong());
    ui.add_space(4.0);
    
    let command_response = ui.text_edit_singleline(&mut app.ui_state.command_input);
    
    if command_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        execute_command(app);
    }
    
    ui.add_space(2.0);
    
    if ui.button("Execute").clicked() {
        execute_command(app);
    }
    
    ui.add_space(6.0);
    
    // Command help
    ui.collapsing("Command Help", |ui| {
        ui.label("Syntax examples:");
        ui.label("• 1 @ 50     - Set channel 1 to 50%");
        ui.label("• 1-10 @ 75  - Set channels 1-10 to 75%");
        ui.label("• 1+3+5 @ 100 - Set channels 1,3,5 to 100%");
        ui.label("• GO 5       - Jump to cue 5");
        ui.label("• GO         - Go to next cue");
        ui.label("• BACK       - Go to previous cue");
        ui.label("• RECORD     - Record new cue");
    });
}

/// Execute a command from the command line
fn execute_command(app: &mut EasyCueApp) {
    let cmd = app.ui_state.command_input.trim().to_uppercase();
    
    if cmd.is_empty() {
        return;
    }
    
    app.ui_state.status_message = format!("Command: {}", cmd);
    
    // Parse and execute command
    if cmd == "GO" {
        if let Some(universe) = app.universes.first() {
            app.playback.go(&mut app.cue_list, universe);
        }
    } else if cmd == "BACK" {
        if let Some(universe) = app.universes.first() {
            app.playback.back(&mut app.cue_list, universe);
        }
    } else if cmd == "STOP" {
        app.playback.stop();
    } else if cmd == "RECORD" {
        let idx = app.record_cue();
        app.ui_state.selected_cue_index = Some(idx);
    } else if cmd.starts_with("GO ") {
        // GO <cue_number>
        if let Ok(cue_num) = cmd[3..].trim().parse::<f32>() {
            // Find cue by number
            if let Some(idx) = app.cue_list.cues().iter().position(|c| c.number == cue_num) {
                if let Some(universe) = app.universes.first() {
                    app.playback.go_to_cue(&app.cue_list, idx, universe);
                    app.ui_state.status_message = format!("Going to cue {}", cue_num);
                }
            } else {
                app.ui_state.status_message = format!("Cue {} not found", cue_num);
            }
        }
    } else if cmd.contains('@') {
        // Channel @ Level command (e.g., "1 @ 50" or "1-10 @ 75")
        parse_channel_at_command(app, &cmd);
    } else {
        app.ui_state.status_message = format!("Unknown command: {}", cmd);
    }
    
    // Clear command input
    app.ui_state.command_input.clear();
}

/// Parse and execute a "channel @ level" command
fn parse_channel_at_command(app: &mut EasyCueApp, cmd: &str) {
    let parts: Vec<&str> = cmd.split('@').collect();
    if parts.len() != 2 {
        app.ui_state.status_message = "Invalid syntax. Use: CHANNEL @ LEVEL".to_string();
        return;
    }
    
    let channels_part = parts[0].trim();
    let level_str = parts[1].trim();
    
    // Parse level (0-100)
    let level = match level_str.parse::<u8>() {
        Ok(l) => l.min(100),
        Err(_) => {
            app.ui_state.status_message = format!("Invalid level: {}", level_str);
            return;
        }
    };
    
    // Parse channel selection (supports ranges and addition)
    let mut channels = Vec::new();
    
    // Handle ranges (1-10) and addition (1+3+5)
    for part in channels_part.split('+') {
        let part = part.trim();
        
        if part.contains('-') {
            // Range (1-10)
            let range_parts: Vec<&str> = part.split('-').collect();
            if range_parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (
                    range_parts[0].trim().parse::<u16>(),
                    range_parts[1].trim().parse::<u16>(),
                ) {
                    for ch in start..=end {
                        if ch >= 1 && ch <= 512 {
                            channels.push(ch);
                        }
                    }
                }
            }
        } else {
            // Single channel
            if let Ok(ch) = part.parse::<u16>() {
                if ch >= 1 && ch <= 512 {
                    channels.push(ch);
                }
            }
        }
    }
    
    if channels.is_empty() {
        app.ui_state.status_message = "No valid channels specified".to_string();
        return;
    }
    
    // Apply to universe
    if let Some(universe) = app.universes.first_mut() {
        for ch in &channels {
            let _ = universe.set_channel(*ch, level);
        }
        
        app.ui_state.status_message = format!(
            "Set {} channel(s) to {}",
            channels.len(),
            level
        );
        
        // Select all affected channels
        app.ui_state.selected_channels.clear();
        for &ch in &channels {
            app.ui_state.selected_channels.insert(ch);
        }
    }
}
