//! User interface components
//!
//! egui-based UI panels and widgets.

mod channels;
mod cues;
mod magic_sheet;
mod properties;
mod patching;

use egui::Context;
use crate::app::{EasyCueApp, TabKind};
use egui_phosphor::regular as ph;

pub use channels::render_channels_panel;
pub use cues::render_cues_panel;
pub use magic_sheet::render_magic_sheet_panel;
pub use properties::{render_cue_properties_panel, render_instrument_properties_panel};
pub use patching::{render_patching_panel, PatchingPanelState};

/// Render the main UI
pub fn render(ctx: &Context, app: &mut EasyCueApp) {
    // Handle global keyboard shortcuts (Cmd+S, Cmd+Q, etc.)
    handle_global_shortcuts(ctx, app);
    
    // Handle keyboard input for command line (context-aware)
    handle_keyboard_input(ctx, app);
    
    // Top panel - menu bar
    render_menu_bar(ctx, app);
    
    // Bottom panel - status bar
    render_status_bar(ctx, app);
    
    // Dockable panel layout
    render_dock_area(ctx, app);
    
    // Modal dialogs (always on top)
    render_quit_confirmation(ctx, app);
    render_device_selector(ctx, app);
}

/// Handle global keyboard shortcuts
fn handle_global_shortcuts(ctx: &Context, app: &mut EasyCueApp) {
    // Detect shortcuts inside input closure, but defer actions
    let mut open_requested = false;
    let mut save_requested = false;
    let mut save_as_requested = false;
    let mut quit_requested = false;
    
    ctx.input(|i| {
        let modifiers = i.modifiers;
        
        // Cmd+O (Mac) or Ctrl+O (Linux/Windows) - Open
        if modifiers.command && i.key_pressed(egui::Key::O) {
            open_requested = true;
        }
        
        // Cmd+S (Mac) or Ctrl+S (Linux/Windows) - Save
        if modifiers.command && !modifiers.shift && i.key_pressed(egui::Key::S) {
            save_requested = true;
        }
        
        // Cmd+Shift+S - Save As
        if modifiers.command && modifiers.shift && i.key_pressed(egui::Key::S) {
            save_as_requested = true;
        }
        
        // Cmd+Q (Mac) or Ctrl+Q (Linux/Windows) - Quit
        if modifiers.command && i.key_pressed(egui::Key::Q) {
            quit_requested = true;
        }
    });
    
    // Execute actions AFTER releasing input state
    if open_requested {
        // Use native file dialog
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("EasyCue Show", &["json"])
            .set_directory("./shows")
            .pick_file()
        {
            match app.load_show(&path) {
                Ok(_) => {
                    app.ui_state.status_message = format!("Loaded: {}", app.show_title);
                    log::info!("Loaded show from {:?}", path);
                }
                Err(e) => {
                    app.ui_state.status_message = format!("Error loading: {}", e);
                    log::error!("Failed to load show: {}", e);
                }
            }
        }
    }
    
    if save_requested {
        // Save - use current file path if available, otherwise prompt
        if let Some(path) = &app.current_file_path.clone() {
            let title = app.show_title.clone();
            match app.save_show(path, &title) {
                Ok(_) => {
                    app.ui_state.status_message = format!("Saved to {:?}", path);
                    log::info!("Saved show to {:?}", path);
                }
                Err(e) => {
                    app.ui_state.status_message = format!("Error saving: {}", e);
                    log::error!("Failed to save show: {}", e);
                }
            }
        } else {
            // No current file, show Save As dialog
            let title = app.show_title.clone();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("EasyCue Show", &["json"])
                .set_directory("./shows")
                .set_file_name(&format!("{}.json", title.to_lowercase().replace(' ', "_")))
                .save_file()
            {
                match app.save_show(&path, &title) {
                    Ok(_) => {
                        app.ui_state.status_message = format!("Saved to {:?}", path);
                        log::info!("Saved show to {:?}", path);
                    }
                    Err(e) => {
                        app.ui_state.status_message = format!("Error saving: {}", e);
                        log::error!("Failed to save show: {}", e);
                    }
                }
            }
        }
    }
    
    if save_as_requested {
        // Save As - always show file dialog
        let title = app.show_title.clone();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("EasyCue Show", &["json"])
            .set_directory("./shows")
            .set_file_name(&format!("{}.json", title.to_lowercase().replace(' ', "_")))
            .save_file()
        {
            match app.save_show(&path, &title) {
                Ok(_) => {
                    app.ui_state.status_message = format!("Saved to {:?}", path);
                    log::info!("Saved show to {:?}", path);
                }
                Err(e) => {
                    app.ui_state.status_message = format!("Error saving: {}", e);
                    log::error!("Failed to save show: {}", e);
                }
            }
        }
    }
    
    if quit_requested {
        log::info!("Quit requested - showing confirmation");
        app.ui_state.show_quit_confirmation = true;
    }
}

