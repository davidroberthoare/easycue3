//! Main application state and logic

use crate::cue::{Cue, CueList, PlaybackEngine};
use crate::dmx::{Universe, backends::{DmxBackend, VirtualBackend}};
#[cfg(feature = "usb")]
use crate::dmx::backends::EnttecUsbProBackend;
use crate::media::MediaManager;
use crate::fixtures::FixtureLibrary;
use crate::show::ShowFile;
use crate::command::CommandContext;
use egui_dock::DockState;
use std::collections::{HashSet, HashMap};

#[cfg(feature = "audio")]
use crate::audio::{AudioCueList, AudioPlayer, AudioPlaybackEngine};

/// Panel types for the docking system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TabKind {
    Channels,
    LightingCues,
    SoundCues,
    Patching,
    Properties,
}

impl std::fmt::Display for TabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabKind::Channels => write!(f, "Channels"),
            TabKind::LightingCues => write!(f, "Lighting Cues"),
            TabKind::SoundCues => write!(f, "Sound Cues"),
            TabKind::Patching => write!(f, "Patching"),
            TabKind::Properties => write!(f, "Properties"),
        }
    }
}

/// UI state flags and dialog state
pub struct UiState {
    // Selection state
    /// Index of the currently selected lighting cue
    pub selected_cue_index: Option<usize>,
    /// Currently selected channels for editing (supports multi-select)
    pub selected_channels: HashSet<u16>,
    /// Last selected channel for shift-range selection
    pub last_selected_channel: Option<u16>,
    /// Stored base levels for proportional scaling (L_i in formula O_i = M * L_i)
    /// Updated when selection changes
    pub channel_base_levels: HashMap<u16, u8>,
    /// Current master level for proportional group control (M in formula, 0-100)
    pub group_master: u8,
    
    /// Status message to display to the user
    pub status_message: String,
    
    // Command line input
    pub command_input: String,
    
    // Master levels and toggles
    /// Lighting master level (0.0 to 1.0, affects all lighting output)
    pub lighting_master: f32,
    /// Sound master level (0.0 to 1.0, affects all sound output)
    pub sound_master: f32,
    /// Previous lighting master level (for blackout toggle restore)
    pub previous_lighting_master: f32,
    /// Previous sound master level (for audio mute toggle restore)
    pub previous_sound_master: f32,
    /// Blackout toggle state
    pub blackout_active: bool,
    /// Audio mute toggle state
    pub audio_mute_active: bool,
    
    // Active pane tracking for context-aware commands
    pub active_pane: Option<TabKind>,
    
    // Command context (derived from active pane)
    pub command_context: CommandContext,    
    /// Selected audio cue index (for editing)
    pub selected_audio_cue_index: Option<usize>,
    
    /// Cached file existence checks (path -> exists)
    #[cfg(feature = "audio")]
    pub audio_file_cache: std::collections::HashMap<std::path::PathBuf, bool>,
    
    /// Show debug info (FPS, frame time)
    pub show_debug_ui: bool,    
    // Theme initialization flag
    pub theme_initialized: bool,
    
    // Dialog states
    pub show_quit_confirmation: bool,
    pub show_device_selector: bool,
    
    // Device selector state
    pub selected_usb_port: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_cue_index: None,
            selected_channels: HashSet::new(),
            last_selected_channel: None,
            channel_base_levels: HashMap::new(),
            group_master: 100,
            status_message: String::new(),
            command_input: String::new(),
            lighting_master: 1.0,
            sound_master: 1.0,
            previous_lighting_master: 1.0,
            previous_sound_master: 1.0,
            blackout_active: false,
            audio_mute_active: false,
            active_pane: None,
            command_context: CommandContext::General,
            theme_initialized: false,
            show_quit_confirmation: false,
            show_device_selector: false,
            selected_usb_port: String::new(),
            selected_audio_cue_index: None,
            #[cfg(feature = "audio")]
            audio_file_cache: std::collections::HashMap::new(),
            show_debug_ui: false,
        }
    }
}

impl UiState {
    /// Update command context based on active pane
    pub fn update_command_context(&mut self) {
        self.command_context = match self.active_pane {
            Some(TabKind::Channels) | Some(TabKind::LightingCues) => CommandContext::Lighting,
            Some(TabKind::SoundCues) => CommandContext::Sound,
            _ => CommandContext::General,
        };
    }
}

