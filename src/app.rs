//! Main application state and logic

use crate::cue::{Cue, CueList, PlaybackEngine};
use crate::dmx::{Universe, backends::{DmxBackend, VirtualBackend}};
#[cfg(feature = "usb")]
use crate::dmx::backends::EnttecUsbProBackend;
use crate::media::MediaManager;
use crate::fixtures::FixtureLibrary;
use crate::show::{ShowFile, CueColorSettings, RgbaColor};
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
    Groups,
    Properties,
    InstrumentProperties,
    MagicSheet,
    Effects,
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
            TabKind::Groups => write!(f, "Groups"),
            TabKind::Properties => write!(f, "Cue Properties"),
            TabKind::InstrumentProperties => write!(f, "Instrument Properties"),
            TabKind::MagicSheet => write!(f, "Magic Sheet"),
            TabKind::Effects => write!(f, "Effects"),
            TabKind::Unknown => write!(f, "?"),
        }
    }
}

/// Ephemeral per-session state for the magic sheet panel (not saved to disk).
pub struct MagicSheetState {
    pub edit_mode: bool,
    /// Currently selected shape IDs in edit mode (multi-select).
    pub selected_shape_ids: std::collections::HashSet<u32>,
    /// Canvas pan offset in screen pixels.
    pub canvas_offset: egui::Vec2,
    /// Zoom level: 1.0 = 100%.
    pub canvas_zoom: f32,
    /// Clipboard for copy/paste (snapshot of shape data).
    pub clipboard: Vec<crate::magic_sheet::MagicSheetShape>,
    /// Whether a drag-select rubber-band is in progress.
    pub drag_select_start: Option<egui::Pos2>,
}

impl MagicSheetState {
    /// Return the single selected ID if exactly one shape is selected, else None.
    #[allow(dead_code)]
    pub fn single_selected(&self) -> Option<u32> {
        if self.selected_shape_ids.len() == 1 {
            self.selected_shape_ids.iter().copied().next()
        } else {
            None
        }
    }
}

impl Default for MagicSheetState {
    fn default() -> Self {
        Self {
            edit_mode: false,
            selected_shape_ids: std::collections::HashSet::new(),
            canvas_offset: egui::Vec2::ZERO,
            canvas_zoom: 1.0,
            clipboard: Vec::new(),
            drag_select_start: None,
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
    pub pending_delete_cue_id: Option<u32>,
    pub pending_update_cue_id: Option<u32>,
    pub show_quit_confirmation: bool,
    pub show_device_selector: bool,
    pub show_colour_settings: bool,
    pub show_fixture_editor: bool,
    pub show_help_shortcuts: bool,
    pub show_help_about: bool,
    pub show_autosave_prompt: bool,
    pub autosave_path: Option<std::path::PathBuf>,
    pub selected_usb_port: String,
    pub selected_open_dmx_port: String,

    /// On-deck cue override: cue number typed by operator. Empty = use the default next cue.
    pub go_cue_input: String,

    // Art-Net configuration UI state
    pub artnet_target_ip: String,
    pub artnet_universe: u16,

    /// True while the operator is in Ctrl+G goto mode (typing a cue number to jump to).
    pub goto_mode: bool,

    /// Edit buffer for the Adjust cue "Target Cue" text field (persists across frames while typing).
    #[cfg(feature = "audio")]
    pub adjust_target_edit: String,

    /// HSV colour wheel widget state (shared across single- and multi-fixture panels).
    pub color_wheel: crate::ui::ColorWheel,
    /// Which single fixture the wheel was last synced from; None when multi-select was active.
    pub last_wheel_fixture_id: Option<usize>,

    /// Effect selected in the Effects panel.
    pub selected_effect_id: Option<u32>,
    /// Effect chosen in the Cue Properties "add effect action" combo.
    pub cue_props_effect_choice: Option<u32>,
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
            pending_delete_cue_id: None,
            pending_update_cue_id: None,
            show_quit_confirmation: false,
            show_device_selector: false,
            show_colour_settings: false,
            show_fixture_editor: false,
            show_help_shortcuts: false,
            show_help_about: false,
            show_autosave_prompt: false,
            autosave_path: None,
            selected_usb_port: String::new(),
            selected_open_dmx_port: String::new(),
            go_cue_input: String::new(),
            goto_mode: false,
            artnet_target_ip: "255.255.255.255".to_string(),
            artnet_universe: 0,
            #[cfg(feature = "audio")]
            adjust_target_edit: String::new(),
            #[cfg(feature = "audio")]
            audio_file_cache: HashMap::new(),
            show_debug_ui: false,
            color_wheel: crate::ui::ColorWheel::new(),
            last_wheel_fixture_id: None,
            selected_effect_id: None,
            cue_props_effect_choice: None,
        }
    }
}

