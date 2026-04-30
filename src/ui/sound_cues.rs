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
    ui.add_space(2.0);
    ui.label(egui::RichText::new("Click cells to edit • Volume controlled by Sound Master").small().italics().color(egui::Color32::GRAY));
    ui.add_space(2.0);
    
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
                
                // Invalidate file cache when cues change
                app.ui_state.audio_file_cache.clear();
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
    let selected_idx = app.ui_state.selected_audio_cue_index;
    
    let available_height = ui.available_height();
    
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(60.0))  // Number
        .column(Column::remainder().at_least(150.0))   // Label (editable)
        .column(Column::exact(120.0))  // File
        .column(Column::exact(70.0))   // Fade In
        .column(Column::exact(70.0))   // Fade Out
        .column(Column::exact(60.0))   // Volume
        .column(Column::exact(80.0))   // Trigger
        .column(Column::exact(60.0))   // Actions
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| { ui.strong("Cue"); });
            header.col(|ui| { ui.strong("Label"); });
            header.col(|ui| { ui.strong("File"); });
            header.col(|ui| { ui.strong("Fade In"); });
            header.col(|ui| { ui.strong("Fade Out"); });
            header.col(|ui| { ui.strong("Vol %"); });
            header.col(|ui| { ui.strong("→ Light"); });
            header.col(|ui| { ui.strong(""); });
        })
        .body(|mut body| {
            let cue_count = app.audio_cue_list.cues().len();
            for idx in 0..cue_count {
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
                    
                    // Get cue for reading (immutable)
                    let cue_number = app.audio_cue_list.get_cue(idx).map(|c| c.number).unwrap_or(0.0);
                    let cue_label = app.audio_cue_list.get_cue(idx).map(|c| c.label.clone()).unwrap_or_default();
                    let cue_filename = app.audio_cue_list.get_cue(idx).map(|c| c.filename()).unwrap_or_default();
                    
                    // Check file existence (cached to avoid expensive I/O every frame)
                    let cue_path = app.audio_cue_list.get_cue(idx).map(|c| c.audio_path.clone());
                    let cue_exists = if let Some(path) = &cue_path {
                        // Check cache first
                        if let Some(&exists) = app.ui_state.audio_file_cache.get(path) {
                            exists
                        } else {
                            // Not in cache, check filesystem and cache result
                            let exists = path.exists();
                            app.ui_state.audio_file_cache.insert(path.clone(), exists);
                            exists
                        }
                    } else {
                        false
                    };
                    
                    let cue_fade_in = app.audio_cue_list.get_cue(idx).map(|c| c.fade_in).unwrap_or(0.0);
                    let cue_fade_out = app.audio_cue_list.get_cue(idx).map(|c| c.fade_out).unwrap_or(0.0);
                    let cue_volume = app.audio_cue_list.get_cue(idx).map(|c| c.volume).unwrap_or(1.0);
                    let cue_trigger = app.audio_cue_list.get_cue(idx).and_then(|c| c.triggers_lighting_cue);
                    
                    // Cue number (read-only)
                    row.col(|ui| {
                        ui.label(format!("{:.1}", cue_number));
                    });
                    
                    // Label (editable)
                    row.col(|ui| {
                        let mut new_label = cue_label.clone();
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut new_label)
                                .desired_width(ui.available_width())
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.label = new_label;
                            }
                        }
                        if response.clicked() {
                            app.ui_state.selected_audio_cue_index = Some(idx);
                        }
                    });
                    
                    // Filename (read-only with warning)
                    row.col(|ui| {
                        if !cue_exists {
                            ui.label(egui::RichText::new("⚠️").color(egui::Color32::RED));
                        }
                        ui.label(if cue_filename.len() > 15 {
                            format!("{}...", &cue_filename[..12])
                        } else {
                            cue_filename
                        });
                    });
                    
                    // Fade In (editable)
                    row.col(|ui| {
                        let mut fade_in = cue_fade_in;
                        let response = ui.add(
                            egui::DragValue::new(&mut fade_in)
                                .speed(0.1)
                                .clamp_range(0.0..=30.0)
                                .suffix("s")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.fade_in = fade_in;
                            }
                        }
                        if response.clicked() {
                            app.ui_state.selected_audio_cue_index = Some(idx);
                        }
                    });
                    
                    // Fade Out (editable)
                    row.col(|ui| {
                        let mut fade_out = cue_fade_out;
                        let response = ui.add(
                            egui::DragValue::new(&mut fade_out)
                                .speed(0.1)
                                .clamp_range(0.0..=30.0)
                                .suffix("s")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.fade_out = fade_out;
                            }
                        }
                        if response.clicked() {
                            app.ui_state.selected_audio_cue_index = Some(idx);
                        }
                    });
                    
                    // Volume % (editable)
                    row.col(|ui| {
                        let mut volume_percent = (cue_volume * 100.0) as i32;
                        let response = ui.add(
                            egui::DragValue::new(&mut volume_percent)
                                .speed(1.0)
                                .clamp_range(0..=100)
                                .suffix("%")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.volume = (volume_percent as f32) / 100.0;
                            }
                        }
                        if response.clicked() {
                            app.ui_state.selected_audio_cue_index = Some(idx);
                        }
                    });
                    
                    // Trigger (editable - shows lighting cue number)
                    row.col(|ui| {
                        let mut has_trigger = cue_trigger.is_some();
                        let mut trigger_value = cue_trigger.unwrap_or(1.0);
                        
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut has_trigger, "").changed() {
                                if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                    cue.triggers_lighting_cue = if has_trigger {
                                        Some(trigger_value)
                                    } else {
                                        None
                                    };
                                }
                            }
                            
                            if has_trigger {
                                if ui.add(
                                    egui::DragValue::new(&mut trigger_value)
                                        .speed(0.1)
                                        .clamp_range(0.0..=999.0)
                                ).changed() {
                                    if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                        cue.triggers_lighting_cue = Some(trigger_value);
                                    }
                                }
                            }
                        });
                    });
                    
                    // Actions
                    row.col(|ui| {
                        if ui.small_button("🗑").on_hover_text("Delete cue").clicked() {
                            // Mark for deletion
                            if let Ok(_) = app.audio_cue_list.remove_cue(idx) {
                                app.ui_state.status_message = format!("Deleted audio cue {:.1}", cue_number);
                                if app.ui_state.selected_audio_cue_index == Some(idx) {
                                    app.ui_state.selected_audio_cue_index = None;
                                }
                                // Invalidate file cache when cues change
                                app.ui_state.audio_file_cache.clear();
                            }
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
            // Show effective volume (cue volume × fade × sound master)
            #[cfg(feature = "audio")]
            {
                let effective_volume = app.audio_player.volume();
                ui.label(format!("Output: {:.0}%", effective_volume * 100.0));
            }
        });
    });
}