/// Main application state
pub struct EasyCueApp {
    /// DMX universes
    pub universes: Vec<Universe>,
    /// DMX output backend
    pub dmx_backend: Box<dyn DmxBackend>,
    /// Cue list
    pub cue_list: CueList,
    /// Playback engine
    pub playback: PlaybackEngine,
    /// Media manager
    pub media: MediaManager,
    /// Fixture library
    pub fixtures: FixtureLibrary,
    /// UI state
    pub ui_state: UiState,
    /// Patching panel state
    pub patching_state: crate::ui::PatchingPanelState,
    /// Current show title
    pub show_title: String,
    /// Current file path (None if never saved)
    pub current_file_path: Option<std::path::PathBuf>,
    /// Docking state for the panel layout
    pub dock_state: DockState<TabKind>,
    
    // Audio system (Phase 4)
    #[cfg(feature = "audio")]
    /// Audio cue list
    pub audio_cue_list: AudioCueList,
    #[cfg(feature = "audio")]
    /// Audio player
    pub audio_player: AudioPlayer,
    #[cfg(feature = "audio")]
    /// Audio playback engine
    pub audio_playback: AudioPlaybackEngine,
    #[cfg(not(feature = "audio"))]
    /// Stub audio cue list (when audio feature disabled)
    pub audio_cue_list: crate::audio::AudioCueList,
    #[cfg(not(feature = "audio"))]
    /// Stub audio player (when audio feature disabled)
    pub audio_player: crate::audio::AudioPlayer,
    #[cfg(not(feature = "audio"))]
    /// Stub audio playback engine (when audio feature disabled)
    pub audio_playback: crate::audio::AudioPlaybackEngine,
}

impl EasyCueApp {
    /// Configure the cobalt dark theme
    fn configure_cobalt_theme(ctx: &egui::Context) {
        // Start with default dark visuals as base
        let mut style = egui::Style {
            visuals: egui::Visuals::dark(),
            ..(*ctx.style()).clone()
        };
        
        // Cobalt color palette (very distinctive blue tint)
        let bg_deep = egui::Color32::from_rgb(5, 20, 40);        // Very dark blue
        let bg_main = egui::Color32::from_rgb(10, 30, 55);       // Dark blue main
        let bg_lighter = egui::Color32::from_rgb(20, 45, 75);    // Lighter blue panels
        let bg_hover = egui::Color32::from_rgb(30, 60, 100);     // Bright blue hover
        let accent_blue = egui::Color32::from_rgb(30, 150, 255); // Vivid blue accent
        let accent_cyan = egui::Color32::from_rgb(0, 220, 255);  // Bright cyan
        let text_bright = egui::Color32::from_rgb(255, 255, 255); // White text
        let text_dim = egui::Color32::from_rgb(150, 190, 220);   // Blue-tinted dim text
        let border_color = egui::Color32::from_rgb(50, 100, 150); // Blue border
        
        // Configure dark mode visuals
        style.visuals = egui::Visuals {
            dark_mode: true,
            override_text_color: Some(text_bright),
            
            // Widget visuals
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    bg_fill: bg_main,
                    weak_bg_fill: bg_main,
                    bg_stroke: egui::Stroke::new(1.0, border_color),
                    fg_stroke: egui::Stroke::new(1.0, text_dim),
                    corner_radius: egui::CornerRadius::same(4),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    bg_fill: bg_lighter,
                    weak_bg_fill: bg_lighter,
                    bg_stroke: egui::Stroke::new(1.0, border_color),
                    fg_stroke: egui::Stroke::new(1.0, text_bright),
                    corner_radius: egui::CornerRadius::same(4),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    bg_fill: bg_hover,
                    weak_bg_fill: bg_hover,
                    bg_stroke: egui::Stroke::new(1.0, accent_blue),
                    fg_stroke: egui::Stroke::new(1.5, text_bright),
                    corner_radius: egui::CornerRadius::same(4),
                    expansion: 1.0,
                },
                active: egui::style::WidgetVisuals {
                    bg_fill: accent_blue,
                    weak_bg_fill: accent_blue,
                    bg_stroke: egui::Stroke::new(1.0, accent_cyan),
                    fg_stroke: egui::Stroke::new(2.0, text_bright),
                    corner_radius: egui::CornerRadius::same(4),
                    expansion: 1.0,
                },
                open: egui::style::WidgetVisuals {
                    bg_fill: bg_hover,
                    weak_bg_fill: bg_hover,
                    bg_stroke: egui::Stroke::new(1.0, accent_blue),
                    fg_stroke: egui::Stroke::new(1.0, text_bright),
                    corner_radius: egui::CornerRadius::same(4),
                    expansion: 0.0,
                },
            },
            
