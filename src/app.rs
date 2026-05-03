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
use crate::audio::{AudioPlayer, AudioPlaybackEngine};

/// Panel types for the docking system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TabKind {
    Channels,
    Cues,       // unified lighting + audio cue list
    Patching,
    Properties,
    // Legacy variants kept for saved dock state deserialization — never shown
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for TabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabKind::Channels => write!(f, "Channels"),
            TabKind::Cues => write!(f, "Cues"),
            TabKind::Patching => write!(f, "Patching"),
            TabKind::Properties => write!(f, "Properties"),
            TabKind::Unknown => write!(f, "?"),
        }
    }
}

/// UI state flags and dialog state
pub struct UiState {
    // Selection state (by stable cue ID, not index)
    /// Stable ID of the selected cue in the unified cue list
    pub selected_cue_id: Option<u32>,
    /// Stable ID of the currently selected lighting cue (legacy, kept for properties panel)
    pub selected_lighting_cue_id: Option<u32>,
    /// Stable ID of the currently selected audio cue (legacy, kept for properties panel)
    pub selected_audio_cue_id: Option<u32>,
    /// Currently selected channels for editing (supports multi-select)
    pub selected_channels: HashSet<u16>,
    /// Last selected channel for shift-range selection
    pub last_selected_channel: Option<u16>,
    /// Stored base levels for proportional scaling (L_i in formula O_i = M * L_i)
    pub channel_base_levels: HashMap<u16, u8>,
    /// Current master level for proportional group control (M in formula, 0-100)
    pub group_master: u8,

    // Fixture selection state
    pub selected_fixtures: HashSet<usize>,
    pub last_selected_fixture: Option<usize>,
    pub show_unpatched_channels: bool,

    pub status_message: String,
    pub command_input: String,

    // Master levels and toggles
    pub lighting_master: f32,
    pub sound_master: f32,
    pub previous_lighting_master: f32,
    pub previous_sound_master: f32,
    pub blackout_active: bool,
    pub audio_mute_active: bool,

    pub active_pane: Option<TabKind>,
    pub command_context: CommandContext,

    /// Cached file existence checks (path -> exists)
    #[cfg(feature = "audio")]
    pub audio_file_cache: HashMap<std::path::PathBuf, bool>,

    pub show_debug_ui: bool,
    pub theme_initialized: bool,

    // Dialog states
    pub show_quit_confirmation: bool,
    pub show_device_selector: bool,
    pub selected_usb_port: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_cue_id: None,
            selected_lighting_cue_id: None,
            selected_audio_cue_id: None,
            selected_channels: HashSet::new(),
            last_selected_channel: None,
            channel_base_levels: HashMap::new(),
            group_master: 100,
            selected_fixtures: HashSet::new(),
            last_selected_fixture: None,
            show_unpatched_channels: false,
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
            #[cfg(feature = "audio")]
            audio_file_cache: HashMap::new(),
            show_debug_ui: false,
        }
    }
}

impl UiState {
    pub fn update_command_context(&mut self) {
        self.command_context = match self.active_pane {
            Some(TabKind::Channels) | Some(TabKind::Cues) => CommandContext::Lighting,
            _ => CommandContext::General,
        };
    }
}

/// Main application state
pub struct EasyCueApp {
    pub universes: Vec<Universe>,
    pub dmx_backend: Box<dyn DmxBackend>,
    /// Unified cue list — contains both lighting and audio cues
    pub cue_list: CueList,
    pub playback: PlaybackEngine,
    pub media: MediaManager,
    pub fixtures: FixtureLibrary,
    pub virtual_intensity: crate::fixtures::VirtualIntensity,
    pub ui_state: UiState,
    pub patching_state: crate::ui::PatchingPanelState,
    pub show_title: String,
    pub current_file_path: Option<std::path::PathBuf>,
    pub dock_state: DockState<TabKind>,

    #[cfg(feature = "audio")]
    pub audio_player: AudioPlayer,
    #[cfg(feature = "audio")]
    pub audio_playback: AudioPlaybackEngine,
    #[cfg(not(feature = "audio"))]
    pub audio_player: crate::audio::AudioPlayer,
    #[cfg(not(feature = "audio"))]
    pub audio_playback: crate::audio::AudioPlaybackEngine,

