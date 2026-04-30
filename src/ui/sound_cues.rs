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
    
    // Toolbar buttons
    ui.horizontal(|ui| {

        // Transport controls
        let go_enabled = app.audio_cue_list.next_index().is_some();
        let go_button = egui::Button::new("⏵ GO")
            .fill(if go_enabled { egui::Color32::from_rgb(50, 120, 50) } else { egui::Color32::from_rgb(30, 60, 30) });
        
        if ui.add_enabled(go_enabled, go_button).clicked() {
            if app.audio_playback.go(&mut app.audio_cue_list, &mut app.audio_player) {
                app.ui_state.status_message = "Audio GO".to_string();
                
                // Check if this audio cue triggers a lighting cue (Phase 4 cross-trigger)
                if let Some(current_idx) = app.audio_cue_list.current_index() {
                    if let Some(cue) = app.audio_cue_list.get_cue(current_idx) {
                        if let Some(light_cue_num) = cue.triggers_lighting_cue {
                            // Find and trigger the lighting cue by number
                            if let Some(light_idx) = app.cue_list.cues().iter()
                                .position(|c| (c.number - light_cue_num).abs() < 0.01) {
                                if let Some(universe) = app.universes.first() {
                                    if app.playback.go_to_cue(&app.cue_list, light_idx, universe) {
                                        app.cue_list.set_current_index(Some(light_idx));
                                        log::info!("Audio cue {:.2} triggered lighting cue {:.2}", cue.number, light_cue_num);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if ui.button("⏹ STOP").clicked() {
            app.audio_playback.stop(&mut app.audio_player);
            app.ui_state.status_message = "Audio STOP".to_string();
        }
        
        ui.separator();
        
        // Sound master control
        ui.label("Master:");
        
        // Mute toggle button
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
        
        // Draggable percentage display (replaces slider)
        let mut sound_percent = (app.ui_state.sound_master * 100.0) as i32;
        let response = ui.add(
            egui::DragValue::new(&mut sound_percent)
                .speed(1.0)
                .range(0..=100)
                .suffix("%")
        );
        
        if response.changed() {
            app.ui_state.sound_master = (sound_percent as f32) / 100.0;
            // If user manually adjusts, turn off mute
            if app.ui_state.audio_mute_active {
                app.ui_state.audio_mute_active = false;
                app.ui_state.previous_sound_master = app.ui_state.sound_master;
            }
        }
        
        ui.separator();

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
        
        if ui.button("🗑 Delete").clicked() {
            if let Some(sel_idx) = app.ui_state.selected_audio_cue_index {
                if let Some(cue) = app.audio_cue_list.get_cue(sel_idx) {
                    let cue_number = cue.number;
                    if app.audio_cue_list.remove_cue(sel_idx).is_ok() {
                        app.ui_state.selected_audio_cue_index = None;
                        app.ui_state.status_message = format!("Deleted audio cue {:.1}", cue_number);
                        // Invalidate file cache when cues change
                        app.ui_state.audio_file_cache.clear();
                    }
                }
            } else {
                app.ui_state.status_message = "Select a cue first".to_string();
            }
        }
        
        
        ui.separator();
        
        // Sound master control
        ui.label("Master:");
        
        // Mute toggle button
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
        
        // Draggable percentage display (replaces slider)
        let mut sound_percent = (app.ui_state.sound_master * 100.0) as i32;
        let response = ui.add(
            egui::DragValue::new(&mut sound_percent)
                .speed(1.0)
                .range(0..=100)
                .suffix("%")
        );
        
        
        
    });
    
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    
    // Audio cue list table
    let current_idx = app.audio_cue_list.current_index();
    let selected_idx = app.ui_state.selected_audio_cue_index;
    
    // Calculate space for footer (reserve space at bottom)
    let footer_height = 60.0; // Increased to ensure footer is visible
    let available_height = ui.available_height() - footer_height;
    
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(30.0))   // Play button
        .column(Column::exact(60.0))  // Number
        .column(Column::remainder().at_least(150.0))   // Label (editable)
        .column(Column::exact(120.0))  // File
        .column(Column::exact(70.0))   // Fade In
        .column(Column::exact(70.0))   // Fade Out
        .column(Column::exact(60.0))   // Volume
        .column(Column::exact(80.0))   // Trigger
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| { ui.strong(""); });
            header.col(|ui| { ui.strong("Cue"); });
            header.col(|ui| { ui.strong("Label"); });
            header.col(|ui| { ui.strong("File"); });
            header.col(|ui| { ui.strong("Fade In"); });
            header.col(|ui| { ui.strong("Fade Out"); });
            header.col(|ui| { ui.strong("Vol %"); });
            header.col(|ui| { ui.strong("→ Light"); });
        })
        .body(|mut body| {
            let cue_count = app.audio_cue_list.cues().len();
            let mut clicked_index: Option<usize> = None;
            let mut file_picker_index: Option<usize> = None;
            let mut go_to_cue_index: Option<usize> = None;
            
            for idx in 0..cue_count {
                body.row(24.0, |mut row| {
                    // Highlight current cue
                    let is_current = current_idx == Some(idx);
                    let is_selected = selected_idx == Some(idx);
                    
                    if is_selected {
                        row.set_selected(true);
                    }
                    
                    // Background color based on state
                    let bg_color = if is_current && is_selected {
                        egui::Color32::from_rgb(80, 120, 160)  // Current + selected
                    } else if is_current {
                        egui::Color32::from_rgb(50, 120, 50)   // Current (playing)
                    } else if is_selected {
                        egui::Color32::from_rgb(80, 80, 120)   // Selected
                    } else {
                        egui::Color32::TRANSPARENT              // Use default striping
                    };
                    
                    // Get cue for reading (immutable)
                    let cue_number = app.audio_cue_list.get_cue(idx).map(|c| c.number).unwrap_or(0.0);
                    let cue_label = app.audio_cue_list.get_cue(idx).map(|c| c.label.clone()).unwrap_or_default();
                    let cue_filename = app.audio_cue_list.get_cue(idx).map(|c| c.filename()).unwrap_or_default();
                    
                    // Check file existence using resolved path (cached to avoid expensive I/O every frame)
                    let cue_path = app.audio_cue_list.get_cue(idx).map(|c| c.audio_path.clone());
                    let resolved_path = app.audio_cue_list.get_cue(idx).map(|c| c.resolved_path());
                    let cue_exists = if let Some(path) = &resolved_path {
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
                    
                    // Collect responses from all columns to make entire row clickable
                    let mut row_responses = Vec::new();
                    
                    // Play button
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        if ui.small_button("⏵").on_hover_text("Go to this cue").clicked() {
                            go_to_cue_index = Some(idx);
                        }
                    });
                    
                    // Cue number (read-only)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let (rect, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click()
                        );
                        ui.painter().text(
                            rect.left_center() + egui::vec2(5.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{:.1}", cue_number),
                            egui::FontId::default(),
                            ui.style().visuals.text_color(),
                        );
                        row_responses.push(response);
                    });
                    
                    // Label (editable)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
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
                            clicked_index = Some(idx);
                        }
                        row_responses.push(response);
                    });
                    
                    // Filename (clickable button to change file)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        if !cue_exists {
                            ui.label(egui::RichText::new("⚠️").color(egui::Color32::RED));
                        }
                        let truncated_filename = if cue_filename.len() > 15 {
                            format!("{}...", &cue_filename[..12])
                        } else {
                            cue_filename
                        };
                        if ui.button(truncated_filename).on_hover_text("Click to change file").clicked() {
                            file_picker_index = Some(idx);
                        }
                    });
                    
                    // Fade In (editable)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let mut fade_in = cue_fade_in;
                        let response = ui.add(
                            egui::DragValue::new(&mut fade_in)
                                .speed(0.1)
                                .range(0.0..=30.0)
                                .suffix("s")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.fade_in = fade_in;
                            }
                        }
                        if response.clicked() {
                            clicked_index = Some(idx);
                        }
                        row_responses.push(response);
                    });
                    
                    // Fade Out (editable)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let mut fade_out = cue_fade_out;
                        let response = ui.add(
                            egui::DragValue::new(&mut fade_out)
                                .speed(0.1)
                                .range(0.0..=30.0)
                                .suffix("s")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.fade_out = fade_out;
                            }
                        }
                        if response.clicked() {
                            clicked_index = Some(idx);
                        }
                        row_responses.push(response);
                    });
                    
                    // Volume % (editable)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let mut volume_percent = (cue_volume * 100.0) as i32;
                        let response = ui.add(
                            egui::DragValue::new(&mut volume_percent)
                                .speed(1.0)
                                .range(0..=100)
                                .suffix("%")
                        );
                        if response.changed() {
                            if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                cue.volume = (volume_percent as f32) / 100.0;
                            }
                        }
                        if response.clicked() {
                            clicked_index = Some(idx);
                        }
                        row_responses.push(response);
                    });
                    
                    // Trigger (editable - shows lighting cue number)
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let mut has_trigger = cue_trigger.is_some();
                        let mut trigger_value = cue_trigger.unwrap_or(1.0);
                        
                        ui.horizontal(|ui| {
                            let checkbox_response = ui.checkbox(&mut has_trigger, "");
                            if checkbox_response.changed() {
                                // Check for circular dependency
                                if has_trigger && app.would_create_circular_audio_to_light(cue_number, trigger_value) {
                                    app.ui_state.status_message = 
                                        format!("⚠️ Cannot create circular trigger: Light {:.2} already triggers Audio {:.2}", 
                                                trigger_value, cue_number);
                                    has_trigger = false;
                                }
                                
                                if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                    cue.triggers_lighting_cue = if has_trigger {
                                        Some(trigger_value)
                                    } else {
                                        None
                                    };
                                }
                            }
                            if checkbox_response.clicked() {
                                clicked_index = Some(idx);
                            }
                            
                            if has_trigger {
                                let drag_response = ui.add(
                                    egui::DragValue::new(&mut trigger_value)
                                        .speed(0.1)
                                        .range(0.0..=999.0)
                                        .fixed_decimals(2)
                                );
                                if drag_response.changed() {
                                    // Check for circular dependency
                                    if app.would_create_circular_audio_to_light(cue_number, trigger_value) {
                                        app.ui_state.status_message = 
                                            format!("⚠️ Cannot create circular trigger: Light {:.2} already triggers Audio {:.2}", 
                                                    trigger_value, cue_number);
                                        // Revert to previous value
                                        if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                            cue.triggers_lighting_cue = cue_trigger;
                                        }
                                    } else {
                                        if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                                            cue.triggers_lighting_cue = Some(trigger_value);
                                        }
                                    }
                                }
                                if drag_response.clicked() {
                                    clicked_index = Some(idx);
                                }
                            }
                        });
                    });
                    
                    // Handle click to select (entire row)
                    let row_clicked = row_responses.iter().any(|r| r.clicked());
                    if row_clicked {
                        clicked_index = Some(idx);
                    }
                });
            }
            
            // Handle selection toggle (after the loop)
            if let Some(idx) = clicked_index {
                if selected_idx == Some(idx) {
                    // Toggle off if already selected
                    app.ui_state.selected_audio_cue_index = None;
                } else {
                    app.ui_state.selected_audio_cue_index = Some(idx);
                }
            }
            
            // Handle file picker (after the loop)
            if let Some(idx) = file_picker_index {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "aac", "m4a"])
                    .set_title("Select Audio File")
                    .pick_file()
                {
                    if let Some(cue) = app.audio_cue_list.get_cue_mut(idx) {
                        cue.audio_path = path;
                        app.ui_state.status_message = format!("Updated file for cue {:.1}", cue.number);
                        // Invalidate file cache when files change
                        app.ui_state.audio_file_cache.clear();
                    }
                }
            }
            
            // Handle go to cue
            if let Some(idx) = go_to_cue_index {
                if app.audio_playback.go_to_cue(&app.audio_cue_list, idx, &mut app.audio_player) {
                    app.audio_cue_list.set_current_index(Some(idx));
                    if let Some(cue) = app.audio_cue_list.get_cue(idx) {
                        app.ui_state.status_message = format!("Going to audio cue {:.1}", cue.number);
                    }
                }
            }
        });
    
    // Show playback status with solid background
    ui.separator();
    
    let state_text = match app.audio_playback.state() {
        crate::audio::AudioCueState::Stopped => "⏹ Stopped".to_string(),
        crate::audio::AudioCueState::FadingIn { progress } => 
            format!("⏵ Fading In ({:.0}%)", progress * 100.0),
        crate::audio::AudioCueState::Playing => "⏵ Playing".to_string(),
        crate::audio::AudioCueState::FadingOut { progress } => 
            format!("⏸ Fading Out ({:.0}%)", progress * 100.0),
    };
    
    egui::Frame::new()
        .fill(ui.style().visuals.extreme_bg_color)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
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
        });
}