            // Selection colors
            selection: egui::style::Selection {
                bg_fill: accent_blue.linear_multiply(0.4),
                stroke: egui::Stroke::new(1.0, accent_cyan),
            },
            
            // Hyperlink color
            hyperlink_color: accent_cyan,
            
            // Faint background color (for code blocks, etc.)
            faint_bg_color: bg_deep,
            
            // Extreme background color (for tooltips, etc.)
            extreme_bg_color: bg_deep,
            
            // Code background color
            code_bg_color: bg_deep,
            
            // Warning color (yellow)
            warn_fg_color: egui::Color32::from_rgb(255, 200, 0),
            
            // Error color (red)
            error_fg_color: egui::Color32::from_rgb(255, 80, 80),
            
            // Window styling
            window_fill: bg_main,
            window_stroke: egui::Stroke::new(1.0, border_color),
            window_corner_radius: egui::CornerRadius::same(6),
            window_shadow: egui::epaint::Shadow {
                offset: [4, 4],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(180),
            },
            
            // Panel fill
            panel_fill: bg_main,
            
            // Popup shadow
            popup_shadow: egui::epaint::Shadow {
                offset: [4, 4],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(180),
            },
            
            // Resize corner size
            resize_corner_size: 12.0,
            
            // Text cursor settings
            text_cursor: egui::style::TextCursorStyle {
                stroke: egui::Stroke::new(2.0, accent_cyan),
                ..Default::default()
            },
            
            // Clip rect margin
            clip_rect_margin: 3.0,
            
            // Button frame
            button_frame: true,
            
            // Collapsing header frame
            collapsing_header_frame: false,
            
            // Indent has background
            indent_has_left_vline: true,
            
            // Striped
            striped: true,
            
            // Slider trailing fill
            slider_trailing_fill: true,
            
            // Handle shape
            handle_shape: egui::style::HandleShape::Circle,
            
            // Menu corner radius
            menu_corner_radius: egui::CornerRadius::same(4),
            