/// Handle keyboard input for command-line operations
fn handle_keyboard_input(ctx: &Context, app: &mut EasyCueApp) {
    // Only process if we're in a command context
    if !matches!(app.ui_state.command_context, crate::command::CommandContext::Lighting | crate::command::CommandContext::Sound) {
        return;
    }
    
    // Check if text field is focused BEFORE entering ctx.input closure
    // to avoid nested borrow issues
    let is_text_focused = ctx.memory(|mem| mem.focused().is_some());
    if is_text_focused {
        return;
    }
    
    ctx.input(|i| {
        // Handle backspace
        if i.key_pressed(egui::Key::Backspace) {
            app.ui_state.command_input.pop();
        }
        
        // Handle Enter to execute
        if i.key_pressed(egui::Key::Enter) {
            execute_command_line(app);
        }
        
        // Handle character input in lighting context
        if matches!(app.ui_state.command_context, crate::command::CommandContext::Lighting) {
            for event in &i.events {
                if let egui::Event::Text(text) = event {
                    // Only accept valid command characters
                    for ch in text.chars() {
                        if ch.is_ascii_digit() || 
                           ch == 'a' || ch == '@' ||  // "at" operator
                           ch == '+' || ch == ',' ||  // addition
                           ch == '-' ||               // range or subtraction
                           ch == 't' || ch == 'h' || ch == 'r' || ch == 'u' || // "thru"
                           ch == 'f' || ch == 'l' || ch == 'o' // "full", "out"
                        {
                            app.ui_state.command_input.push(ch);
                        }
                    }
                }
            }
        }
    });
}

