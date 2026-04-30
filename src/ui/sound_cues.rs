//! Sound cue list panel

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the sound cue list panel
pub fn render_sound_cues_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    #[cfg(feature = "audio")]
    {
        render_audio_cues_ui(ui, app);
    }
    
    #[cfg(not(feature = "audio"))]
    {
        render_audio_disabled_message(ui);
    }
}

#[cfg(not(feature = "audio"))]
fn render_audio_disabled_message(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(30.0);
        ui.label(egui::RichText::new("🔊 Sound Cues").size(24.0));
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Audio feature not enabled").color(egui::Color32::GRAY));
        ui.add_space(20.0);
        
        ui.label("To enable audio playback:");
        ui.add_space(6.0);
        ui.label("• Rebuild with: cargo build --features audio");
        ui.label("• Or enable in Cargo.toml: default = [\"audio\"]");
    });
}

#[cfg(feature = "audio")]
fn render_audio_cues_ui(ui: &mut Ui, app: &mut EasyCueApp) {
    use egui_extras::{TableBuilder, Column};
    
    ui.heading("🔊 Sound Cues");
    ui.add_space(4.0);
    
    // Toolbar buttons
    ui.horizontal(|ui| {
        if ui.button("➕ Add Audio Cue").clicked() {
            // Open file dialog to select audio file
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "aac", "m4a"])
                .set_title("Select Audio File")
                .pick_file()
            {
                // Calculate next cue number
                let next_number = app.audio_cue_list.cues().last()
                    .map(|c| c.number.floor() + 1.0)
                    .unwrap_or(1.0);
                
                let filename = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Audio")
                    .to_string();
                
                let cue = crate::audio::AudioCue::with_label(
                    next_number,
                    path,
                    format!("Cue {:.0}: {}", next_number, filename)
                );
                
                app.audio_cue_list.add_cue(cue);
                app.ui_state.status_message = format!("Added audio cue {:.0}", next_number);
                log::info!("Added audio cue {:.0}", next_number);
            }
        }
        
        // Transport controls
        ui.separator();
        
        let go_enabled = app.audio_cue_list.next_index().is_some();
        if ui.add_enabled(go_enabled, egui::Button::new("⏵ GO")).clicked() {
            app.audio_playback.go(&mut app.audio_cue_list, &mut app.audio_player);
            app.ui_state.status_message = "Audio GO".to_string();
        }
        
        let back_enabled = app.audio_cue_list.previous_index().is_some();
        if ui.add_enabled(back_enabled, egui::Button::new("⏮ BACK")).clicked() {
            app.audio_playback.back(&mut app.audio_cue_list, &mut app.audio_player);
            app.ui_state.status_message = "Audio BACK".to_string();
        }
        
        if ui.button("⏹ STOP").clicked() {
            app.audio_playback.stop(&mut app.audio_player);
            app.ui_state.status_message = "Audio STOP".to_string();
        }
    });
    
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    
    // Audio cue list table
    let current_idx = app.audio_cue_list.current_index();
    let selected_idx = None; // TODO: Track selected audio cue in UI state
    
    let available_height = ui.available_height();
    
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(60.0))  // Number
        .column(Column::remainder())   // Label
        .column(Column::exact(120.0))  // File
        .column(Column::exact(60.0))   // Duration
        .column(Column::exact(50.0))   // Volume
        .column(Column::exact(60.0))   // Trigger
        .column(Column::exact(60.0))   // Actions
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| { ui.strong("Cue"); });
            header.col(|ui| { ui.strong("Label"); });
            header.col(|ui| { ui.strong("File"); });
            header.col(|ui| { ui.strong("Duration"); });
            header.col(|ui| { ui.strong("Vol"); });
            header.col(|ui| { ui.strong("Trigger"); });
            header.col(|ui| { ui.strong(""); });
        })
        .body(|mut body| {
            let cues = app.audio_cue_list.cues().to_vec();
            for (idx, cue) in cues.iter().enumerate() {
                body.row(24.0, |mut row| {
                    // Highlight current cue
                    let is_current = current_idx == Some(idx);
                    let is_selected = selected_idx == Some(idx);
                    
                    let row_bg = if is_current {
                        Some(egui::Color32::from_rgb(50, 120, 50))
                    } else if is_selected {
                        Some(egui::Color32::from_rgb(40, 80, 120))
                    } else {
                        None
                    };
                    
                    if let Some(_bg) = row_bg {
                        row.set_selected(true);
                    }
                    
                    // Cue number
                    row.col(|ui| {
                        ui.label(format!("{:.1}", cue.number));
                    });
                    
                    // Label
                    row.col(|ui| {
                        ui.label(&cue.label);
                    });
                    
                    // Filename
                    row.col(|ui| {
                        let filename = cue.filename();
                        
                        // Check if file exists
                        let exists = cue.audio_path.exists();
                        if !exists {
                            ui.label(egui::RichText::new("⚠️").color(egui::Color32::RED));
                        }
                        
                        ui.label(if filename.len() > 15 {
                            format!("{}...", &filename[..12])
                        } else {
                            filename
                        });
                    });
                    
                    // Duration (placeholder - rodio doesn't easily provide this before playback)
                    row.col(|ui| {
                        ui.label("--:--");
                    });
                    
                    // Volume
                    row.col(|ui| {
                        ui.label(format!("{:.0}%", cue.volume * 100.0));
                    });
                    
                    // Trigger indicator
                    row.col(|ui| {
                        if let Some(trigger_cue) = cue.triggers_lighting_cue {
                            ui.label(egui::RichText::new(format!("→🎭{:.1}", trigger_cue))
                                .color(egui::Color32::from_rgb(255, 200, 100)));
                        }
                    });
                    
                    // Actions
                    row.col(|ui| {
                        if ui.small_button("🗑").clicked() {
                            // Remove this cue (will be handled outside the table)
                        }
                    });
                });
            }
        });
    
    // Show playback status
    ui.add_space(4.0);
    ui.separator();
    
    let state_text = match app.audio_playback.state() {
        crate::audio::AudioCueState::Stopped => "⏹ Stopped".to_string(),
        crate::audio::AudioCueState::FadingIn { progress } => 
            format!("⏵ Fading In ({:.0}%)", progress * 100.0),
        crate::audio::AudioCueState::Playing => "⏵ Playing".to_string(),
        crate::audio::AudioCueState::FadingOut { progress } => 
            format!("⏸ Fading Out ({:.0}%)", progress * 100.0),
    };
    
    ui.horizontal(|ui| {
        ui.label("Status:");
        ui.label(egui::RichText::new(state_text).strong());
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Volume control
            let mut volume = app.audio_player.volume();
            ui.label("Volume:");
            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)
                .text("%")
                .custom_formatter(|v, _| format!("{:.0}", v * 100.0))
                .fixed_decimals(0)).changed()
            {
                app.audio_player.set_volume(volume);
            }
        });
    });
}
