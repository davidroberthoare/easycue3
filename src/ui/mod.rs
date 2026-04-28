//! User interface components
//!
//! egui-based UI panels and widgets.

mod channels;
mod lighting_cues;
mod sound_cues;
mod properties;
mod controls;

use egui::Context;
use crate::app::{EasyCueApp, TabKind};

pub use channels::render_channels_panel;
pub use lighting_cues::render_lighting_cues_panel;
pub use sound_cues::render_sound_cues_panel;
pub use properties::render_properties_panel;
pub use controls::render_controls_panel;

/// Render the main UI
pub fn render(ctx: &Context, app: &mut EasyCueApp) {
    // Top panel - menu bar
    render_menu_bar(ctx, app);
    
    // Bottom panel - status bar
    render_status_bar(ctx, app);
    
    // Dockable panel layout
    render_dock_area(ctx, app);
    
    // Dialogs (floating windows)
    render_dialogs(ctx, app);
}

/// Render the dockable area
fn render_dock_area(ctx: &Context, app: &mut EasyCueApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Temporarily take dock_state out to avoid borrow checker issues
        let mut dock_state = std::mem::replace(
            &mut app.dock_state,
            egui_dock::DockState::new(vec![]),
        );
        
        // Render with DockArea
        egui_dock::DockArea::new(&mut dock_state)
            .show_inside(ui, &mut MyTabViewer { app });
        
        // Put dock_state back
        app.dock_state = dock_state;
    });
}

/// Wrapper struct for TabViewer
struct MyTabViewer<'a> {
    app: &'a mut EasyCueApp,
}

impl<'a> egui_dock::TabViewer for MyTabViewer<'a> {
    type Tab = TabKind;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            TabKind::Channels => render_channels_panel(ui, self.app),
            TabKind::LightingCues => render_lighting_cues_panel(ui, self.app),
            TabKind::SoundCues => render_sound_cues_panel(ui, self.app),
            TabKind::Properties => render_properties_panel(ui, self.app),
            TabKind::Controls => render_controls_panel(ui, self.app),
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }
}

/// Render the top menu bar
fn render_menu_bar(ctx: &Context, app: &mut EasyCueApp) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            // File menu
            ui.menu_button("File", |ui| {
                if ui.button("New Show").clicked() {
                    app.cue_list.clear();
                    app.playback.stop();
                    app.show_title = "New Show".to_string();
                    app.ui_state.show_title_input = "New Show".to_string();
                    app.ui_state.selected_cue_index = None;
                    app.ui_state.selected_channels.clear();
                    app.ui_state.channel_base_levels.clear();
                    app.ui_state.group_master = 100;
                    app.ui_state.last_selected_channel = None;
                    app.ui_state.status_message = "New show created".to_string();
                    log::info!("New show created");
                    ui.close_menu();
                }
                if ui.button("Open… (Ctrl+O)").clicked() {
                    app.ui_state.show_open_dialog = true;
                    ui.close_menu();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    app.ui_state.show_save_dialog = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            
            // View menu - add panels to dock
            ui.menu_button("View", |ui| {
                ui.label(egui::RichText::new("Add Panel:").strong());
                ui.separator();
                
                if ui.button("Channels").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Channels);
                    ui.close_menu();
                }
                if ui.button("Lighting Cues").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::LightingCues);
                    ui.close_menu();
                }
                if ui.button("Sound Cues").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::SoundCues);
                    ui.close_menu();
                }
                if ui.button("Properties").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Properties);
                    ui.close_menu();
                }
                if ui.button("Controls").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Controls);
                    ui.close_menu();
                }
                
                ui.separator();
                ui.label(egui::RichText::new("Layout:").strong());
                
                if ui.button("↺ Reset Layout").clicked() {
                    app.reset_dock_layout();
                    app.ui_state.status_message = "Layout reset to default".to_string();
                    ui.close_menu();
                }
                
                ui.separator();
                ui.label(egui::RichText::new("💡 Drag tabs to rearrange").italics().small());
            });
            
            // Help menu
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    log::info!("Keyboard shortcuts: Space=GO, B=BACK, S=STOP, Ctrl+R=Record, Ctrl+S=Save, Ctrl+O=Open");
                    app.ui_state.status_message = "See console for keyboard shortcuts".to_string();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("About").clicked() {
                    log::info!("EasyCue3 - Theatrical Lighting & Media Console");
                    app.ui_state.status_message = "EasyCue3 v0.1.0".to_string();
                    ui.close_menu();
                }
            });

            // Show title in menu bar
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(&app.show_title).strong());
                ui.separator();
            });
        });
    });
}

