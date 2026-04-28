//! User interface components
//!
//! egui-based UI panels and widgets.

use egui::{Context, Ui};
use crate::app::EasyCueApp;

/// Render the main UI
pub fn render(ctx: &Context, app: &mut EasyCueApp) {
    // Top panel - menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Show").clicked() {
                    log::info!("New show requested");
                }
                if ui.button("Open...").clicked() {
                    log::info!("Open show requested");
                }
                if ui.button("Save").clicked() {
                    log::info!("Save show requested");
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut app.ui_state.show_fixture_panel, "Fixture Control");
                ui.checkbox(&mut app.ui_state.show_media_panel, "Media Panel");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    log::info!("About requested");
                }
            });
        });
    });

    // Bottom panel - transport controls
    egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
        render_transport_panel(ui, app);
    });

    // Left panel - cue list (takes priority)
    egui::SidePanel::left("cue_list")
        .default_width(300.0)
        .show(ctx, |ui| {
            render_cue_list_panel(ui, app);
        });

    // Right panel - fixture control (optional)
    if app.ui_state.show_fixture_panel {
        egui::SidePanel::right("fixture_control")
            .default_width(250.0)
            .show(ctx, |ui| {
                render_fixture_panel(ui, app);
            });
    }

    // Center panel - media/workspace
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.ui_state.show_media_panel {
            render_media_panel(ui, app);
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.heading("EasyCue3");
                ui.label("Theatrical Lighting & Media Console");
                ui.add_space(20.0);
                ui.label("Press GO to start your show ✨");
            });
        }
    });
}

/// Render the cue list panel
fn render_cue_list_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    ui.heading("Cue List");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let cues = app.cue_list.cues();
        let current_index = app.cue_list.current_index();

        if cues.is_empty() {
            ui.label("No cues");
            ui.label("Record your first cue to begin");
        } else {
            for (idx, cue) in cues.iter().enumerate() {
                let is_current = Some(idx) == current_index;
                let bg_color = if is_current {
                    egui::Color32::from_rgb(60, 60, 100)
                } else {
                    egui::Color32::from_gray(40)
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("{:.1}", cue.number));
                            ui.label(&cue.label);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(format!("{:.1}s", cue.fade_up));
                            });
                        });
                    });
                
                ui.add_space(2.0);
            }
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Record").clicked() {
            log::info!("Record cue requested");
            // TODO: Implement cue recording
        }
        if ui.button("Delete").clicked() {
            log::info!("Delete cue requested");
            // TODO: Implement cue deletion
        }
    });
}

/// Render transport controls (GO/BACK/STOP)
fn render_transport_panel(ui: &mut Ui, app: &mut EasyCueApp) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        
        // Status indicator
        let state_text = match app.playback.state() {
            crate::cue::CueState::Stopped => "⏹ Stopped",
            crate::cue::CueState::Fading { progress } => {
                &format!("⏵ Fading {:.0}%", progress * 100.0)
            }
            crate::cue::CueState::Active => "⏸ Active",
        };
        ui.label(state_text);
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            
            if ui.button("⏹ STOP").clicked() {
                app.playback.stop();
            }
            
            if ui.button("⏮ BACK").clicked() {
                app.playback.back(&mut app.cue_list);
            }
            
            // Big GO button
            let go_button = egui::Button::new("⏵ GO")
                .fill(egui::Color32::from_rgb(50, 100, 50))
                .min_size(egui::vec2(80.0, 30.0));
            
            if ui.add(go_button).clicked() {
                app.playback.go(&mut app.cue_list);
            }
        });
    });
}

/// Render fixture control panel
fn render_fixture_panel(ui: &mut Ui, _app: &mut EasyCueApp) {
    ui.heading("Fixture Control");
    ui.separator();
    
    ui.label("Fixture selection and control");
    ui.label("Coming in Phase 4");
    
    // TODO: Fixture selection UI
    // TODO: Parameter sliders
}

/// Render media panel
fn render_media_panel(ui: &mut Ui, _app: &mut EasyCueApp) {
    ui.heading("Media Panel");
    ui.separator();
    
    ui.label("Audio/Video/Image playback");
    ui.label("Coming in Phase 5");
    
    // TODO: Media file browser
    // TODO: Playback controls
}