impl UiState {
    pub fn update_command_context(&mut self) {
        self.command_context = match self.active_pane {
            Some(TabKind::Channels) | Some(TabKind::Cues) | Some(TabKind::MagicSheet) => CommandContext::Lighting,
            _ => CommandContext::General,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum PersistedDmxBackend {
    Virtual,
    UsbPro { port: String },
    OpenDmx { port: String },
    ArtNet { target: String, universe: u16 },
}

impl Default for PersistedDmxBackend {
    fn default() -> Self {
        Self::Virtual
    }
}

/// Main application state
pub struct EasyCueApp {
    pub universes: Vec<Universe>,
    pub dmx_backend: Box<dyn DmxBackend>,
    /// Unified cue list — contains both lighting and audio cues
    pub cue_list: CueList,
    pub playback: PlaybackEngine,
    /// Show-level effect library (saved with the show file).
    pub effect_list: crate::effects::EffectList,
    /// Runtime state of currently running effects (never persisted).
    pub effect_engine: crate::effects::EffectEngine,
    /// This frame's effect-modulated output for UI display (None when no
    /// effects run). Panels read modulated values from here; edits go to base.
    pub effect_display: Option<crate::effects::EffectDisplay>,
    #[allow(dead_code)]
    pub media: MediaManager,
    pub fixtures: FixtureLibrary,
    pub virtual_intensity: crate::fixtures::VirtualIntensity,
    /// Lighting groups — fixture selection shortcuts.
    pub groups: crate::groups::GroupList,
    pub ui_state: UiState,
    pub fixture_editor: crate::ui::FixtureEditorState,
    pub patching_state: crate::ui::PatchingPanelState,
    pub groups_state: crate::ui::GroupsPanelState,
    /// Serialised magic sheet layout (saved with the show file).
    pub magic_sheet: crate::magic_sheet::MagicSheet,
    /// Ephemeral magic sheet panel state (not saved).
    pub magic_sheet_state: MagicSheetState,
    pub show_title: String,
    pub current_file_path: Option<std::path::PathBuf>,
    pub dock_state: DockState<TabKind>,
    pub cue_colors: CueColorSettings,

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

    /// In-progress sound master fade driven by an Adjust cue.
    #[cfg(feature = "audio")]
    pub sound_fade: Option<SoundFadeState>,

    /// Last saved file path to persistent storage (avoid redundant saves).
    last_persisted_file_path: Option<std::path::PathBuf>,
    /// Operator-selected DMX backend to restore on the next launch.
    preferred_dmx_backend: PersistedDmxBackend,
    /// Last DMX preference written to persistent storage.
    last_persisted_dmx_backend: PersistedDmxBackend,
    /// Whether a DMX backend preference existed in persistent storage at startup.
    startup_had_saved_dmx_backend: bool,
}

/// Tracks a timed fade of the sound master, driven by an Adjust cue.
#[cfg(feature = "audio")]
pub struct SoundFadeState {
    pub start_volume: f32,
    pub target_volume: f32,
    pub fade_time: f32,
    pub start: std::time::Instant,
    pub stop_when_complete: bool,
    /// Stable ID of the Adjust cue that triggered this fade (for row highlighting).
    pub trigger_cue_id: u32,
}

impl EasyCueApp {
    pub fn color32_from_rgba(color: RgbaColor) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(color.r, color.g, color.b, color.a)
    }

    pub fn rgba_from_color32(color: egui::Color32) -> RgbaColor {
        let [r, g, b, a] = color.to_array();
        RgbaColor { r, g, b, a }
    }

    pub fn reset_cue_colors_to_defaults(&mut self) {
        self.cue_colors = CueColorSettings::default();
    }

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
        let app_init_start = std::time::Instant::now();
        log::info!("[startup] EasyCueApp::new begin");

        let theme_start = std::time::Instant::now();
        Self::configure_cobalt_theme(&cc.egui_ctx);
        log::info!("[startup] Theme configured in {:.2}ms", theme_start.elapsed().as_secs_f64() * 1000.0);

        let font_start = std::time::Instant::now();
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        log::info!("[startup] Fonts configured in {:.2}ms", font_start.elapsed().as_secs_f64() * 1000.0);

        let universe_start = std::time::Instant::now();
        // Create 8 universes (1-based IDs 1–8). Only those referenced by patched
        // fixtures will carry any output; the rest stay at zero and cost nothing.
        let universes: Vec<Universe> = (1..=8).map(Universe::new).collect();
        log::info!("[startup] Universes created in {:.2}ms", universe_start.elapsed().as_secs_f64() * 1000.0);

        let saved_dmx_backend = cc.storage
            .and_then(|storage| eframe::get_value(storage, "preferred_dmx_backend"));
        let had_saved_dmx_backend = saved_dmx_backend.is_some();

        let dmx_init_start = std::time::Instant::now();
        let dmx_backend: Box<dyn DmxBackend> = Box::new(VirtualBackend::new(true));
        log::info!("[startup] DMX backend selected in {:.2}ms", dmx_init_start.elapsed().as_secs_f64() * 1000.0);

        log::info!("EasyCue3 application initialized");
        log::info!("DMX Backend: {}", dmx_backend.name());

        let dock_load_start = std::time::Instant::now();
        let dock_state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, "dock_state").unwrap_or_else(|| Self::create_default_dock_layout())
        } else {
            Self::create_default_dock_layout()
        };
        log::info!("[startup] Dock layout restored in {:.2}ms", dock_load_start.elapsed().as_secs_f64() * 1000.0);

        #[cfg(feature = "audio")]
        let (audio_player, audio_playback) = {
            let audio_init_start = std::time::Instant::now();
            log::info!("[startup][audio] Initializing audio subsystem");
            let player = AudioPlayer::new().unwrap_or_else(|e| {
                panic!("Could not open default audio output: {}", e);
            });
            let playback = AudioPlaybackEngine::new();
            log::info!(
                "[startup][audio] Audio subsystem initialized in {:.2}ms",
                audio_init_start.elapsed().as_secs_f64() * 1000.0,
            );
            (player, playback)
        };

        #[cfg(not(feature = "audio"))]
        let (audio_player, audio_playback) = {
            let audio_init_start = std::time::Instant::now();
            let player = crate::audio::AudioPlayer::new().unwrap();
            let playback = crate::audio::AudioPlaybackEngine::new();
            log::info!(
                "[startup][audio] Audio stubs initialized in {:.2}ms",
                audio_init_start.elapsed().as_secs_f64() * 1000.0
            );
            (player, playback)
        };

        let mut app = Self {
            universes,
            dmx_backend,
            cue_list: CueList::new(),
            playback: PlaybackEngine::new(),
            effect_list: crate::effects::EffectList::new(),
            effect_engine: crate::effects::EffectEngine::new(),
            effect_display: None,
            media: MediaManager::new(),
            fixtures: FixtureLibrary::new(),
            virtual_intensity: crate::fixtures::VirtualIntensity::new(),
            groups: crate::groups::GroupList::default(),
            ui_state: UiState::default(),
            fixture_editor: crate::ui::FixtureEditorState::default(),
            patching_state: crate::ui::PatchingPanelState::default(),
            groups_state: crate::ui::GroupsPanelState::default(),
            magic_sheet: crate::magic_sheet::MagicSheet::default(),
            magic_sheet_state: MagicSheetState::default(),
            show_title: "Example Show".to_string(),
            current_file_path: None,
            dock_state,
            cue_colors: CueColorSettings::default(),
            audio_player,
            audio_playback,
            autofollow_timer: None,
            #[cfg(feature = "audio")]
            sound_fade: None,
            last_persisted_file_path: None,
            preferred_dmx_backend: saved_dmx_backend.clone().unwrap_or_default(),
            last_persisted_dmx_backend: saved_dmx_backend.unwrap_or_default(),
            startup_had_saved_dmx_backend: had_saved_dmx_backend,
        };

        app.restore_startup_dmx_backend();

