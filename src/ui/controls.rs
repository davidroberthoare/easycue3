//! Controls panel - transport controls

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
    
    // Lighting master: button + slider + percentage display
    ui.label("Lighting Master:");
    ui.horizontal(|ui| {
        // Blackout toggle button
        let blackout_text = if app.ui_state.blackout_active { "⚫" } else { "💡" };
        let blackout_color = if app.ui_state.blackout_active {
            egui::Color32::from_rgb(80, 40, 40)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };
        
        let blackout_button = egui::Button::new(blackout_text)
            .fill(blackout_color)
            .min_size(egui::vec2(30.0, 20.0));
        
        if ui.add(blackout_button).clicked() {
            if app.ui_state.blackout_active {
                // Restore previous lighting master
                app.ui_state.lighting_master = app.ui_state.previous_lighting_master;
                app.ui_state.blackout_active = false;
                app.ui_state.status_message = "Blackout OFF".to_string();
            } else {
                // Save current lighting master and set to 0
                app.ui_state.previous_lighting_master = app.ui_state.lighting_master;
                app.ui_state.lighting_master = 0.0;
                app.ui_state.blackout_active = true;
                app.ui_state.status_message = "Blackout ON".to_string();
            }
        }
        
        // Lighting slider
        let mut lighting_percent = app.ui_state.lighting_master * 100.0;
        let lighting_slider = egui::Slider::new(&mut lighting_percent, 0.0..=100.0)
            .suffix("%");
        
        if ui.add(lighting_slider).changed() {
            app.ui_state.lighting_master = lighting_percent / 100.0;
            // If user manually adjusts, turn off blackout
            if app.ui_state.blackout_active {
                app.ui_state.blackout_active = false;
                app.ui_state.previous_lighting_master = app.ui_state.lighting_master;
            }
        }
    });
    
    ui.add_space(10.0);
    
    // Sound master: button + slider + percentage display
    ui.label("Sound Master:");
    ui.horizontal(|ui| {
        // Audio mute toggle button
        let mute_text = if app.ui_state.audio_mute_active { "🔇" } else { "🔊" };
        let mute_color = if app.ui_state.audio_mute_active {
            egui::Color32::from_rgb(80, 40, 40)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };
        
        let mute_button = egui::Button::new(mute_text)
            .fill(mute_color)
            .min_size(egui::vec2(30.0, 20.0));
        
        if ui.add(mute_button).clicked() {
            if app.ui_state.audio_mute_active {
                // Restore previous sound master
                app.ui_state.sound_master = app.ui_state.previous_sound_master;
                app.ui_state.audio_mute_active = false;
                app.ui_state.status_message = "Audio unmuted".to_string();
            } else {
                // Save current sound master and set to 0
                app.ui_state.previous_sound_master = app.ui_state.sound_master;
                app.ui_state.sound_master = 0.0;
                app.ui_state.audio_mute_active = true;
                app.ui_state.status_message = "Audio muted".to_string();
            }
        }
        
        // Sound slider
        let mut sound_percent = app.ui_state.sound_master * 100.0;
        let sound_slider = egui::Slider::new(&mut sound_percent, 0.0..=100.0)
            .suffix("%");
        
        if ui.add(sound_slider).changed() {
            app.ui_state.sound_master = sound_percent / 100.0;
            // If user manually adjusts, turn off mute
            if app.ui_state.audio_mute_active {
                app.ui_state.audio_mute_active = false;
                app.ui_state.previous_sound_master = app.ui_state.sound_master;
            }
        }
    });
    
}

// Command line functionality removed - use GUI controls instead