/// Render the dockable area
fn render_dock_area(ctx: &Context, app: &mut EasyCueApp) {
    // Custom frame with cobalt background
    let frame = egui::Frame::central_panel(&ctx.style())
        .fill(egui::Color32::from_rgb(10, 30, 55));  // Cobalt blue background
    
    egui::CentralPanel::default()
        .frame(frame)
        .show(ctx, |ui| {
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
        // Track which pane is active when mouse enters or content is clicked
        if ui.ui_contains_pointer() {
            self.app.ui_state.active_pane = Some(*tab);
            self.app.ui_state.update_command_context();
        }
        
        match tab {
            TabKind::Channels => render_channels_panel(ui, self.app),
            TabKind::Cues => render_cues_panel(ui, self.app),
            TabKind::Patching => {
                let mut patching_state = std::mem::take(&mut self.app.patching_state);
                render_patching_panel(ui, self.app, &mut patching_state);
                self.app.patching_state = patching_state;
            }
            TabKind::Properties => render_cue_properties_panel(ui, self.app),
            TabKind::InstrumentProperties => render_instrument_properties_panel(ui, self.app),
            TabKind::MagicSheet => render_magic_sheet_panel(ui, self.app),
            TabKind::Unknown => { ui.label("(unknown tab)"); }
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }
    
    // Enable horizontal scrolling, disable vertical
    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [true, false] // [horizontal, vertical]
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
                    app.current_file_path = None;
                    app.ui_state.selected_cue_id = None;
                    app.ui_state.selected_lighting_cue_id = None;
                    app.ui_state.selected_audio_cue_id = None;
                    app.ui_state.selected_channels.clear();
                    app.ui_state.channel_base_levels.clear();
                    app.ui_state.group_master = 100;
                    app.ui_state.last_selected_channel = None;
                    app.ui_state.status_message = "New show created".to_string();
                    log::info!("New show created");
                    ui.close_menu();
                }
                if ui.button("Open… (Ctrl+O)").clicked() {
                    // Use native file dialog
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("EasyCue Show", &["json"])
                        .set_directory("./shows")
                        .pick_file()
                    {
                        match app.load_show(&path) {
                            Ok(_) => {
                                app.ui_state.status_message = format!("Loaded: {}", app.show_title);
                            }
                            Err(e) => {
                                app.ui_state.status_message = format!("Error loading: {}", e);
                                log::error!("Failed to load show: {}", e);
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    // Save - use current file path if available, otherwise prompt
                    if let Some(path) = &app.current_file_path.clone() {
                        let title = app.show_title.clone();
                        match app.save_show(path, &title) {
                            Ok(_) => {
                                app.ui_state.status_message = format!("Saved to {:?}", path);
                            }
                            Err(e) => {
                                app.ui_state.status_message = format!("Error saving: {}", e);
                                log::error!("Failed to save show: {}", e);
                            }
                        }
                    } else {
                        // No current file, show Save As dialog
                        let title = app.show_title.clone();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("EasyCue Show", &["json"])
                            .set_directory("./shows")
                            .set_file_name(&format!("{}.json", title.to_lowercase().replace(' ', "_")))
                            .save_file()
                        {
                            match app.save_show(&path, &title) {
                                Ok(_) => {
                                    app.ui_state.status_message = format!("Saved to {:?}", path);
                                }
                                Err(e) => {
                                    app.ui_state.status_message = format!("Error saving: {}", e);
                                    log::error!("Failed to save show: {}", e);
                                }
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Save As… (Ctrl+Shift+S)").clicked() {
                    // Save As - always show file dialog
                    let title = app.show_title.clone();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("EasyCue Show", &["json"])
                        .set_directory("./shows")
                        .set_file_name(&format!("{}.json", title.to_lowercase().replace(' ', "_")))
                        .save_file()
                    {
                        match app.save_show(&path, &title) {
                            Ok(_) => {
                                app.ui_state.status_message = format!("Saved to {:?}", path);
                            }
                            Err(e) => {
                                app.ui_state.status_message = format!("Error saving: {}", e);
                                log::error!("Failed to save show: {}", e);
                            }
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit (Ctrl+Q)").clicked() {
                    app.ui_state.show_quit_confirmation = true;
                    ui.close_menu();
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
                if ui.button("Cues").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Cues);
                    ui.close_menu();
                }
                if ui.button("Patching").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Patching);
                    ui.close_menu();
                }
                if ui.button("Cue Properties").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::Properties);
                    ui.close_menu();
                }
                if ui.button("Instrument Properties").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::InstrumentProperties);
                    ui.close_menu();
                }
                if ui.button("Magic Sheet").clicked() {
                    app.dock_state.main_surface_mut().push_to_focused_leaf(TabKind::MagicSheet);
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
                ui.checkbox(&mut app.ui_state.show_debug_ui, format!("{} Show Debug Info (FPS)", ph::BUG));
                
                ui.separator();
                ui.label(egui::RichText::new(format!("{} Drag tabs to rearrange", ph::LIGHTBULB)).italics().small());
            });
            
            // Settings menu
            ui.menu_button("Settings", |ui| {
                if ui.button("DMX Device...").clicked() {
                    app.ui_state.show_device_selector = true;
                    ui.close_menu();
                }
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

/// Render the quit confirmation modal dialog
fn render_quit_confirmation(ctx: &Context, app: &mut EasyCueApp) {
    if !app.ui_state.show_quit_confirmation {
        return;
    }
    
    egui::Window::new("Quit EasyCue3?")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.label("Are you sure you want to quit?");
                ui.add_space(10.0);
                
                ui.horizontal(|ui| {
                    if ui.button("  Cancel  ").clicked() {
                        app.ui_state.show_quit_confirmation = false;
                    }
                    
                    ui.add_space(10.0);
                    
                    if ui.button("  Quit  ").clicked() {
                        log::info!("User confirmed quit");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.add_space(5.0);
            });
        });
}

/// Render the DMX device selector dialog
fn render_device_selector(ctx: &Context, app: &mut EasyCueApp) {
    if !app.ui_state.show_device_selector {
        return;
    }
    
    egui::Window::new("DMX Device Selection")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(10.0);
                ui.label("Select DMX output device:");
                ui.add_space(10.0);
                
                // Virtual DMX (always available)
                ui.horizontal(|ui| {
                    if ui.button("📋 Virtual DMX (Logging)").clicked() {
                        app.switch_to_virtual();
                        app.ui_state.status_message = format!("✓ Switched to {}", app.dmx_backend.name());
                        app.ui_state.show_device_selector = false;
                    }
                    ui.label("- Log output only, no hardware");
                });
                
                ui.add_space(5.0);
                
                // Enttec USB Pro (if feature enabled)
                #[cfg(feature = "usb")]
                {
                    use crate::dmx::backends::EnttecUsbProBackend;
                    
                    ui.horizontal(|ui| {
                        ui.label("🔌 Enttec DMXUSB Pro:");
                    });
                    
                    ui.indent("enttec_ports", |ui| {
                        match EnttecUsbProBackend::list_ports() {
                            Ok(ports) if !ports.is_empty() => {
                                // Filter to likely USB ports (ttyUSB* and ttyACM*)
                                let usb_ports: Vec<String> = ports.into_iter()
                                    .filter(|p| p.contains("ttyUSB") || p.contains("ttyACM"))
                                    .collect();
                                
                                if usb_ports.is_empty() {
                                    ui.label(egui::RichText::new("No USB DMX devices found").italics().color(egui::Color32::GRAY));
                                } else {
                                    // Initialize selected port if empty
                                    if app.ui_state.selected_usb_port.is_empty() && !usb_ports.is_empty() {
                                        app.ui_state.selected_usb_port = usb_ports[0].clone();
                                    }
                                    
                                    // Dropdown to select port
                                    egui::ComboBox::from_id_salt("usb_port_selector")
                                        .selected_text(&app.ui_state.selected_usb_port)
                                        .show_ui(ui, |ui| {
                                            for port in &usb_ports {
                                                ui.selectable_value(&mut app.ui_state.selected_usb_port, port.clone(), port);
                                            }
                                        });
                                    
                                    ui.add_space(5.0);
                                    
                                    // Connect button
                                    if ui.button("Connect").clicked() {
                                        let port = app.ui_state.selected_usb_port.clone();
                                        match app.switch_to_enttec(&port) {
                                            Ok(_) => {
                                                app.ui_state.status_message = format!("✓ Connected to Enttec at {}", port);
                                                log::info!("✓ Switched to Enttec DMXUSB Pro at {}", port);
                                                app.ui_state.show_device_selector = false;
                                            }
                                            Err(e) => {
                                                app.ui_state.status_message = format!("✗ Error: {}", e);
                                                log::error!("Failed to switch to Enttec: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(_) => {
                                ui.label(egui::RichText::new("No devices found").italics().color(egui::Color32::GRAY));
                            }
                            Err(e) => {
                                ui.label(egui::RichText::new(format!("Error: {}", e)).color(egui::Color32::RED));
                            }
                        }
                    });
                }
                
                #[cfg(not(feature = "usb"))]
                {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔌 Enttec USB Pro").strikethrough());
                        ui.label(egui::RichText::new("(build with --features usb)").small().italics());
                    });
                }
                
                ui.add_space(5.0);
                
                // Art-Net (disabled for now)
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🌐 Art-Net").strikethrough());
                    ui.label(egui::RichText::new("(coming soon)").small().italics());
                });
                
                ui.add_space(15.0);
                
                // Close button
                ui.vertical_centered(|ui| {
                    if ui.button("  Close  ").clicked() {
                        app.ui_state.show_device_selector = false;
                    }
                });
                
                ui.add_space(5.0);
            });
        });
}

