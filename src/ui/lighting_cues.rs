//! Lighting cue list panel

use egui::Ui;
use crate::app::EasyCueApp;

/// Render the lighting cue list panel
pub fn render_lighting_cues_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    // Cue list controls
    ui.horizontal(|ui| {
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
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} cues", app.cue_list.len()));
        });
    });
    
    ui.separator();
    
    // Scrollable cue list
    let selected = app.ui_state.selected_cue_index;
    let current = app.cue_list.current_index();
    
    egui::ScrollArea::vertical()
        .id_salt("lighting_cues_scroll")
        .show(ui, |ui| {
            if app.cue_list.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(egui::RichText::new("No Lighting Cues").color(egui::Color32::GRAY));
                    ui.add_space(10.0);
                    ui.label("Press 'Record' or Ctrl+R to create your first cue");
                });
                return;
            }
            
            let cue_count = app.cue_list.len();
            let mut clicked_index: Option<usize> = None;
            
            // Calculate equal column width based on available space
            let available_width = ui.available_width();
            let col_width = (available_width - 24.0) / 4.0; // 4 columns, minus spacing
            
            // Table header
            egui::Grid::new("cue_list_header")
                .num_columns(4)
                .spacing([8.0, 4.0])
                .striped(false)
                .min_col_width(col_width)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Q").strong());
                    ui.label(egui::RichText::new("Label").strong());
                    ui.label(egui::RichText::new("Fade").strong());
                    ui.label(egui::RichText::new("Ch").strong());
                    ui.end_row();
                });
            
            ui.separator();
            
            // Cue rows
            for idx in 0..cue_count {
                if let Some(cue) = app.cue_list.get_cue(idx) {
                    let is_current = Some(idx) == current;
                    let is_selected = Some(idx) == selected;
                    
                    // Background color based on state
                    let bg_color = if is_current && is_selected {
                        egui::Color32::from_rgb(80, 120, 160)  // Current + selected
                    } else if is_current {
                        egui::Color32::from_rgb(60, 100, 60)   // Current (playing)
                    } else if is_selected {
                        egui::Color32::from_rgb(80, 80, 120)   // Selected
                    } else {
                        egui::Color32::from_gray(35)           // Default
                    };
                    
                    let response = egui::Frame::new()
                        .fill(bg_color)
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            egui::Grid::new(format!("cue_row_{}", idx))
                                .num_columns(4)
                                .spacing([8.0, 0.0])
                                .min_col_width(col_width)
                                .show(ui, |ui| {
                                    // Cue number
                                    ui.label(format!("{:.1}", cue.number));
                                    
                                    // Label
                                    let label_text = if cue.label.is_empty() {
                                        egui::RichText::new("(untitled)").italics().color(egui::Color32::GRAY)
                                    } else {
                                        egui::RichText::new(&cue.label)
                                    };
                                    ui.label(label_text);
                                    
                                    // Fade time
                                    ui.label(format!("{:.1}s", cue.fade_up));
                                    
                                    // Channel count
                                    ui.label(format!("{}", cue.channel_values.len()));
                                    
                                    ui.end_row();
                                });
                        })
                        .response;
                    
                    // Handle click to select
                    if response.interact(egui::Sense::click()).clicked() {
                        clicked_index = Some(idx);
                    }
                    
                    // Context menu (right-click)
                    response.context_menu(|ui| {
                        if ui.button("Edit").clicked() {
                            app.ui_state.selected_cue_index = Some(idx);
                            ui.close_menu();
                        }
                        if ui.button("Go To").clicked() {
                            app.playback.go_to_cue(&app.cue_list, idx);
                            ui.close_menu();
                        }
                        if ui.button("Update from Live").clicked() {
                            if let Some(cue_mut) = app.cue_list.get_cue_mut(idx) {
                                if let Some(universe) = app.universes.first() {
                                    cue_mut.channel_values.clear();
                                    for ch in 1u16..=512 {
                                        if let Ok(val) = universe.get_channel(ch) {
                                            if val > 0 {
                                                cue_mut.set_channel(ch, val);
                                            }
                                        }
                                    }
                                    app.ui_state.status_message = 
                                        format!("Updated cue {:.1}", cue_mut.number);
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
                    
                    ui.add_space(1.0);
                }
            }
            
            // Handle selection toggle
            if let Some(idx) = clicked_index {
                if selected == Some(idx) {
                    // Toggle off if already selected
                    app.ui_state.selected_cue_index = None;
                } else {
                    app.ui_state.selected_cue_index = Some(idx);
                }
            }
        });
}