    /// Pending autofollow: time the current cue fired + delay to wait before calling go_next()
    pub autofollow_timer: Option<(std::time::Instant, f32)>,
}

impl EasyCueApp {
    fn configure_cobalt_theme(ctx: &egui::Context) {
        let mut style = egui::Style {
            visuals: egui::Visuals::dark(),
            ..(*ctx.style()).clone()
        };

        let bg_deep = egui::Color32::from_rgb(5, 20, 40);
        let bg_main = egui::Color32::from_rgb(10, 30, 55);
        let bg_lighter = egui::Color32::from_rgb(20, 45, 75);
        let bg_hover = egui::Color32::from_rgb(30, 60, 100);
        let accent_blue = egui::Color32::from_rgb(30, 150, 255);
        let accent_cyan = egui::Color32::from_rgb(0, 220, 255);
        let text_bright = egui::Color32::from_rgb(255, 255, 255);
        let text_dim = egui::Color32::from_rgb(150, 190, 220);
        let border_color = egui::Color32::from_rgb(50, 100, 150);

        style.visuals = egui::Visuals {
            dark_mode: true,
            override_text_color: Some(text_bright),
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
            selection: egui::style::Selection {
                bg_fill: accent_blue.linear_multiply(0.4),
                stroke: egui::Stroke::new(1.0, accent_cyan),
            },
            hyperlink_color: accent_cyan,
            faint_bg_color: bg_deep,
            extreme_bg_color: bg_deep,
            code_bg_color: bg_deep,
            warn_fg_color: egui::Color32::from_rgb(255, 200, 0),
            error_fg_color: egui::Color32::from_rgb(255, 80, 80),
            window_fill: bg_main,
            window_stroke: egui::Stroke::new(1.0, border_color),
            window_corner_radius: egui::CornerRadius::same(6),
            window_shadow: egui::epaint::Shadow {
                offset: [4, 4],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(180),
            },
            panel_fill: bg_main,
            popup_shadow: egui::epaint::Shadow {
                offset: [4, 4],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(180),
            },
            resize_corner_size: 12.0,
            text_cursor: egui::style::TextCursorStyle {
                stroke: egui::Stroke::new(2.0, accent_cyan),
                ..Default::default()
            },
            clip_rect_margin: 3.0,
            button_frame: true,
            collapsing_header_frame: false,
            indent_has_left_vline: true,
            striped: true,
            slider_trailing_fill: true,
            handle_shape: egui::style::HandleShape::Circle,
            menu_corner_radius: egui::CornerRadius::same(4),
            ..Default::default()
        };

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 20.0;
        style.spacing.slider_width = 150.0;

        ctx.set_style(style);
        log::info!("Applied cobalt dark theme");
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_cobalt_theme(&cc.egui_ctx);

        let universes = vec![Universe::new(0), Universe::new(1)];

        let dmx_backend: Box<dyn DmxBackend> = {
            #[cfg(feature = "usb")]
            {
                match EnttecUsbProBackend::list_ports() {
                    Ok(ports) if !ports.is_empty() => {
                        match EnttecUsbProBackend::new(&ports[0]) {
                            Ok(backend) => {
                                log::info!("✓ Connected to Enttec DMXUSB Pro at {}", ports[0]);
                                Box::new(backend)
                            }
                            Err(e) => {
                                log::warn!("Failed to connect to Enttec device: {}", e);
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
            virtual_intensity: crate::fixtures::VirtualIntensity::new(),
            ui_state: UiState::default(),
            patching_state: crate::ui::PatchingPanelState::default(),
            show_title: "Example Show".to_string(),
            current_file_path: None,
            dock_state,
            #[cfg(feature = "audio")]
            audio_player: AudioPlayer::new().unwrap_or_else(|e| {
                log::error!("Failed to initialize audio player: {}", e);
                AudioPlayer::new().unwrap()
            }),
            #[cfg(feature = "audio")]
            audio_playback: AudioPlaybackEngine::new(),
            #[cfg(not(feature = "audio"))]
            audio_player: crate::audio::AudioPlayer::new().unwrap(),
            #[cfg(not(feature = "audio"))]
            audio_playback: crate::audio::AudioPlaybackEngine::new(),
            autofollow_timer: None,
        };

        let example_path = std::path::Path::new("shows/example_show.json");
        if example_path.exists() {
            match app.load_show(example_path) {
                Ok(_) => log::info!("Loaded example show on startup"),
                Err(e) => log::warn!("Could not load example show: {}", e),
            }
        }

        app
    }

    fn create_default_dock_layout() -> DockState<TabKind> {
        let mut dock_state = DockState::new(vec![TabKind::Channels]);
        let tree = dock_state.main_surface_mut();
        let [_channels, _properties] = tree.split_right(egui_dock::NodeIndex::root(), 0.7, vec![TabKind::Properties]);
        let [_, _] = tree.split_below(egui_dock::NodeIndex::root(), 0.5, vec![TabKind::Cues, TabKind::Patching]);
        dock_state
    }

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
        self.cue_list.set_next_id(show.next_cue_id);

        // Load patch
        *self.fixtures.patch_list_mut() = crate::fixtures::PatchList::new();
        for patch in show.patch {
            if self.fixtures.get_profile(&patch.profile_id).is_some() {
                match self.fixtures.add_patch(patch.label.clone(), patch.profile_id.clone(), patch.start_address) {
                    Ok(_) => log::debug!("Loaded patch: {} ({}) at {}", patch.label, patch.profile_id, patch.start_address),
                    Err(e) => log::warn!("Failed to load patch '{}': {}", patch.label, e),
                }
            } else {
                log::warn!("Skipping patch '{}': profile '{}' not found", patch.label, patch.profile_id);
            }
        }

        self.show_title = show.title.clone();
        self.current_file_path = Some(path.to_path_buf());
        self.ui_state.selected_cue_id = None;
        self.ui_state.selected_lighting_cue_id = None;
        self.ui_state.selected_audio_cue_id = None;
        self.ui_state.status_message = format!("Loaded show from {:?}", path);
        log::info!("Loaded show: {} ({} cues, {} fixtures)",
            self.show_title, self.cue_list.len(), self.fixtures.patch_list().len());
        Ok(())
    }

    /// Save the current show to a file
    pub fn save_show(&mut self, path: &std::path::Path, title: &str) -> anyhow::Result<()> {
        let mut show = ShowFile::new(title);
        show.next_cue_id = self.cue_list.next_id();
        show.cues = self.cue_list.cues().to_vec();
        show.patch = self.fixtures.patch_list().patches().to_vec();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        show.save(path)?;
        self.current_file_path = Some(path.to_path_buf());
        log::info!("Saved show: {} ({} cues, {} fixtures)", title, show.cues.len(), show.patch.len());
        Ok(())
    }

    /// Apply lighting master and blackout before DMX output
    pub fn apply_masters(&self, universe: &Universe) -> Universe {
        let mut output = universe.clone();
        if self.ui_state.blackout_active {
            output.clear();
            return output;
        }
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

    pub fn switch_to_virtual(&mut self) {
        self.dmx_backend = Box::new(VirtualBackend::new(true));
        log::info!("Switched to Virtual DMX backend");
    }

    #[cfg(feature = "usb")]
    pub fn switch_to_enttec(&mut self, port: &str) -> anyhow::Result<()> {
        let backend = EnttecUsbProBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        log::info!("Switched to Enttec USB Pro at {}", port);
        Ok(())
    }

    /// Check if adding a lighting→audio trigger would create a circular dependency
    #[cfg(feature = "audio")]
    pub fn would_create_circular_light_to_audio(&self, lighting_cue_num: f32, audio_cue_num: f32) -> bool {
        self.cue_list.cues().iter()
            .filter(|c| c.is_audio())
            .find(|c| (c.number - audio_cue_num).abs() < 0.01)
            .and_then(|c| c.audio_data())
            .and_then(|d| d.triggers_lighting_cue)
            .map(|trigger| (trigger - lighting_cue_num).abs() < 0.01)
            .unwrap_or(false)
    }

    /// Check if adding an audio→lighting trigger would create a circular dependency
    #[cfg(feature = "audio")]
    pub fn would_create_circular_audio_to_light(&self, audio_cue_num: f32, lighting_cue_num: f32) -> bool {
        self.cue_list.cues().iter()
            .filter(|c| c.is_lighting())
            .find(|c| (c.number - lighting_cue_num).abs() < 0.01)
            .and_then(|c| c.lighting_data())
            .and_then(|d| d.triggers_audio_cue)
            .map(|trigger| (trigger - audio_cue_num).abs() < 0.01)
            .unwrap_or(false)
    }

    // --- Navigation helpers (all UI panels call these instead of engines directly) ---

    /// Advance to the next cue of any kind (unified GO). Returns true if a cue fired.
    pub fn go_next(&mut self) -> bool {
        self.autofollow_timer = None;
        let Some(next_idx) = self.cue_list.next_any_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(_) => {
                if let Some(universe) = self.universes.first() {
                    self.playback.start(&cue, universe);
                    true
                } else { false }
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
        };
        if fired {
            self.cue_list.set_current_index(Some(next_idx));
            log::info!("GO → cue {:.1} '{}'", cue.number, cue.label);
            if let Some(delay) = cue.autofollow.filter(|&d| d > 0.0) {
                self.autofollow_timer = Some((std::time::Instant::now(), delay));
                log::info!("  autofollow armed: {:.1}s", delay);
            }
            #[cfg(feature = "audio")]
            if cue.is_lighting() {
                self.fire_audio_cross_trigger(cue.id);
            } else {
                self.fire_lighting_cross_trigger(cue.id);
            }
        }
        fired
    }

    /// Return to the previous cue of any kind (unified BACK). Returns true if a cue fired.
    pub fn go_back(&mut self) -> bool {
        self.autofollow_timer = None;
        let Some(prev_idx) = self.cue_list.previous_any_index() else { return false };
        let cue = self.cue_list.get_cue(prev_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(_) => {
                if let Some(universe) = self.universes.first() {
                    self.playback.start(&cue, universe);
                    true
                } else { false }
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
        };
        if fired {
            self.cue_list.set_current_index(Some(prev_idx));
            log::info!("BACK → cue {:.1} '{}'", cue.number, cue.label);
        }
        fired
    }

    /// Advance to the next lighting cue and start its fade. Returns true if a cue fired.
    pub fn go_lighting(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_lighting_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        if let Some(universe) = self.universes.first() {
            self.playback.start(&cue, universe);
        }
        self.cue_list.set_current_index(Some(next_idx));
        log::info!("Lighting GO → cue {:.1} '{}'", cue.number, cue.label);
        #[cfg(feature = "audio")]
        self.fire_audio_cross_trigger(cue.id);
        true
    }

    /// Return to the previous lighting cue. Returns true if a cue fired.
    pub fn go_back_lighting(&mut self) -> bool {
        let Some(prev_idx) = self.cue_list.previous_lighting_index() else { return false };
        let cue = self.cue_list.get_cue(prev_idx).cloned();
        let Some(cue) = cue else { return false };
        if let Some(universe) = self.universes.first() {
            self.playback.start(&cue, universe);
        }
        self.cue_list.set_current_index(Some(prev_idx));
        log::info!("Lighting BACK → cue {:.1} '{}'", cue.number, cue.label);
        true
    }

    /// Advance to the next audio cue and start playback. Returns true if a cue fired.
    #[cfg(feature = "audio")]
    pub fn go_audio(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_audio_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = self.audio_playback.start(&cue, &self.audio_player);
        if fired {
            self.cue_list.set_current_index(Some(next_idx));
            log::info!("Audio GO → cue {:.1} '{}'", cue.number, cue.label);
            self.fire_lighting_cross_trigger(cue.id);
        }
        fired
    }

    /// Return to the previous audio cue. Returns true if a cue fired.
    #[cfg(feature = "audio")]
    pub fn go_back_audio(&mut self) -> bool {
        let Some(prev_idx) = self.cue_list.previous_audio_index() else { return false };
        let cue = self.cue_list.get_cue(prev_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = self.audio_playback.start(&cue, &self.audio_player);
        if fired {
            self.cue_list.set_current_index(Some(prev_idx));
            log::info!("Audio BACK → cue {:.1} '{}'", cue.number, cue.label);
        }
        fired
    }

    /// Jump to and fire the cue at `abs_idx` (regardless of kind). Updates the play head.
    pub fn go_to_cue(&mut self, abs_idx: usize) -> bool {
        let cue = self.cue_list.get_cue(abs_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(_) => {
                if let Some(universe) = self.universes.first() {
                    self.playback.start(&cue, universe);
                }
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
        };
        if fired {
            self.cue_list.set_current_index(Some(abs_idx));
        }
        fired
    }

    /// Fire the audio cross-trigger linked to the given lighting cue, if any.
    #[cfg(feature = "audio")]
    fn fire_audio_cross_trigger(&mut self, lighting_cue_id: u32) {
        let trigger_num = self.cue_list.find_by_id(lighting_cue_id)
            .and_then(|c| c.lighting_data())
            .and_then(|d| d.triggers_audio_cue);
        if let Some(audio_num) = trigger_num {
            let audio_idx = self.cue_list.cues().iter()
                .position(|c| c.is_audio() && (c.number - audio_num).abs() < 0.01);
            if let Some(idx) = audio_idx {
                let cue = self.cue_list.get_cue(idx).cloned();
                if let Some(cue) = cue {
                    if self.audio_playback.start(&cue, &self.audio_player) {
                        log::info!("Cross-trigger: lighting → audio cue {:.2}", audio_num);
                    }
                }
            }
        }
    }

    /// Fire the lighting cross-trigger linked to the given audio cue, if any.
    #[cfg(feature = "audio")]
    fn fire_lighting_cross_trigger(&mut self, audio_cue_id: u32) {
        let trigger_num = self.cue_list.find_by_id(audio_cue_id)
            .and_then(|c| c.audio_data())
            .and_then(|d| d.triggers_lighting_cue);
        if let Some(light_num) = trigger_num {
            let light_idx = self.cue_list.cues().iter()
                .position(|c| c.is_lighting() && (c.number - light_num).abs() < 0.01);
            if let Some(idx) = light_idx {
                let cue = self.cue_list.get_cue(idx).cloned();
                if let Some(cue) = cue {
                    if let Some(universe) = self.universes.first() {
                        self.playback.start(&cue, universe);
                        log::info!("Cross-trigger: audio → lighting cue {:.2}", light_num);
                    }
                }
            }
        }
    }

    /// Record a new lighting cue from the current universe state.
    /// Returns the stable ID assigned to the new cue.
    pub fn record_cue(&mut self) -> u32 {
        let next_number = self.cue_list.cues()
            .iter()
            .filter(|c| c.is_lighting())
            .last()
            .map(|c| c.number.floor() + 1.0)
            .unwrap_or(1.0);

        let mut cue = Cue::new_lighting(next_number);
        cue.label = format!("Cue {:.0}", next_number);

        // The ID that will be assigned by add_cue (cue.id is 0 → next_id is used)
        let assigned_id = self.cue_list.next_id();

        if let Some(universe) = self.universes.first() {
            if let Some(data) = cue.lighting_data_mut() {
                for ch in 1u16..=512 {
                    if let Ok(val) = universe.get_channel(ch) {
                        if val > 0 {
                            data.set_channel(ch, val);
                        }
                    }
                }
            }
        }

        let channel_count = cue.lighting_data().map(|d| d.channel_values.len()).unwrap_or(0);
        self.cue_list.add_cue(cue);
        self.ui_state.status_message = format!("Recorded cue {:.0}", next_number);
        log::info!("Recorded cue {:.0} with {} channels", next_number, channel_count);
        assigned_id
    }
}

impl eframe::App for EasyCueApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.ui_state.theme_initialized {
            Self::configure_cobalt_theme(ctx);
            self.ui_state.theme_initialized = true;
            log::info!("Theme reapplied in update()");
        }

        let (go, stop, record) = ctx.input(|i| (
            i.key_pressed(egui::Key::Space) && !i.modifiers.any(),
            i.key_pressed(egui::Key::S)     && !i.modifiers.any(),
            i.key_pressed(egui::Key::R)     && i.modifiers.ctrl,
        ));

        if stop {
            self.playback.stop();
            #[cfg(feature = "audio")]
            self.audio_playback.stop_all();
            self.autofollow_timer = None;
        }
        if record {
            let id = self.record_cue();
            self.ui_state.selected_cue_id = Some(id);
            self.ui_state.selected_lighting_cue_id = Some(id);
        }
        if go {
            self.go_next();
        }

        // Autofollow: fire next cue when timer elapses
        if let Some((start, delay)) = self.autofollow_timer {
            if start.elapsed().as_secs_f32() >= delay {
                self.autofollow_timer = None;
                self.go_next();
            }
        }

        if let Some(universe) = self.universes.first_mut() {
            self.playback.update(universe);
        }

        #[cfg(feature = "audio")]
        {
            self.audio_playback.update(self.ui_state.sound_master);

            // Cross-triggers from audio→lighting queued at cue-start time
            for lighting_num in self.audio_playback.take_pending_lighting_triggers() {
                let light_idx = self.cue_list.cues().iter()
                    .position(|c| c.is_lighting() && (c.number - lighting_num).abs() < 0.01);
                if let Some(idx) = light_idx {
                    let cue = self.cue_list.get_cue(idx).cloned();
                    if let Some(cue) = cue {
                        if let Some(universe) = self.universes.first() {
                            self.playback.start(&cue, universe);
                            log::info!("Audio cross-trigger: lighting cue {:.2}", lighting_num);
                        }
                    }
                }
            }
        }

        let dmx_send_start = std::time::Instant::now();
        if let Some(universe) = self.universes.first() {
            let output_universe = self.apply_masters(universe);
            if let Err(e) = self.dmx_backend.send_universe(&output_universe) {
                log::error!("DMX output error: {}", e);
            }
        }
        let dmx_send_time = dmx_send_start.elapsed();

        let ui_render_start = std::time::Instant::now();
        crate::ui::render(ctx, self);
        let ui_render_time = ui_render_start.elapsed();

        if self.ui_state.show_debug_ui {
            egui::Window::new("🐛 Debug Info")
                .default_pos([10.0, 10.0])
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.label(format!("FPS: {:.1}", ctx.input(|i| 1.0 / i.stable_dt)));
                    ui.label(format!("Frame time: {:.2}ms", ctx.input(|i| i.stable_dt * 1000.0)));
                    ui.separator();
                    ui.label(egui::RichText::new("Performance:").strong());
                    ui.label(format!("  DMX send: {:.2}ms", dmx_send_time.as_secs_f64() * 1000.0));
                    ui.label(format!("  UI render: {:.2}ms", ui_render_time.as_secs_f64() * 1000.0));
                    ui.separator();
                    ui.label(format!("Total cues: {}", self.cue_list.len()));
                    #[cfg(feature = "audio")]
                    {
                        let audio_count = self.cue_list.cues().iter().filter(|c| c.is_audio()).count();
                        let lighting_count = self.cue_list.cues().iter().filter(|c| c.is_lighting()).count();
                        ui.label(format!("  Lighting: {}  Audio: {}", lighting_count, audio_count));
                        ui.label(format!("File cache: {} entries", self.ui_state.audio_file_cache.len()));
                        ui.label(format!("Audio playing: {}", self.audio_playback.is_playing()));
                    }
                    ui.label(format!("Lighting playing: {}", self.playback.is_playing()));
                    ui.separator();
                    if ui.button("Clear file cache").clicked() {
                        #[cfg(feature = "audio")]
                        self.ui_state.audio_file_cache.clear();
                    }
                });
        }

        if self.playback.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(feature = "audio")]
        if self.audio_playback.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.ui_state.show_debug_ui {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dock_state", &self.dock_state);
        log::info!("Saved UI layout");
    }
}