/// Render the bottom status bar
fn render_status_bar(ctx: &Context, app: &mut EasyCueApp) {
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Left side: Status message
            if !app.ui_state.status_message.is_empty() {
                ui.label(
                    egui::RichText::new(&app.ui_state.status_message)
                        .color(egui::Color32::from_rgb(180, 180, 100)),
                );
            }
            
            // Right side: DMX backend and emergency controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Emergency controls (compact versions)
                let panic_button = egui::Button::new("🚨 PANIC")
                    .fill(egui::Color32::from_rgb(140, 40, 40))
                    .min_size(egui::vec2(80.0, 20.0));
                
                if ui.add(panic_button).clicked() {
                    // Stop all playback
                    app.playback.stop();
                    #[cfg(feature = "audio")]
                    app.audio_playback.stop_all();
                    
                    // Activate blackout
                    if !app.ui_state.blackout_active {
                        app.ui_state.previous_lighting_master = app.ui_state.lighting_master;
                        app.ui_state.lighting_master = 0.0;
                        app.ui_state.blackout_active = true;
                    }
                    
                    // Activate audio mute
                    if !app.ui_state.audio_mute_active {
                        app.ui_state.previous_sound_master = app.ui_state.sound_master;
                        app.ui_state.sound_master = 0.0;
                        app.ui_state.audio_mute_active = true;
                    }
                    
                    app.ui_state.status_message = "🚨 PANIC - All stopped".to_string();
                    log::warn!("PANIC button activated");
                }
                
                let all_stop_button = egui::Button::new(format!("{} ALL STOP", ph::STOP))
                    .fill(egui::Color32::from_rgb(120, 50, 50))
                    .min_size(egui::vec2(95.0, 20.0));
                
                if ui.add(all_stop_button).clicked() {
                    app.playback.stop();
                    #[cfg(feature = "audio")]
                    app.audio_playback.stop_all();
                    app.ui_state.status_message = "ALL STOP".to_string();
                    log::info!("All Stop activated");
                }
                
                ui.separator();
                
                ui.label(
                    egui::RichText::new(format!("DMX: {}", app.dmx_backend.name()))
                        .small()
                        .color(egui::Color32::GRAY)
                );
            });
        });
    });
}

/// Execute the current command line input
pub fn execute_command_line(app: &mut EasyCueApp) {
    let input = app.ui_state.command_input.trim().to_string();
    
    if input.is_empty() {
        return;
    }
    
    // Use context-aware parsing
    let context = app.ui_state.command_context;
    
    match context {
        crate::command::CommandContext::Lighting | crate::command::CommandContext::General => {
            match crate::command::parse_lighting_command_with_context(&input, context) {
                Ok(cmd) => {
                    crate::command::execute_command(cmd, app);
                }
                Err(e) => {
                    app.ui_state.status_message = format!("Error: {}", e);
                    log::warn!("Failed to parse command '{}': {}", input, e);
                }
            }
        }
        crate::command::CommandContext::Sound => {
            app.ui_state.status_message = "Sound commands not yet implemented".to_string();
        }
    }
    
    // Clear command input after execution
    app.ui_state.command_input.clear();
}