            ..Default::default()
        };
        
        // Apply larger spacing for theatrical console feel
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 20.0;
        style.spacing.slider_width = 150.0;
        
        // Apply the style
        ctx.set_style(style);
        
        log::info!("Applied cobalt dark theme");
    }
    
    /// Create a new application instance
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply cobalt theme
        Self::configure_cobalt_theme(&cc.egui_ctx);
        
        // Initialize with 2 universes (configurable later)
        let universes = vec![
            Universe::new(0),
            Universe::new(1),
        ];

        // Try to auto-detect Enttec USB device, fall back to Virtual
        let dmx_backend: Box<dyn DmxBackend> = {
            #[cfg(feature = "usb")]
            {
                match EnttecUsbProBackend::list_ports() {
                    Ok(ports) if !ports.is_empty() => {
                        // Try to connect to first available port
                        match EnttecUsbProBackend::new(&ports[0]) {
                            Ok(backend) => {
                                log::info!("✓ Connected to Enttec DMXUSB Pro at {}", ports[0]);
                                Box::new(backend)
                            }
                            Err(e) => {
                                log::warn!("Failed to connect to Enttec device: {}", e);
                                log::info!("Falling back to Virtual DMX");
                                Box::new(VirtualBackend::new(true))
                            }
                        }
                    }
                    _ => {
                        log::info!("No Enttec USB device found, using Virtual DMX");
                        Box::new(VirtualBackend::new(true))
                    }
                }
            }
            #[cfg(not(feature = "usb"))]
            {
                log::info!("USB support not enabled, using Virtual DMX");
                Box::new(VirtualBackend::new(true))
            }
        };

        log::info!("EasyCue3 application initialized");
        log::info!("DMX Backend: {}", dmx_backend.name());

        // Load dock layout from persistence or create default
        let dock_state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, "dock_state").unwrap_or_else(|| Self::create_default_dock_layout())
        } else {
            Self::create_default_dock_layout()
        };

        let mut app = Self {
            universes,
            dmx_backend,
            cue_list: CueList::new(),
            playback: PlaybackEngine::new(),
            media: MediaManager::new(),
            fixtures: FixtureLibrary::new(),
            ui_state: UiState::default(),
            patching_state: crate::ui::PatchingPanelState::default(),
            show_title: "Example Show".to_string(),
            current_file_path: None,
            dock_state,
            
            // Initialize audio system (Phase 4)
            #[cfg(feature = "audio")]
            audio_cue_list: AudioCueList::new(),
            #[cfg(feature = "audio")]
            audio_player: AudioPlayer::new().unwrap_or_else(|e| {
                log::error!("Failed to initialize audio player: {}", e);
                log::warn!("Audio playback will be disabled");
                AudioPlayer::new().unwrap()  // This will panic if it fails twice
            }),
            #[cfg(feature = "audio")]
            audio_playback: AudioPlaybackEngine::new(),
            #[cfg(not(feature = "audio"))]
            audio_cue_list: crate::audio::AudioCueList::new(),
            #[cfg(not(feature = "audio"))]
            audio_player: crate::audio::AudioPlayer::new().unwrap(),
            #[cfg(not(feature = "audio"))]
            audio_playback: crate::audio::AudioPlaybackEngine::new(),
        };

        // Try to load example show on startup
        let example_path = std::path::Path::new("shows/example_show.json");
        if example_path.exists() {
            match app.load_show(example_path) {
                Ok(_) => log::info!("Loaded example show on startup"),
                Err(e) => log::warn!("Could not load example show: {}", e),
            }
        }

        app
    }

    /// Create the default dock layout
    fn create_default_dock_layout() -> DockState<TabKind> {
        let mut dock_state = DockState::new(vec![TabKind::Channels]);
        let tree = dock_state.main_surface_mut();
        
        // Top row: Channels (left) and Properties (right)
        let [_channels, _properties] = tree.split_right(egui_dock::NodeIndex::root(), 0.7, vec![TabKind::Properties]);
        
        // Split entire layout horizontally to create bottom row
        let [_top_row, bottom_row] = tree.split_below(egui_dock::NodeIndex::root(), 0.5, vec![TabKind::LightingCues]);
        
        // Bottom row: Lighting Cues, Patching, Sound Cues
        let [_lighting, right_tabs] = tree.split_right(bottom_row, 0.3, vec![TabKind::Patching]);
        let [_patching, _sound] = tree.split_right(right_tabs, 0.5, vec![TabKind::SoundCues]);
        
        dock_state
    }

    /// Reset the dock layout to the default configuration
    pub fn reset_dock_layout(&mut self) {
        self.dock_state = Self::create_default_dock_layout();
        log::info!("Reset UI layout to default");
    }

    /// Load a show file and populate the cue list
    pub fn load_show(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let show = ShowFile::load(path)?;
        self.cue_list.clear();
        for cue in show.cues {
            self.cue_list.add_cue(cue);
        }
        
        // Load audio cues (Phase 4)
        #[cfg(feature = "audio")]
        {
            self.audio_cue_list.load_cues(show.audio_cues);
            log::info!("Loaded {} audio cues", self.audio_cue_list.len());
        }
        
        // Load patch into fixture library
        *self.fixtures.patch_list_mut() = crate::fixtures::PatchList::new();
        for patch in show.patch {
            // Add patch directly (bypassing validation since it's from a saved show)
            if self.fixtures.get_profile(&patch.profile_id).is_some() {
                match self.fixtures.add_patch(
                    patch.label.clone(),
                    patch.profile_id.clone(),
                    patch.start_address,
                ) {
                    Ok(_) => {
                        log::debug!(
                            "Loaded patch: {} ({}) at address {}",
                            patch.label,
                            patch.profile_id,
                            patch.start_address
                        );
                    }
                    Err(e) => {
                        log::warn!("Failed to load patch '{}': {}", patch.label, e);
                    }
                }
            } else {
                log::warn!(
                    "Skipping patch '{}': profile '{}' not found",
                    patch.label,
                    patch.profile_id
                );
            }
        }
        
        self.show_title = show.title.clone();
        self.current_file_path = Some(path.to_path_buf());
        self.ui_state.selected_cue_index = None;
        self.ui_state.status_message = format!("Loaded show from {:?}", path);
        log::info!(
            "Loaded show: {} ({} cues, {} fixtures)",
            self.show_title,
            self.cue_list.len(),
            self.fixtures.patch_list().len()
        );
        Ok(())
    }

    /// Save the current cue list to a show file
    pub fn save_show(&mut self, path: &std::path::Path, title: &str) -> anyhow::Result<()> {
        let mut show = ShowFile::new(title);
        show.cues = self.cue_list.cues().to_vec();
        show.patch = self.fixtures.patch_list().patches().to_vec();
        
        // Save audio cues (Phase 4)
        #[cfg(feature = "audio")]
        {
            show.audio_cues = self.audio_cue_list.cues().to_vec();
        }
        
        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        show.save(path)?;
        self.current_file_path = Some(path.to_path_buf());
        log::info!(
            "Saved show: {} ({} cues, {} fixtures)",
            title,
            show.cues.len(),
            show.patch.len()
        );
        Ok(())
    }

    /// Apply lighting master and blackout to universe before output
    /// Returns a new Universe with the master levels applied
    pub fn apply_masters(&self, universe: &Universe) -> Universe {
        let mut output = universe.clone();
        
        // If blackout is active, zero all channels
        if self.ui_state.blackout_active {
            output.clear();
            return output;
        }
        
        // Apply lighting master (0.0 to 1.0) to all channels
        if self.ui_state.lighting_master < 1.0 {
            for ch in 1..=512 {
                if let Ok(value) = universe.get_channel(ch) {
                    if value > 0 {
                        let scaled = (value as f32 * self.ui_state.lighting_master).round() as u8;
                        let _ = output.set_channel(ch, scaled);
                    }
                }
            }
        }
        
        output
    }

    /// Switch to Virtual DMX backend
    pub fn switch_to_virtual(&mut self) {
        self.dmx_backend = Box::new(VirtualBackend::new(true));
        log::info!("Switched to Virtual DMX backend");
    }

    /// Switch to Enttec USB Pro backend
    #[cfg(feature = "usb")]
    pub fn switch_to_enttec(&mut self, port: &str) -> anyhow::Result<()> {
        let backend = EnttecUsbProBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        log::info!("Switched to Enttec USB Pro at {}", port);
        Ok(())
    }

    /// Record a new cue from the current universe state
    ///
    /// Creates a new cue with the next sequential cue number and captures
    /// all non-zero channel values from the first universe.
    pub fn record_cue(&mut self) -> usize {
        // Calculate the next cue number (increment by 1 from last cue)
        let next_number = self.cue_list.cues().last()
            .map(|c| c.number.floor() + 1.0)
            .unwrap_or(1.0);

        let mut cue = Cue::new(next_number);
        cue.label = format!("Cue {:.0}", next_number);

        // Capture current universe state
        if let Some(universe) = self.universes.first() {
            for ch in 1u16..=512 {
                if let Ok(val) = universe.get_channel(ch) {
                    if val > 0 {
                        cue.set_channel(ch, val);
                    }
                }
            }
        }

        let insert_idx = self.cue_list.len();
        self.cue_list.add_cue(cue);
        self.ui_state.status_message = format!("Recorded cue {:.0}", next_number);
        log::info!("Recorded cue {:.0} with {} channels",
            next_number,
            self.cue_list.get_cue(insert_idx).map(|c| c.channel_values.len()).unwrap_or(0));
        insert_idx
    }
}

