//! Lighting cue list panel

use egui::Ui;
use egui_extras::{TableBuilder, Column};
use crate::app::EasyCueApp;

/// Render the lighting cue list panel
pub fn render_lighting_cues_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // Top toolbar: Transport controls, master, record/delete
    ui.horizontal(|ui| {
        // Transport controls
        let go_enabled = app.cue_list.next_index().is_some();
        let go_button = egui::Button::new("⏵ GO")
            .fill(if go_enabled { egui::Color32::from_rgb(50, 120, 50) } else { egui::Color32::from_rgb(30, 60, 30) });
        
        if ui.add_enabled(go_enabled, go_button).clicked() {
            if let Some(universe) = app.universes.first() {
                if app.playback.go(&mut app.cue_list, universe) {
                    app.ui_state.status_message = "GO".to_string();
                    
                    // Check if this lighting cue triggers an audio cue (Phase 4 cross-trigger)
                    #[cfg(feature = "audio")]
                    if let Some(current_idx) = app.cue_list.current_index() {
                        if let Some(cue) = app.cue_list.get_cue(current_idx) {
                            if let Some(audio_cue_num) = cue.triggers_audio_cue {
                                // Find and trigger the audio cue by number
                                if let Some(audio_idx) = app.audio_cue_list.cues().iter()
                                    .position(|c| (c.number - audio_cue_num).abs() < 0.01) {
                                    if app.audio_playback.go_to_cue(&app.audio_cue_list, audio_idx, &mut app.audio_player) {
                                        app.audio_cue_list.set_current_index(Some(audio_idx));
                                        log::info!("Lighting cue {:.2} triggered audio cue {:.2}", cue.number, audio_cue_num);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if ui.button("⏹ STOP").clicked() {
            app.playback.stop();
            app.ui_state.status_message = "STOP".to_string();
        }
        
        ui.separator();
        
        
        
        // Record/Delete buttons
        if ui.button("➕ Record").clicked() {
            let idx = app.record_cue();
            app.ui_state.selected_cue_index = Some(idx);
        }
        
        if ui.button("🗑 Delete").clicked() {
            if let Some(sel_idx) = app.ui_state.selected_cue_index {
                if app.cue_list.remove_cue(sel_idx).is_ok() {
                    app.ui_state.selected_cue_index = None;
                    app.ui_state.status_message = "Cue deleted".to_string();
                }
            } else {
                app.ui_state.status_message = "Select a cue first".to_string();
            }
        }
        
        if ui.button("🔄 Update").clicked() {
            if let Some(sel_idx) = app.ui_state.selected_cue_index {
                if let Some(cue_mut) = app.cue_list.get_cue_mut(sel_idx) {
                    if let Some(universe) = app.universes.first() {
                        cue_mut.channel_values.clear();
                        for ch in 1u16..=512 {
                            if let Ok(val) = universe.get_channel(ch) {
                                if val > 0 {
                                    cue_mut.set_channel(ch, val);
                                }
                            }
                        }
                        app.ui_state.status_message = format!("Updated cue {:.1}", cue_mut.number);
                    }
                }
            } else {
                app.ui_state.status_message = "Select a cue first".to_string();
            }
        }


        ui.separator();


        // Lighting master control
        ui.label("Master:");
        
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
        
        // Draggable percentage display (replaces slider)
        let mut lighting_percent = (app.ui_state.lighting_master * 100.0) as i32;
        let response = ui.add(
            egui::DragValue::new(&mut lighting_percent)
                .speed(1.0)
                .range(0..=100)
                .suffix("%")
        );
        
        if response.changed() {
            app.ui_state.lighting_master = (lighting_percent as f32) / 100.0;
            // If user manually adjusts, turn off blackout
            if app.ui_state.blackout_active {
                app.ui_state.blackout_active = false;
                app.ui_state.previous_lighting_master = app.ui_state.lighting_master;
            }
        }
        


    });
    
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    
    // Reserve extra bottom space so rows never scroll under the footer,
    // while keeping the footer visually compact.
    let footer_visual_height = 48.0;
    let footer_reserved_height = 76.0;
    let available_for_table = (ui.available_height() - footer_reserved_height).max(0.0);
    
    // Scrollable cue list with resizable columns
    let selected = app.ui_state.selected_cue_index;
    let current = app.cue_list.current_index();
    
    if app.cue_list.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(egui::RichText::new("No Lighting Cues").color(egui::Color32::GRAY));
            ui.add_space(10.0);
            ui.label("Press 'Record' or Ctrl+R to create your first cue");
        });
        
    } else {
        let cue_count = app.cue_list.len();
        let mut clicked_index: Option<usize> = None;
        let mut go_to_cue_index: Option<usize> = None;
        
        // Use TableBuilder for resizable columns
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(30.0))                     // Play button
            .column(Column::initial(60.0).at_least(40.0))   // Q number
            .column(Column::remainder().at_least(100.0))     // Label (takes remaining space)
            .column(Column::initial(80.0).at_least(60.0))   // Fade
            .column(Column::initial(90.0).at_least(60.0))   // Autofollow
            .column(Column::initial(80.0).at_least(60.0))   // Sound trigger
            .min_scrolled_height(0.0)
            .max_scroll_height(available_for_table);
    
    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("");
            });
            header.col(|ui| {
                ui.strong("Q");
            });
            header.col(|ui| {
                ui.strong("Label");
            });
            header.col(|ui| {
                ui.strong("Fade");
            });
            header.col(|ui| {
                ui.strong("Autofollow");
            });
            header.col(|ui| {
                ui.strong("→ Sound");
            });
        })
        .body(|body| {
            body.rows(22.0, cue_count, |mut row| {
                let idx = row.index();
                
                // Read cue data (immutable borrow, released immediately)
                let (cue_number, cue_label, cue_fade_up, cue_autofollow, cue_triggers_audio) = 
                    if let Some(cue) = app.cue_list.get_cue(idx) {
                        (cue.number, cue.label.clone(), cue.fade_up, cue.autofollow, cue.triggers_audio_cue)
                    } else {
                        return;
                    };
                
                let is_current = Some(idx) == current;
                let is_selected = Some(idx) == selected;
                
                // Set row selection styling
                if is_selected {
                    row.set_selected(true);
                }
                
                // Background color based on state
                let bg_color = if is_current && is_selected {
                    egui::Color32::from_rgb(80, 120, 160)  // Current + selected
                } else if is_current {
                    egui::Color32::from_rgb(60, 100, 60)   // Current (playing)
                } else if is_selected {
                    egui::Color32::from_rgb(80, 80, 120)   // Selected
                } else {
                    egui::Color32::TRANSPARENT              // Use default striping
                };
                
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
                
                // Cue number
                row.col(|ui| {
                    if bg_color != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                    }
                    // Allocate entire cell space and make it interactive
                    let (rect, response) = ui.allocate_exact_size(
                        ui.available_size(),
                        egui::Sense::click()
                    );
                    // Draw text centered in the allocated space
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
                        if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                            cue.label = new_label;
                        }
                    }
                    if response.clicked() {
                        clicked_index = Some(idx);
                    }
                    
                    row_responses.push(response);
                });
                
                // Fade time (editable drag value)
                row.col(|ui| {
                    if bg_color != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                    }
                    
                    let mut fade_up = cue_fade_up;
                    let response = ui.add(
                        egui::DragValue::new(&mut fade_up)
                            .speed(0.1)
                            .range(0.0..=30.0)
                            .suffix("s")
                    );
                    
                    if response.changed() {
                        if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                            cue.fade_up = fade_up;
                        }
                    }
                    if response.clicked() {
                        clicked_index = Some(idx);
                    }
                    
                    row_responses.push(response);
                });
                
                // Autofollow time (editable - shows delay before next cue)
                row.col(|ui| {
                    if bg_color != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                    }
                    
                    let mut has_autofollow = cue_autofollow.is_some();
                    let mut autofollow_value = cue_autofollow.unwrap_or(0.0);
                    
                    let response = ui.horizontal(|ui| {
                        let checkbox_response = ui.checkbox(&mut has_autofollow, "");
                        if checkbox_response.changed() {
                            if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                cue.autofollow = if has_autofollow {
                                    Some(autofollow_value)
                                } else {
                                    None
                                };
                            }
                        }
                        if checkbox_response.clicked() {
                            clicked_index = Some(idx);
                        }
                        
                        if has_autofollow {
                            let drag_response = ui.add(
                                egui::DragValue::new(&mut autofollow_value)
                                    .speed(0.1)
                                    .range(0.0..=999.0)
                                    .suffix("s")
                            );
                            if drag_response.changed() {
                                if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                    cue.autofollow = Some(autofollow_value);
                                }
                            }
                            if drag_response.clicked() {
                                clicked_index = Some(idx);
                            }
                        }
                        checkbox_response
                    }).inner;
                    
                    row_responses.push(response);
                });
                
                // Sound trigger (editable - shows audio cue number)
                row.col(|ui| {
                    if bg_color != egui::Color32::TRANSPARENT {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                    }
                    
                    let mut has_trigger = cue_triggers_audio.is_some();
                    let mut trigger_value = cue_triggers_audio.unwrap_or(1.0);
                    
                    let response = ui.horizontal(|ui| {
                        let checkbox_response = ui.checkbox(&mut has_trigger, "");
                        if checkbox_response.changed() {
                            #[cfg(feature = "audio")]
                            {
                                // Check for circular dependency
                                if has_trigger && app.would_create_circular_light_to_audio(cue_number, trigger_value) {
                                    app.ui_state.status_message = 
                                        format!("⚠️ Cannot create circular trigger: Audio {:.2} already triggers Light {:.2}", 
                                                trigger_value, cue_number);
                                    has_trigger = false;
                                }
                            }
                            
                            if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                cue.triggers_audio_cue = if has_trigger {
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
                                #[cfg(feature = "audio")]
                                {
                                    // Check for circular dependency
                                    if app.would_create_circular_light_to_audio(cue_number, trigger_value) {
                                        app.ui_state.status_message = 
                                            format!("⚠️ Cannot create circular trigger: Audio {:.2} already triggers Light {:.2}", 
                                                    trigger_value, cue_number);
                                        // Revert to previous value (None or find previous)
                                        if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                            cue.triggers_audio_cue = cue_triggers_audio;
                                        }
                                    } else {
                                        if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                            cue.triggers_audio_cue = Some(trigger_value);
                                        }
                                    }
                                }
                                #[cfg(not(feature = "audio"))]
                                {
                                    if let Some(cue) = app.cue_list.get_cue_mut(idx) {
                                        cue.triggers_audio_cue = Some(trigger_value);
                                    }
                                }
                            }
                            if drag_response.clicked() {
                                clicked_index = Some(idx);
                            }
                        }
                        checkbox_response
                    }).inner;
                    
                    row_responses.push(response);
                });
                
                // Handle click to select (entire row) - check if any cell was clicked
                let row_clicked = row_responses.iter().any(|r| r.clicked());
                if row_clicked {
                    clicked_index = Some(idx);
                }
                
                // Context menu (right-click on entire row) - use first response
                if let Some(first_response) = row_responses.first() {
                    let combined_response = row_responses.iter().skip(1).fold(
                        first_response.clone(),
                        |acc, r| acc.union(r.clone())
                    );
                    
                    combined_response.context_menu(|ui| {
                        if ui.button("Edit").clicked() {
                            app.ui_state.selected_cue_index = Some(idx);
                            ui.close_menu();
                        }
                        if ui.button("Go To").clicked() {
                            if let Some(universe) = app.universes.first() {
                                if app.playback.go_to_cue(&app.cue_list, idx, universe) {
                                    app.cue_list.set_current_index(Some(idx));
                                    
                                    // Check if this lighting cue triggers an audio cue (Phase 4 cross-trigger)
                                    #[cfg(feature = "audio")]
                                    if let Some(cue) = app.cue_list.get_cue(idx) {
                                        if let Some(audio_cue_num) = cue.triggers_audio_cue {
                                            // Find and trigger the audio cue by number
                                            if let Some(audio_idx) = app.audio_cue_list.cues().iter()
                                                .position(|c| (c.number - audio_cue_num).abs() < 0.01) {
                                                if app.audio_playback.go_to_cue(&app.audio_cue_list, audio_idx, &mut app.audio_player) {
                                                    app.audio_cue_list.set_current_index(Some(audio_idx));
                                                    log::info!("Lighting cue {:.2} triggered audio cue {:.2}", cue.number, audio_cue_num);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            if app.cue_list.remove_cue(idx).is_ok() {
                                app.ui_state.selected_cue_index = None;
                                app.ui_state.status_message = "Cue deleted".to_string();
                            }
                            ui.close_menu();
                        }
                    });
                }
            });
        });
    
    // Handle selection toggle
    if let Some(idx) = clicked_index {
        if selected == Some(idx) {
            // Toggle off if already selected
            app.ui_state.selected_cue_index = None;
        } else {
            app.ui_state.selected_cue_index = Some(idx);
        }
    }
    
        // Handle go to cue
        if let Some(idx) = go_to_cue_index {
            if let Some(universe) = app.universes.first() {
                if app.playback.go_to_cue(&app.cue_list, idx, universe) {
                    app.cue_list.set_current_index(Some(idx));
                    if let Some(cue) = app.cue_list.get_cue(idx) {
                        app.ui_state.status_message = format!("Going to cue {:.1}", cue.number);
                        
                        // Check if this lighting cue triggers an audio cue (Phase 4 cross-trigger)
                        #[cfg(feature = "audio")]
                        if let Some(audio_cue_num) = cue.triggers_audio_cue {
                            // Find and trigger the audio cue by number
                            if let Some(audio_idx) = app.audio_cue_list.cues().iter()
                                .position(|c| (c.number - audio_cue_num).abs() < 0.01) {
                                if app.audio_playback.go_to_cue(&app.audio_cue_list, audio_idx, &mut app.audio_player) {
                                    app.audio_cue_list.set_current_index(Some(audio_idx));
                                    log::info!("Lighting cue {:.2} triggered audio cue {:.2}", cue.number, audio_cue_num);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Footer: Lighting control panel pinned to panel bottom
    let max_rect = ui.max_rect();
    let footer_rect = egui::Rect::from_min_max(
        egui::pos2(max_rect.left(), max_rect.bottom() - footer_visual_height),
        egui::pos2(max_rect.right(), max_rect.bottom()),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(footer_rect), |ui| {
        ui.separator();
        egui::Frame::new()
            .fill(ui.style().visuals.extreme_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Playback status with autofollow countdown
                    let state_text = match app.playback.state() {
                        crate::cue::CueState::Stopped => "⏹ Stopped".to_string(),
                        crate::cue::CueState::Fading { progress } => {
                            format!("⏵ Fading {:.0}%", progress * 100.0)
                        }
                        crate::cue::CueState::Active => {
                            // Check for autofollow countdown
                            if let Some(remaining) = app.playback.autofollow_remaining() {
                                format!("⏸ Active (→ {:.1}s)", remaining)
                            } else {
                                "⏸ Active".to_string()
                            }
                        }
                    };
                    ui.label(state_text);

                    // Current cue
                    if let Some(idx) = app.cue_list.current_index() {
                        if let Some(cue) = app.cue_list.get_cue(idx) {
                            ui.separator();
                            ui.label(format!("Q{:.1}", cue.number));
                        }
                    }

                    ui.separator();

                    // Command line
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center).with_main_justify(true), |ui| {
                        ui.horizontal(|ui| {
                            // Context indicator
                            let context_label = match app.ui_state.command_context {
                                crate::command::CommandContext::Lighting => "💡",
                                crate::command::CommandContext::Sound => "🔊",
                                _ => "⌨",
                            };
                            ui.label(egui::RichText::new(context_label).size(16.0));

                            // Command input
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut app.ui_state.command_input)
                                    .desired_width(ui.available_width() - 80.0)
                                    .hint_text("Click channels...")
                                    .font(egui::TextStyle::Monospace)
                            );

                            // Handle Enter key to execute command
                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                crate::ui::execute_command_line(app);
                            }

                            // Execute button
                            if ui.button("⏎").clicked() {
                                crate::ui::execute_command_line(app);
                            }

                            // Clear button
                            if ui.button("✖").clicked() {
                                app.ui_state.command_input.clear();
                                app.ui_state.status_message = "Command cleared".to_string();
                            }
                        });
                    });
                });
            });
    });
}