/// Render the bottom status bar
fn render_status_bar(ctx: &Context, app: &mut EasyCueApp) {
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            
            // Playback status indicator
            let state_text = match app.playback.state() {
                crate::cue::CueState::Stopped => "⏹ Stopped",
                crate::cue::CueState::Fading { progress } => {
                    &format!("⏵ Fading {:.0}%", progress * 100.0)
                }
                crate::cue::CueState::Active => "⏸ Active",
            };
            ui.label(state_text);
            
            // Current cue
            if let Some(idx) = app.cue_list.current_index() {
                if let Some(cue) = app.cue_list.get_cue(idx) {
                    ui.separator();
                    ui.label(format!("Cue {:.1}: {}", cue.number, cue.label));
                }
            }
            
            // Status message
            if !app.ui_state.status_message.is_empty() {
                ui.separator();
                ui.label(
                    egui::RichText::new(&app.ui_state.status_message)
                        .color(egui::Color32::from_rgb(180, 180, 100)),
                );
            }
            
            // Right-aligned: DMX backend info
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!("DMX: {}", app.dmx_backend.name()))
                        .color(egui::Color32::GRAY)
                );
            });
        });
    });
}

/// Render dialogs (file open/save)
fn render_dialogs(ctx: &Context, app: &mut EasyCueApp) {
    // Open show dialog
    if app.ui_state.show_open_dialog {
        render_open_dialog(ctx, app);
    }

    // Save show dialog
    if app.ui_state.show_save_dialog {
        render_save_dialog(ctx, app);
    }
}

/// Render the open-show dialog
fn render_open_dialog(ctx: &Context, app: &mut EasyCueApp) {
    let mut open = app.ui_state.show_open_dialog;
    egui::Window::new("Open Show")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .min_width(380.0)
        .show(ctx, |ui| {
            ui.label("Show file path:");
            ui.text_edit_singleline(&mut app.ui_state.file_path_input);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    let path = std::path::PathBuf::from(&app.ui_state.file_path_input);
                    match app.load_show(&path) {
                        Ok(_) => {
                            app.ui_state.show_open_dialog = false;
                        }
                        Err(e) => {
                            app.ui_state.status_message = format!("Error: {}", e);
                            log::error!("Failed to load show: {}", e);
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.ui_state.show_open_dialog = false;
                }
            });
        });
    app.ui_state.show_open_dialog = open;
}

/// Render the save-show dialog
fn render_save_dialog(ctx: &Context, app: &mut EasyCueApp) {
    let mut open = app.ui_state.show_save_dialog;
    egui::Window::new("Save Show")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .min_width(380.0)
        .show(ctx, |ui| {
            ui.label("Show title:");
            ui.text_edit_singleline(&mut app.ui_state.show_title_input);
            ui.label("Save path:");
            ui.text_edit_singleline(&mut app.ui_state.file_path_input);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    let path = std::path::PathBuf::from(&app.ui_state.file_path_input);
                    let title = app.ui_state.show_title_input.clone();
                    match app.save_show(&path, &title) {
                        Ok(_) => {
                            app.show_title = title;
                            app.ui_state.status_message =
                                format!("Saved to {:?}", path);
                            app.ui_state.show_save_dialog = false;
                        }
                        Err(e) => {
                            app.ui_state.status_message = format!("Error: {}", e);
                            log::error!("Failed to save show: {}", e);
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    app.ui_state.show_save_dialog = false;
                }
            });
        });
    app.ui_state.show_save_dialog = open;
}