impl eframe::App for EasyCueApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ensure theme is applied (reapply if not marked as initialized)
        if !self.ui_state.theme_initialized {
            Self::configure_cobalt_theme(ctx);
            self.ui_state.theme_initialized = true;
            log::info!("Theme reapplied in update()");
        }
        
        // Handle keyboard shortcuts (checked before UI to avoid consuming events)
        let (go, back, stop, record) = ctx.input(|i| (
            i.key_pressed(egui::Key::Space) && !i.modifiers.any(),
            i.key_pressed(egui::Key::B)     && !i.modifiers.any(),
            i.key_pressed(egui::Key::S)     && !i.modifiers.any(),
            i.key_pressed(egui::Key::R)     && i.modifiers.ctrl,
        ));

        if stop { self.playback.stop(); }
        if record {
            let idx = self.record_cue();
            self.ui_state.selected_cue_index = Some(idx);
        }

        // Update playback engine and apply to first universe
        // Handle go/back within universe access since they need current state
        if let Some(universe) = self.universes.first_mut() {
            // Handle go/back commands with access to current universe state
            if go  { self.playback.go(&mut self.cue_list, universe); }
            if back { self.playback.back(&mut self.cue_list, universe); }
            
            self.playback.update(universe);
        }
        
        // Update audio playback engine (Phase 4)
        #[cfg(feature = "audio")]
        {
            // Update fades and get base volume (cue volume with fades applied)
            let base_volume = self.audio_playback.update(&mut self.audio_player);
            
            // Apply sound master on top of base volume (allows real-time master adjustment)
            let effective_volume = base_volume * self.ui_state.sound_master;
            self.audio_player.set_volume(effective_volume);
            
            // Check for cross-triggers from audio to lighting
            if let Some(lighting_cue_number) = self.audio_playback.take_pending_lighting_trigger() {
                // Find the lighting cue with this number and trigger it
                for (idx, cue) in self.cue_list.cues().iter().enumerate() {
                    if (cue.number - lighting_cue_number).abs() < 0.01 {
                        if let Some(universe) = self.universes.first() {
                            self.playback.go_to_cue(&self.cue_list, idx, universe);
                            self.cue_list.set_current_index(Some(idx));
                            log::info!("Audio cross-trigger: lighting cue {}", lighting_cue_number);
                        }
                        break;
                    }
                }
            }
        }

        // Apply master level and blackout before sending (separate borrow)
        let dmx_send_start = std::time::Instant::now();
        if let Some(universe) = self.universes.first() {
            let output_universe = self.apply_masters(universe);
            
            // Send DMX output
            if let Err(e) = self.dmx_backend.send_universe(&output_universe) {
                log::error!("DMX output error: {}", e);
            }
        }
        let dmx_send_time = dmx_send_start.elapsed();
        
        // Render UI
        let ui_render_start = std::time::Instant::now();
        crate::ui::render(ctx, self);
        let ui_render_time = ui_render_start.elapsed();
        
        // Debug UI overlay (FPS counter, frame time)
        if self.ui_state.show_debug_ui {
            egui::Window::new("🐛 Debug Info")
                .default_pos([10.0, 10.0])
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.label(format!("FPS: {:.1}", ctx.input(|i| 1.0 / i.stable_dt)));
                    ui.label(format!("Frame time: {:.2}ms", ctx.input(|i| i.stable_dt * 1000.0)));
                    ui.separator();
                    
                    // Performance breakdown
                    ui.label(egui::RichText::new("Performance:").strong());
                    ui.label(format!("  DMX send: {:.2}ms", dmx_send_time.as_secs_f64() * 1000.0));
                    ui.label(format!("  UI render: {:.2}ms", ui_render_time.as_secs_f64() * 1000.0));
                    
                    ui.separator();
                    
                    #[cfg(feature = "audio")]
                    {
                        ui.label(format!("Audio cues: {}", self.audio_cue_list.cues().len()));
                        ui.label(format!("File cache: {} entries", self.ui_state.audio_file_cache.len()));
                        ui.label(format!("Audio playing: {}", self.audio_playback.is_playing()));
                    }
                    
                    ui.separator();
                    ui.label(format!("Lighting cues: {}", self.cue_list.len()));
                    ui.label(format!("Lighting playing: {}", self.playback.is_playing()));
                    
                    ui.separator();
                    if ui.button("Clear file cache").clicked() {
                        #[cfg(feature = "audio")]
                        self.ui_state.audio_file_cache.clear();
                    }
                });
        }

        // Request continuous repaint for smooth fades (but only when needed)
        if self.playback.is_playing() {
            // Request repaint after 16ms (60 FPS) for smooth fades
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        
        // Request repaint for audio playback (Phase 4)
        #[cfg(feature = "audio")]
        if self.audio_playback.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        
        // Always request repaint if debug UI is showing
        if self.ui_state.show_debug_ui {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Called on shutdown to save persistent state
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dock_state", &self.dock_state);
        log::info!("Saved UI layout");
    }
}
