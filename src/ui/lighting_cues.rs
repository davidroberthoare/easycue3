//! Lighting cue list panel

use egui::Ui;
use egui_extras::{TableBuilder, Column};
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
        return;
    }
    
    let cue_count = app.cue_list.len();
    let mut clicked_index: Option<usize> = None;
    
    // Use TableBuilder for resizable columns
    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(60.0).at_least(40.0))   // Q number
        .column(Column::remainder().at_least(100.0))     // Label (takes remaining space)
        .column(Column::initial(80.0).at_least(60.0))   // Fade
        .column(Column::initial(50.0).at_least(40.0));  // Ch count
    
    table
        .header(20.0, |mut header| {
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
                ui.strong("Ch");
            });
        })
        .body(|body| {
            body.rows(22.0, cue_count, |mut row| {
                let idx = row.index();
                
                if let Some(cue) = app.cue_list.get_cue(idx) {
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
                    let mut row_response: Option<egui::Response> = None;
                    
                    // Cue number
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let r = ui.label(format!("{:.1}", cue.number));
                        row_response = Some(r.interact(egui::Sense::click()));
                    });
                    
                    // Label
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let label_text = if cue.label.is_empty() {
                            egui::RichText::new("(untitled)").italics().color(egui::Color32::GRAY)
                        } else {
                            egui::RichText::new(&cue.label)
                        };
                        let r = ui.label(label_text);
                        if let Some(prev) = row_response.take() {
                            row_response = Some(prev.union(r.interact(egui::Sense::click())));
                        }
                    });
                    
                    // Fade time
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let r = ui.label(format!("{:.1}s", cue.fade_up));
                        if let Some(prev) = row_response.take() {
                            row_response = Some(prev.union(r.interact(egui::Sense::click())));
                        }
                    });
                    
                    // Channel count
                    row.col(|ui| {
                        if bg_color != egui::Color32::TRANSPARENT {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg_color);
                        }
                        let r = ui.label(format!("{}", cue.channel_values.len()));
                        if let Some(prev) = row_response.take() {
                            row_response = Some(prev.union(r.interact(egui::Sense::click())));
                        }
                    });
                    
                    // Handle click to select (entire row)
                    if let Some(response) = row_response {
                        if response.clicked() {
                            clicked_index = Some(idx);
                        }
                        
                        // Context menu (right-click on entire row)
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
                    }
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
}