        let startup_show_load_start = std::time::Instant::now();
        let last_file = cc.storage
            .and_then(|s| s.get_string("last_file"))
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists());

        let loaded_path = if let Some(path) = last_file {
            match app.load_show(&path) {
                Ok(_) => {
                    log::info!("Loaded last used show: {}", path.display());
                    Some(path)
                }
                Err(e) => {
                    log::warn!("Could not load last used show: {}", e);
                    None
                }
            }
        } else {
            if let Some(default_path) = crate::paths::find_resource_file(std::path::Path::new("shows/default_show.json")) {
                match app.load_show(&default_path) {
                    Ok(_) => {
                        log::info!("Loaded default show on startup");
                        Some(default_path)
                    }
                    Err(e) => {
                        log::warn!("Could not load default show: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        };

        // Check for autosave recovery after show is loaded
        if let Some(autosave_path) = Self::check_autosave_recovery(loaded_path.as_deref()) {
            app.ui_state.show_autosave_prompt = true;
            app.ui_state.autosave_path = Some(autosave_path);
            log::info!("Autosave recovery available on startup");
        }

        log::info!(
            "[startup] Startup show load phase completed in {:.2}ms",
            startup_show_load_start.elapsed().as_secs_f64() * 1000.0
        );
        log::info!(
            "[startup] EasyCueApp::new finished in {:.2}ms",
            app_init_start.elapsed().as_secs_f64() * 1000.0
        );

        app
    }

    fn create_default_dock_layout() -> DockState<TabKind> {
        let mut dock_state = DockState::new(vec![TabKind::Channels]);
        let tree = dock_state.main_surface_mut();
        // Channels (TL) | Instrument Properties + Patching (TR)
        // Cues      (BL) | Cue Properties + Magic Sheet    (BR)
        // Ratios tuned to mirror the persisted app.ron layout baseline.
        let [top, bottom] = tree.split_below(
            egui_dock::NodeIndex::root(),
            0.462_599_84,
            vec![TabKind::Cues],
        );

        let [_, _] = tree.split_right(
            top,
            0.588_360_5,
            vec![TabKind::InstrumentProperties, TabKind::Patching],
        );

        let [_, _] = tree.split_right(
            bottom,
            0.607_848_4,
            vec![TabKind::Properties, TabKind::MagicSheet, TabKind::Effects],
        );

        dock_state
    }

    pub fn reset_dock_layout(&mut self) {
        self.dock_state = Self::create_default_dock_layout();
        log::info!("Reset UI layout to default");
    }

    /// Load a show file and populate the cue list
    pub fn load_show(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let load_start = std::time::Instant::now();
        log::info!("[show][load] Begin loading {}", path.display());

        let show = ShowFile::load(path)?;
        self.cue_list.clear();
        for cue in show.cues {
            self.cue_list.add_cue(cue);
        }
        self.cue_list.set_next_id(show.next_cue_id);

        // Load patch — preserve fixture ID and universe from the saved file.
        *self.fixtures.patch_list_mut() = crate::fixtures::PatchList::new();
        for patch in show.patch {
            if self.fixtures.get_profile(&patch.profile_id).is_some() {
                match self.fixtures.add_patch_with_id(
                    patch.id,
                    patch.label.clone(),
                    patch.profile_id.clone(),
                    patch.start_address,
                    patch.universe,
                ) {
                    Ok(_) => log::debug!(
                        "Loaded patch: {} ({}) at U{}:{}",
                        patch.label, patch.profile_id, patch.universe, patch.start_address
                    ),
                    Err(e) => log::warn!("Failed to load patch '{}': {}", patch.label, e),
                }
            } else {
                log::warn!("Skipping patch '{}': profile '{}' not found", patch.label, patch.profile_id);
            }
        }

        self.effect_engine.clear();
        self.effect_list = crate::effects::EffectList::from_parts(show.effects, show.next_effect_id);
        self.ui_state.selected_effect_id = None;
        self.ui_state.cue_props_effect_choice = None;

        self.groups = show.groups;
        self.magic_sheet = show.magic_sheet;
        self.cue_colors = show.cue_colors;
        self.magic_sheet_state = MagicSheetState {
            canvas_offset: egui::Vec2::new(
                self.magic_sheet.canvas_offset[0],
                self.magic_sheet.canvas_offset[1],
            ),
            canvas_zoom: self.magic_sheet.canvas_zoom,
            ..MagicSheetState::default()
        };
        self.show_title = path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        self.current_file_path = Some(path.to_path_buf());
        self.ui_state.selected_cue_id = None;
        self.ui_state.selected_lighting_cue_id = None;
        self.ui_state.selected_audio_cue_id = None;
        self.ui_state.status_message = format!("Loaded show from {:?}", path);
        log::info!(
            "[show][load] Loaded show: {} ({} cues, {} fixtures) in {:.2}ms",
            self.show_title,
            self.cue_list.len(),
            self.fixtures.patch_list().len(),
            load_start.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }

    /// Save the current show to a file
    pub fn save_show(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");

        let mut show = ShowFile::new();
        show.next_cue_id = self.cue_list.next_id();
        show.cues = self.cue_list.cues().to_vec();
        show.patch = self.fixtures.patch_list().patches().to_vec();
        show.groups = self.groups.clone();
        show.magic_sheet = self.magic_sheet.clone();
        show.cue_colors = self.cue_colors.clone();
        show.effects = self.effect_list.effects().to_vec();
        show.next_effect_id = self.effect_list.next_id();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        show.save(path)?;
        self.current_file_path = Some(path.to_path_buf());
        self.show_title = title.to_string();
        log::info!("Saved show: {} ({} cues, {} fixtures)", title, show.cues.len(), show.patch.len());
        Ok(())
    }

    /// Apply lighting master and blackout before DMX output.
    /// Returns a cloned Vec of universes with masters applied, ready to send.
    pub fn apply_masters(&self, universes: &[Universe]) -> Vec<Universe> {
        universes.iter().map(|universe| {
            let mut output = universe.clone();
            if self.ui_state.blackout_active {
                output.clear();
                return output;
            }
            if self.ui_state.lighting_master < 1.0 {
                for ch in 1..=512u16 {
                    if let Ok(value) = universe.get_channel(ch) {
                        if value > 0 {
                            let scaled = (value as f32 * self.ui_state.lighting_master).round() as u8;
                            let _ = output.set_channel(ch, scaled);
                        }
                    }
                }
            }
            output
        }).collect()
    }

    pub fn switch_to_virtual(&mut self) {
        self.activate_virtual_backend();
        self.preferred_dmx_backend = PersistedDmxBackend::Virtual;
        log::info!("Switched to Virtual DMX backend");
    }

    #[cfg(feature = "usb")]
    pub fn switch_to_enttec(&mut self, port: &str) -> anyhow::Result<()> {
        self.activate_virtual_backend();
        let backend = EnttecUsbProBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        self.ui_state.selected_usb_port = port.to_string();
        self.preferred_dmx_backend = PersistedDmxBackend::UsbPro { port: port.to_string() };
        log::info!("Switched to Enttec USB Pro at {}", port);
        Ok(())
    }

    #[cfg(feature = "usb")]
    pub fn switch_to_open_dmx(&mut self, port: &str) -> anyhow::Result<()> {
        use crate::dmx::backends::{EnttecOpenDmxBackend, VirtualBackend};
        // Drop the current backend before opening the new port. If the current backend
        // is an Open DMX (or Pro), its Drop impl joins the output thread, which releases
        // the serial port FD — otherwise the open() below would fail with EBUSY.
        self.dmx_backend = Box::new(VirtualBackend::default());
        let backend = EnttecOpenDmxBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        self.ui_state.selected_open_dmx_port = port.to_string();
        self.preferred_dmx_backend = PersistedDmxBackend::OpenDmx { port: port.to_string() };
        log::info!("Switched to Enttec Open DMX USB at {}", port);
        Ok(())
    }

    /// Switch to Art-Net UDP output. `target` is the destination IP (or broadcast).
    /// `universe` is the Art-Net universe number (0-based, 0–32767).
    pub fn switch_to_artnet(&mut self, target: &str, universe: u16) -> anyhow::Result<()> {
        use crate::dmx::backends::ArtNetBackend;
        let backend = ArtNetBackend::new(target, universe)?;
        self.dmx_backend = Box::new(backend);
        self.ui_state.artnet_target_ip = target.to_string();
        self.ui_state.artnet_universe = universe;
        self.preferred_dmx_backend = PersistedDmxBackend::ArtNet {
            target: target.to_string(),
            universe,
        };
        log::info!("Switched to Art-Net → {} universe {}", target, universe);
        Ok(())
    }

    fn activate_virtual_backend(&mut self) {
        self.dmx_backend = Box::new(VirtualBackend::new(true));
    }

    fn sync_ui_dmx_selection_from_preference(&mut self) {
        match &self.preferred_dmx_backend {
            PersistedDmxBackend::Virtual => {}
            PersistedDmxBackend::UsbPro { port } => {
                self.ui_state.selected_usb_port = port.clone();
            }
            PersistedDmxBackend::OpenDmx { port } => {
                self.ui_state.selected_open_dmx_port = port.clone();
            }
            PersistedDmxBackend::ArtNet { target, universe } => {
                self.ui_state.artnet_target_ip = target.clone();
                self.ui_state.artnet_universe = *universe;
            }
        }
    }

    fn restore_startup_dmx_backend(&mut self) {
        self.sync_ui_dmx_selection_from_preference();

        let preferred = self.preferred_dmx_backend.clone();
        let restore_result = match &preferred {
            PersistedDmxBackend::Virtual => {
                self.activate_virtual_backend();
                Ok(())
            }
            #[cfg(feature = "usb")]
            PersistedDmxBackend::UsbPro { port } => self.switch_to_enttec(port),
            #[cfg(not(feature = "usb"))]
            PersistedDmxBackend::UsbPro { .. } => anyhow::bail!("USB support not enabled"),
            #[cfg(feature = "usb")]
            PersistedDmxBackend::OpenDmx { port } => self.switch_to_open_dmx(port),
            #[cfg(not(feature = "usb"))]
            PersistedDmxBackend::OpenDmx { .. } => anyhow::bail!("USB support not enabled"),
            PersistedDmxBackend::ArtNet { target, universe } => self.switch_to_artnet(target, *universe),
        };

        if let Err(error) = restore_result {
            log::warn!(
                "[startup][dmx] Could not restore saved DMX backend {:?}: {}. Falling back to Virtual DMX",
                self.preferred_dmx_backend,
                error
            );
            self.activate_virtual_backend();
            self.ui_state.status_message = format!(
                "Saved DMX device unavailable — using Virtual DMX instead"
            );
            return;
        }

        if !self.startup_had_saved_dmx_backend
            && matches!(self.preferred_dmx_backend, PersistedDmxBackend::Virtual)
        {
            #[cfg(feature = "usb")]
            {
                let (tx, rx) = std::sync::mpsc::channel::<Option<EnttecUsbProBackend>>();
                std::thread::spawn(move || {
                    let scan_thread_start = std::time::Instant::now();
                    log::info!("[startup][dmx] USB scan thread started");

                    let result = match EnttecUsbProBackend::list_recommended_ports() {
                        Ok(ports) => {
                            log::info!(
                                "[startup][dmx] USB port enumeration completed in {:.2}ms ({} ports)",
                                scan_thread_start.elapsed().as_secs_f64() * 1000.0,
                                ports.len()
                            );
                            if let Some(port) = ports.into_iter().next() {
                                let connect_start = std::time::Instant::now();
                                log::info!("[startup][dmx] Attempting Enttec open on {}", port);
                                match EnttecUsbProBackend::new(&port) {
                                    Ok(backend) => {
                                        log::info!(
                                            "[startup][dmx] Enttec open succeeded in {:.2}ms",
                                            connect_start.elapsed().as_secs_f64() * 1000.0
                                        );
                                        Some(backend)
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "[startup][dmx] Enttec open failed in {:.2}ms: {}",
                                            connect_start.elapsed().as_secs_f64() * 1000.0,
                                            e
                                        );
                                        None
                                    }
                                }
                            } else {
                                log::info!("[startup][dmx] No USB serial devices detected; skipping Enttec probe and using Virtual DMX");
                                None
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "[startup][dmx] USB port enumeration failed in {:.2}ms: {}",
                                scan_thread_start.elapsed().as_secs_f64() * 1000.0,
                                e
                            );
                            None
                        }
                    };

                    log::info!(
                        "[startup][dmx] USB scan thread finished in {:.2}ms",
                        scan_thread_start.elapsed().as_secs_f64() * 1000.0
                    );
                    let _ = tx.send(result);
                });

                let wait_start = std::time::Instant::now();
                log::info!("[startup][dmx] Waiting up to 3s for USB scan thread");
                match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                    Ok(Some(backend)) => {
                        let port_name = backend.name().to_string();
                        log::info!(
                            "[startup][dmx] Connected to Enttec DMXUSB Pro after {:.2}ms: {}",
                            wait_start.elapsed().as_secs_f64() * 1000.0,
                            port_name
                        );
                        self.dmx_backend = Box::new(backend);
                        if let Some(port) = Self::extract_port_from_backend_name(&port_name) {
                            self.ui_state.selected_usb_port = port.clone();
                            self.preferred_dmx_backend = PersistedDmxBackend::UsbPro { port };
                        }
                    }
                    Ok(None) => {
                        log::info!(
                            "[startup][dmx] No Enttec USB device found after {:.2}ms, using Virtual DMX",
                            wait_start.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                    Err(_) => {
                        log::warn!(
                            "[startup][dmx] USB scan wait timed out at {:.2}ms (Bluetooth stall suspected) — using Virtual DMX",
                            wait_start.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                }
            }
        }
    }

    fn extract_port_from_backend_name(name: &str) -> Option<String> {
        let start = name.find('(')? + 1;
        let end = name.rfind(')')?;
        if start >= end {
            return None;
        }
        Some(name[start..end].to_string())
    }

    // --- Effects ---

    /// Resolve fixture (patch) IDs into plain channel data for the effect engine.
    /// Unknown fixtures are skipped with a warning. Called at effect start / cue
    /// fire / jump sync — never per frame, so repatching mid-effect uses stale
    /// addresses until the effect is restarted (acceptable).
    pub fn resolve_effect_fixtures(&self, ids: &[usize]) -> Vec<crate::effects::EffectFixture> {
        use crate::fixtures::profiles::FixtureParameter;
        let mut resolved = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(patch) = self.fixtures.patch_list().get_patch(id) else {
                log::warn!("Effect fixture #{} not found in patch — skipping", id);
                continue;
            };
            let Some(profile) = self.fixtures.get_profile(&patch.profile_id) else {
                log::warn!("Effect fixture #{}: profile '{}' missing — skipping", id, patch.profile_id);
                continue;
            };
            let universe_idx = (patch.universe as usize).saturating_sub(1);
            if universe_idx >= self.universes.len() {
                log::warn!("Effect fixture #{}: universe {} not available — skipping", id, patch.universe);
                continue;
            }
            let abs = |offset: u16| patch.start_address + offset;
            let rgb_chs = match (
                profile.get_parameter_offset(&FixtureParameter::Red),
                profile.get_parameter_offset(&FixtureParameter::Green),
                profile.get_parameter_offset(&FixtureParameter::Blue),
            ) {
                (Some(r), Some(g), Some(b)) => Some((abs(r), abs(g), abs(b))),
                _ => None,
            };
            resolved.push(crate::effects::EffectFixture {
                fixture_id: id,
                universe_idx,
                intensity_ch: profile.get_parameter_offset(&FixtureParameter::Intensity).map(abs),
                color_chs: profile.color_parameters().iter().map(|p| abs(p.channel_offset)).collect(),
                rgb_chs,
                pan_ch: profile.get_parameter_offset(&FixtureParameter::Pan).map(abs),
                tilt_ch: profile.get_parameter_offset(&FixtureParameter::Tilt).map(abs),
            });
        }
        resolved
    }

    /// Run a cue's effect actions: starts ramp in over the cue's fade-up,
    /// stops ramp out over its fade-down.
    fn execute_effect_actions(&mut self, actions: &[crate::effects::EffectAction], fade_up: f32, fade_down: f32) {
        use crate::effects::EffectAction;
        for action in actions {
            match action {
                EffectAction::Start { effect_id, fixtures } => {
                    if self.effect_list.find(*effect_id).is_none() {
                        log::warn!("Cue references missing effect {} — skipping", effect_id);
                        continue;
                    }
                    let resolved = self.resolve_effect_fixtures(fixtures);
                    self.effect_engine.start(*effect_id, fixtures.clone(), resolved, fade_up);
                }
                EffectAction::Stop { effect_id } => self.effect_engine.stop(*effect_id, fade_down),
                EffectAction::StopAll => self.effect_engine.stop_all(fade_down),
            }
        }
    }

    /// Reconcile running effects with the tracked effect state at cue index
    /// `idx` — the effect analogue of `tracked_state_up_to`, used by BACK and
    /// GOTO so jumps land with the correct effects running. Retargets keep the
    /// effect clock, so surviving effects never phase-snap.
    fn sync_effects_to_index(&mut self, idx: usize, fade: f32) {
        let desired = self.cue_list.effect_state_up_to(idx);
        let running_ids: Vec<u32> = self.effect_engine.running().iter().map(|r| r.effect_id()).collect();
        for id in running_ids {
            if !desired.iter().any(|(d, _)| *d == id) {
                self.effect_engine.stop(id, fade);
            }
        }
        for (id, fixture_ids) in desired {
            if self.effect_list.find(id).is_none() {
                log::warn!("Tracked effect {} missing from library — skipping", id);
                continue;
            }
            let needs_start = match self.effect_engine.running().iter().find(|r| r.effect_id() == id) {
                Some(r) => r.is_stopping() || r.fixture_ids() != fixture_ids.as_slice(),
                None => true,
            };
            if needs_start {
                let resolved = self.resolve_effect_fixtures(&fixture_ids);
                self.effect_engine.start(id, fixture_ids, resolved, fade);
            }
        }
    }

    // --- Navigation helpers (all UI panels call these instead of engines directly) ---

    /// Advance to the next cue of any kind (unified GO). Returns true if a cue fired.
    pub fn go_next(&mut self) -> bool {
        self.autofollow_timer = None;
        let Some(next_idx) = self.cue_list.next_any_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                self.playback.start(&cue, &self.universes);
                self.execute_effect_actions(&data.effect_actions, data.fade_up, data.fade_down);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(data) => {
                self.fire_adjust_cue(cue.id, data.clone());
                true
            }
        };
        if fired {
            self.ui_state.selected_cue_id = None;
            self.ui_state.go_cue_input.clear();
            self.cue_list.set_current_index(Some(next_idx));
            log::info!("GO → cue {:.1} '{}'", cue.number, cue.label);
            if let Some(delay) = cue.autofollow.filter(|&d| d > 0.0) {
                self.autofollow_timer = Some((std::time::Instant::now(), delay));
                log::info!("  autofollow armed: {:.1}s", delay);
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
            crate::cue::CueKind::Lighting(data) => {
                let tracked = self.cue_list.tracked_state_up_to(prev_idx);
                let fade_time = data.fade_up;
                self.playback.start_to_state(&tracked, fade_time, Some(cue.id), &self.universes);
                self.sync_effects_to_index(prev_idx, fade_time);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(data) => {
                self.fire_adjust_cue(cue.id, data.clone());
                true
            }
        };
        if fired {
            self.cue_list.set_current_index(Some(prev_idx));
            log::info!("BACK → cue {:.1} '{}'", cue.number, cue.label);
        }
        fired
    }

    /// Advance to the next lighting cue and start its fade. Returns true if a cue fired.
    #[allow(dead_code)]
    pub fn go_lighting(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_lighting_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        self.playback.start(&cue, &self.universes);
        self.cue_list.set_current_index(Some(next_idx));
        log::info!("Lighting GO → cue {:.1} '{}'", cue.number, cue.label);
        true
    }

    /// Return to the previous lighting cue. Returns true if a cue fired.
    #[allow(dead_code)]
    pub fn go_back_lighting(&mut self) -> bool {
        let Some(prev_idx) = self.cue_list.previous_lighting_index() else { return false };
        let cue = self.cue_list.get_cue(prev_idx).cloned();
        let Some(cue) = cue else { return false };
        self.playback.start(&cue, &self.universes);
        self.cue_list.set_current_index(Some(prev_idx));
        log::info!("Lighting BACK → cue {:.1} '{}'", cue.number, cue.label);
        true
    }

    /// Advance to the next audio cue and start playback. Returns true if a cue fired.
    #[cfg(feature = "audio")]
    #[allow(dead_code)]
    pub fn go_audio(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_audio_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = self.audio_playback.start(&cue, &self.audio_player);
        if fired {
            self.cue_list.set_current_index(Some(next_idx));
            log::info!("Audio GO → cue {:.1} '{}'", cue.number, cue.label);
        }
        fired
    }

    /// Return to the previous audio cue. Returns true if a cue fired.
    #[cfg(feature = "audio")]
    #[allow(dead_code)]
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

    /// Jump to and fire the cue at `abs_idx` (regardless of kind). Updates the play head
    /// and arms autofollow — identical behaviour to go_next().
    pub fn go_to_cue(&mut self, abs_idx: usize) -> bool {
        self.autofollow_timer = None;
        let cue = self.cue_list.get_cue(abs_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                let tracked = self.cue_list.tracked_state_up_to(abs_idx);
                let fade_time = data.fade_up;
                self.playback.start_to_state(&tracked, fade_time, Some(cue.id), &self.universes);
                self.sync_effects_to_index(abs_idx, fade_time);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback.start(&cue, &self.audio_player)
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(data) => {
                self.fire_adjust_cue(cue.id, data.clone());
                true
            }
        };
        if fired {
            self.ui_state.selected_cue_id = None;
            self.cue_list.set_current_index(Some(abs_idx));
            log::info!("GO→ cue {:.1} '{}'", cue.number, cue.label);
            if let Some(delay) = cue.autofollow.filter(|&d| d > 0.0) {
                self.autofollow_timer = Some((std::time::Instant::now(), delay));
                log::info!("  autofollow armed: {:.1}s", delay);
            }
        }
        fired
    }

    /// Jump to a cue by its display number. Cue 0 is a special blackout: fades lights to zero
    /// and stops all audio. Returns true if the operation succeeded.
    pub fn goto_cue_by_number(&mut self, num: f32) -> bool {
        if num == 0.0 {
            self.fade_to_black(3.0);
            return true;
        }
        let idx = self.cue_list.cues().iter()
            .position(|c| (c.number - num).abs() < 0.005);
        if let Some(abs_idx) = idx {
            self.go_to_cue(abs_idx)
        } else {
            self.ui_state.status_message = format!("Cue {:.1} not found", num);
            log::warn!("Goto: cue {:.1} not found", num);
            false
        }
    }

    /// Fade all lighting channels across all universes to zero over `fade_seconds`
    /// and stop all audio immediately.
    pub fn fade_to_black(&mut self, fade_seconds: f32) {
        self.playback.start_fade_to_black(&self.universes, fade_seconds);
        // With the base fading to 0, a running intensity effect would keep
        // flashing 0→size in the black — Cue 0 stops effects with the fade.
        self.effect_engine.stop_all(fade_seconds);
        #[cfg(feature = "audio")]
        self.audio_playback.stop_all();
        self.autofollow_timer = None;
        self.cue_list.set_current_index(None);
        log::info!("Cue 0: blackout ({:.1}s fade)", fade_seconds);
    }

    /// Execute an Adjust cue: fade per-device volume/pan on the targeted audio stream.
    /// `target_audio_cue = None` targets all playing streams.
    #[cfg(feature = "audio")]
    fn fire_adjust_cue(&mut self, _adjust_cue_id: u32, data: crate::cue::AdjustData) {
        let target_id: u32 = if let Some(target_num) = data.target_audio_cue {
            self.cue_list.cues().iter()
                .find(|c| (c.number - target_num).abs() < 0.005)
                .map(|c| c.id)
                .unwrap_or_else(|| {
                    log::warn!("Adjust: target cue {:.1} not found", target_num);
                    0
                })
        } else {
            0
        };

        for fade in &data.output_fades {
            self.audio_playback.adjust_stream_output(
                target_id,
                &fade.device_name,
                fade.target_volume,
                fade.target_pan,
                data.fade_time,
                data.stop_when_complete,
            );
        }

        log::info!(
            "Adjust: {} fade(s) on {} over {:.1}s{}",
            data.output_fades.len(),
            data.target_audio_cue.map(|n| format!("Q{:.1}", n)).unwrap_or_else(|| "all".into()),
            data.fade_time,
            if data.stop_when_complete { " then stop" } else { "" },
        );
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

        // Tracking mode: only record channels that differ from the accumulated state
        // of all existing cues. A channel going to 0 that was non-zero must be stored
        // explicitly so the next cue knows to fade it out.
        let tracked = if self.cue_list.is_empty() {
            std::collections::HashMap::new()
        } else {
            self.cue_list.tracked_state_up_to(self.cue_list.len() - 1)
        };

        if let Some(data) = cue.lighting_data_mut() {
            for (uni_idx, universe) in self.universes.iter().enumerate() {
                let universe_num = (uni_idx + 1) as u16;
                for ch in 1u16..=512 {
                    if let Ok(live_val) = universe.get_channel(ch) {
                        let key = crate::cue::universe_key(universe_num, ch);
                        let tracked_val = tracked.get(&key).copied().unwrap_or(0);
                        if live_val != tracked_val {
                            data.channel_values.insert(key, live_val);
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

    /// Compare two show files, ignoring timestamps. Returns true if they're substantially identical.
    fn shows_are_equivalent(show_a: &ShowFile, show_b: &ShowFile) -> bool {
        // Serialize to JSON and compare, which ignores timestamp fields
        let json_a = serde_json::to_value(show_a).ok();
        let json_b = serde_json::to_value(show_b).ok();

        if let (Some(mut a), Some(mut b)) = (json_a, json_b) {
            // Remove timestamp fields before comparing
            if let Some(obj_a) = a.as_object_mut() {
                obj_a.remove("created");
                obj_a.remove("modified");
            }
            if let Some(obj_b) = b.as_object_mut() {
                obj_b.remove("created");
                obj_b.remove("modified");
            }
            a == b
        } else {
            false
        }
    }

    /// Check if autosave exists, is more recent than the loaded show, and has different content.
    /// If so, offer to recover it.
    fn check_autosave_recovery(loaded_path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
        let autosave_path = std::path::PathBuf::from("shows/.autosave.json");

        if !autosave_path.exists() {
            return None;
        }

        let autosave_mtime = std::fs::metadata(&autosave_path)
            .ok()
            .and_then(|m| m.modified().ok());

        if autosave_mtime.is_none() {
            return None;
        }

        let loaded_mtime = loaded_path
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        // Autosave must be more recent than the loaded file (or there's no loaded file)
        if let (Some(autosave_time), Some(loaded_time)) = (autosave_mtime, loaded_mtime) {
            if autosave_time <= loaded_time {
                return None;
            }
        }

        // Compare file contents, ignoring timestamps
        match (ShowFile::load(&autosave_path), loaded_path.and_then(|p| ShowFile::load(p).ok())) {
            (Ok(autosave), Some(loaded)) => {
                if !Self::shows_are_equivalent(&autosave, &loaded) {
                    log::info!("Autosave recovery: found newer, different autosave");
                    return Some(autosave_path);
                }
            }
            (Ok(_), None) => {
                log::info!("Autosave recovery: no loaded show, but autosave exists");
                return Some(autosave_path);
            }
            _ => {}
        }

        None
    }
}

impl eframe::App for EasyCueApp {
    /// Force-terminate the process on window close so that any background threads
    /// stuck in blocking OS calls (e.g. IOKit serial-port enumeration on macOS)
    /// don't keep the process alive after the user has quit.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let shutdown_start = std::time::Instant::now();
        log::warn!("[shutdown] on_exit invoked; beginning shutdown sequence");

        #[cfg(feature = "audio")]
        {
            let audio_stop_start = std::time::Instant::now();
            self.audio_playback.stop_all();
            log::info!(
                "[shutdown] audio_playback.stop_all completed in {:.2}ms",
                audio_stop_start.elapsed().as_secs_f64() * 1000.0
            );
        }

        let autosave_start = std::time::Instant::now();
        let autosave_path = std::path::PathBuf::from("shows/.autosave.json");
        match self.save_show(&autosave_path) {
            Ok(_) => {
                log::info!(
                    "[shutdown] Auto-saved to {:?} in {:.2}ms",
                    autosave_path,
                    autosave_start.elapsed().as_secs_f64() * 1000.0
                );
            }
            Err(e) => {
                log::warn!(
                    "[shutdown] Failed to auto-save: {} (took {:.2}ms)",
                    e,
                    autosave_start.elapsed().as_secs_f64() * 1000.0
                );
            }
        }

        let dmx_close_start = std::time::Instant::now();
        match self.dmx_backend.close() {
            Ok(()) => {
                log::info!(
                    "[shutdown] dmx_backend.close completed in {:.2}ms",
                    dmx_close_start.elapsed().as_secs_f64() * 1000.0
                );
            }
            Err(e) => {
                log::error!(
                    "[shutdown] dmx_backend.close failed after {:.2}ms: {}",
                    dmx_close_start.elapsed().as_secs_f64() * 1000.0,
                    e
                );
            }
        }

        log::warn!(
            "[shutdown] Forcing process exit after {:.2}ms total shutdown work",
            shutdown_start.elapsed().as_secs_f64() * 1000.0
        );
        log::logger().flush();
        std::process::exit(0);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.ui_state.theme_initialized {
            Self::configure_cobalt_theme(ctx);
            self.ui_state.theme_initialized = true;
            log::info!("Theme reapplied in update()");
        }

        // Suppress hotkeys while any text field (label editors, property boxes, etc.) has focus.
        // Ctrl+R (record) is safe to allow regardless.
        let text_focused = ctx.memory(|m| m.focused().is_some());
        let (go, stop, record, ctrl_g, escape, arrow_up, arrow_down) = ctx.input(|i| (
            i.key_pressed(egui::Key::Space)     && !i.modifiers.any() && !text_focused,
            i.key_pressed(egui::Key::S)         && !i.modifiers.any() && !text_focused,
            i.key_pressed(egui::Key::R)         && i.modifiers.ctrl,
            i.key_pressed(egui::Key::G)         && i.modifiers.ctrl && !text_focused,
            // Escape is a safety/pause key — works even when a text field has focus.
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::ArrowUp)   && !text_focused,
            i.key_pressed(egui::Key::ArrowDown) && !text_focused,
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
        if ctrl_g {
            self.ui_state.goto_mode = true;
            // Prefix with "go" so execute_goto can strip it and parse the number.
            self.ui_state.command_input = "go".to_string();
        }
        // Escape: always fade out audio (safety stop). Freeze lighting only if playing.
        // Skip if goto_mode is active — Escape should cancel that instead.
        if escape && !self.ui_state.goto_mode {
            if self.playback.is_playing() {
                self.playback.freeze();
            }
            #[cfg(feature = "audio")]
            self.audio_playback.stop_all_with_fade(1.0);
            self.autofollow_timer = None;
            self.ui_state.status_message = "Paused".to_string();
        }
        // Up/Down arrows: navigate the cue selection and set on-deck.
        if arrow_up || arrow_down {
            let cue_count = self.cue_list.len();
            if cue_count > 0 {
                // Prefer the currently selected cue as the movement origin; fall back to
                // the current on-deck cue so arrows always move relative to what's next.
                let current_sel = self.ui_state.selected_cue_id
                    .and_then(|id| self.cue_list.cues().iter().position(|c| c.id == id))
                    .or_else(|| self.cue_list.next_any_index());
                let new_idx = if arrow_up {
                    current_sel.map(|i| i.saturating_sub(1)).unwrap_or(0)
                } else {
                    current_sel.map(|i| (i + 1).min(cue_count - 1)).unwrap_or(0)
                };
                if let Some(cue) = self.cue_list.get_cue(new_idx) {
                    let num = cue.number;
                    let id  = cue.id;
                    let is_lighting = cue.is_lighting();
                    // Move play head to just before this cue so next_any_index() points here.
                    let prev_idx = if new_idx > 0 { Some(new_idx - 1) } else { None };
                    drop(cue);
                    self.cue_list.set_current_index(prev_idx);
                    self.ui_state.selected_cue_id = Some(id);
                    if is_lighting {
                        self.ui_state.selected_lighting_cue_id = Some(id);
                        self.ui_state.selected_audio_cue_id = None;
                    } else {
                        self.ui_state.selected_audio_cue_id = Some(id);
                        self.ui_state.selected_lighting_cue_id = None;
                    }
                    self.ui_state.go_cue_input = format!("{:.1}", num);
                    self.ui_state.status_message = format!("On deck: Q{:.1}", num);
                }
            }
        }
        if go {
            let pending_idx = {
                let input = self.ui_state.go_cue_input.trim();
                if input.is_empty() {
                    None
                } else {
                    input.parse::<f32>().ok().and_then(|num| {
                        self.cue_list.cues().iter()
                            .position(|c| (c.number - num).abs() < 0.005)
                    })
                }
            };
            if let Some(abs_idx) = pending_idx {
                if self.go_to_cue(abs_idx) {
                    self.ui_state.go_cue_input.clear();
                }
            } else {
                self.go_next();
            }
        }

        // Autofollow: fire next cue when timer elapses
        if let Some((start, delay)) = self.autofollow_timer {
            if start.elapsed().as_secs_f32() >= delay {
                self.autofollow_timer = None;
                self.go_next();
            }
        }

        // Adjust cue: ramp sound master toward target
        #[cfg(feature = "audio")]
        if let Some(fade) = self.sound_fade.take() {
            let elapsed = fade.start.elapsed().as_secs_f32();
            let progress = if fade.fade_time > 0.0 {
                (elapsed / fade.fade_time).clamp(0.0, 1.0)
            } else {
                1.0
            };
            self.ui_state.sound_master =
                fade.start_volume + (fade.target_volume - fade.start_volume) * progress;
            if progress < 1.0 {
                self.sound_fade = Some(fade); // put it back, still running
            } else if fade.stop_when_complete {
                self.audio_playback.stop_all();
                log::debug!("Adjust fade complete: stopping all audio");
            }
        }

        self.playback.update(&mut self.universes);

        // Keep VirtualIntensity state in sync with whatever the playback engine wrote to the
        // universes this frame, so that intensity reads in the UI panels are never stale.
        if self.playback.is_playing() {
            let patches: Vec<_> = self.fixtures.patch_list().patches().to_vec();
            for patch in &patches {
                let uni_idx = (patch.universe as usize).saturating_sub(1);
                if let Some(universe) = self.universes.get(uni_idx) {
                    if let Some(profile) = self.fixtures.get_profile(&patch.profile_id) {
                        if !profile.has_intensity() {
                            self.virtual_intensity.update_from_universe(
                                patch.id, universe, patch, profile,
                            );
                        }
                    }
                }
            }
        }

        #[cfg(feature = "audio")]
        self.audio_playback.update(self.ui_state.sound_master);

        // Auto-fallback: if the hardware backend lost the device, switch to Virtual.
        if !self.dmx_backend.is_connected() {
            log::warn!("DMX device lost — falling back to Virtual DMX");
            self.ui_state.status_message = format!(
                "DMX device lost — switched to Virtual (was: {})",
                self.dmx_backend.name()
            );
            self.activate_virtual_backend();
        }

        let dmx_send_start = std::time::Instant::now();
        // Effects modulate a clone of the base look at output time only, then
        // masters scale the result — blackout and grand master govern effect
        // output, and the stored universes never see effect values.
        let output_universes = if self.effect_engine.is_active() {
            let mut staged = self.universes.clone();
            let footprint = self.effect_engine.apply(&mut staged, &self.effect_list);
            let output = self.apply_masters(&staged);
            // Keep the pre-master staged look for UI readouts (panels always
            // show pre-master values, so FX display matches that convention).
            self.effect_display = Some(crate::effects::EffectDisplay { universes: staged, footprint });
            output
        } else {
            self.effect_display = None;
            self.apply_masters(&self.universes)
        };
        if let Err(e) = self.dmx_backend.send_universes(&output_universes) {
            log::error!("DMX output error: {}", e);
        }
        let dmx_send_time = dmx_send_start.elapsed();

        let ui_render_start = std::time::Instant::now();
        crate::ui::render(ctx, self);
        let ui_render_time = ui_render_start.elapsed();

        if self.ui_state.show_debug_ui {
            egui::Window::new(format!("{} Debug Info", egui_phosphor::regular::BUG))
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
        // Effects animate the DMX output every frame; without this the app
        // idles and a running effect freezes between input events.
        if self.effect_engine.is_active() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(feature = "audio")]
        if self.audio_playback.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.ui_state.show_debug_ui {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        // Persist file path only if it changed (avoid redundant saves every frame)
        if self.current_file_path != self.last_persisted_file_path
            || self.preferred_dmx_backend != self.last_persisted_dmx_backend
        {
            if let Some(storage) = frame.storage_mut() {
                self.save(storage);
                self.last_persisted_file_path = self.current_file_path.clone();
                self.last_persisted_dmx_backend = self.preferred_dmx_backend.clone();
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dock_state", &self.dock_state);
        eframe::set_value(storage, "preferred_dmx_backend", &self.preferred_dmx_backend);
        if let Some(path) = &self.current_file_path {
            storage.set_string("last_file", path.to_string_lossy().to_string());
        }
        log::info!("Saved UI layout");
    }
}
