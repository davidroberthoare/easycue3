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
    Properties,
    InstrumentProperties,
    MagicSheet,
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
            TabKind::Properties => write!(f, "Cue Properties"),
            TabKind::InstrumentProperties => write!(f, "Instrument Properties"),
            TabKind::MagicSheet => write!(f, "Magic Sheet"),
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
    pub show_quit_confirmation: bool,
    pub show_device_selector: bool,
    pub show_colour_settings: bool,
    pub show_fixture_editor: bool,
    pub selected_usb_port: String,

    /// On-deck cue override: cue number typed by operator. Empty = use the default next cue.
    pub go_cue_input: String,

    /// Edit buffer for the Adjust cue "Target Cue" text field (persists across frames while typing).
    #[cfg(feature = "audio")]
    pub adjust_target_edit: String,

    /// HSV colour wheel widget state (shared across single- and multi-fixture panels).
    pub color_wheel: crate::ui::ColorWheel,
    /// Which single fixture the wheel was last synced from; None when multi-select was active.
    pub last_wheel_fixture_id: Option<usize>,
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
            show_colour_settings: false,
            show_fixture_editor: false,
            selected_usb_port: String::new(),
            go_cue_input: String::new(),
            #[cfg(feature = "audio")]
            adjust_target_edit: String::new(),
            #[cfg(feature = "audio")]
            audio_file_cache: HashMap::new(),
            show_debug_ui: false,
            color_wheel: crate::ui::ColorWheel::new(),
            last_wheel_fixture_id: None,
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
    pub fixture_editor: crate::ui::FixtureEditorState,
    pub patching_state: crate::ui::PatchingPanelState,
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
        let universes = vec![Universe::new(0), Universe::new(1)];
        log::info!("[startup] Universes created in {:.2}ms", universe_start.elapsed().as_secs_f64() * 1000.0);

        let dmx_init_start = std::time::Instant::now();
        let dmx_backend: Box<dyn DmxBackend> = {
            #[cfg(feature = "usb")]
            {
                // Run enumeration + open on a background thread with a 3-second timeout.
                // On macOS, serialport::available_ports() can hang indefinitely when
                // Bluetooth virtual serial ports (e.g. Bluetooth-Incoming-Port) are
                // present and the Bluetooth stack is slow to respond — blocking the
                // main thread and preventing the window from ever appearing.
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
                        log::info!(
                            "[startup][dmx] Connected to Enttec DMXUSB Pro after {:.2}ms: {}",
                            wait_start.elapsed().as_secs_f64() * 1000.0,
                            backend.name()
                        );
                        Box::new(backend) as Box<dyn DmxBackend>
                    }
                    Ok(None) => {
                        log::info!(
                            "[startup][dmx] No Enttec USB device found after {:.2}ms, using Virtual DMX",
                            wait_start.elapsed().as_secs_f64() * 1000.0
                        );
                        Box::new(VirtualBackend::new(true))
                    }
                    Err(_) => {
                        log::warn!(
                            "[startup][dmx] USB scan wait timed out at {:.2}ms (Bluetooth stall suspected) — using Virtual DMX",
                            wait_start.elapsed().as_secs_f64() * 1000.0
                        );
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
                log::error!("Failed to initialize audio player: {}", e);
                AudioPlayer::new().unwrap()
            });
            let playback = AudioPlaybackEngine::new();
            log::info!(
                "[startup][audio] Audio subsystem initialized in {:.2}ms",
                audio_init_start.elapsed().as_secs_f64() * 1000.0
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
            media: MediaManager::new(),
            fixtures: FixtureLibrary::new(),
            virtual_intensity: crate::fixtures::VirtualIntensity::new(),
            ui_state: UiState::default(),
            fixture_editor: crate::ui::FixtureEditorState::default(),
            patching_state: crate::ui::PatchingPanelState::default(),
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
        };

        let startup_show_load_start = std::time::Instant::now();
        let last_file = cc.storage
            .and_then(|s| s.get_string("last_file"))
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists());

        if let Some(path) = last_file {
            match app.load_show(&path) {
                Ok(_) => log::info!("Loaded last used show: {}", path.display()),
                Err(e) => log::warn!("Could not load last used show: {}", e),
            }
        } else {
            if let Some(default_path) = crate::paths::find_resource_file(std::path::Path::new("shows/default_show.json")) {
                match app.load_show(&default_path) {
                    Ok(_) => log::info!("Loaded default show on startup"),
                    Err(e) => log::warn!("Could not load default show: {}", e),
                }
            }
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
            vec![TabKind::Properties, TabKind::MagicSheet],
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
        self.show_title = show.title.clone();
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
    pub fn save_show(&mut self, path: &std::path::Path, title: &str) -> anyhow::Result<()> {
        let mut show = ShowFile::new(title);
        show.next_cue_id = self.cue_list.next_id();
        show.cues = self.cue_list.cues().to_vec();
        show.patch = self.fixtures.patch_list().patches().to_vec();
        show.magic_sheet = self.magic_sheet.clone();
        show.cue_colors = self.cue_colors.clone();

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
    pub fn go_lighting(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_lighting_index() else { return false };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        if let Some(universe) = self.universes.first() {
            self.playback.start(&cue, universe);
        }
        self.cue_list.set_current_index(Some(next_idx));
        log::info!("Lighting GO → cue {:.1} '{}'", cue.number, cue.label);
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

    /// Jump to and fire the cue at `abs_idx` (regardless of kind). Updates the play head
    /// and arms autofollow — identical behaviour to go_next().
    pub fn go_to_cue(&mut self, abs_idx: usize) -> bool {
        self.autofollow_timer = None;
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

    /// Execute an Adjust cue: ramp a specific audio stream's volume, or the global sound master.
    #[cfg(feature = "audio")]
    fn fire_adjust_cue(&mut self, adjust_cue_id: u32, data: crate::cue::AdjustData) {
        if let Some(target_num) = data.target_audio_cue {
            // Targeted: find the active stream by cue number
            let target_id = self.cue_list.cues().iter()
                .find(|c| (c.number - target_num).abs() < 0.005)
                .map(|c| c.id);
            if let Some(target_id) = target_id {
                self.audio_playback.adjust_stream(target_id, data.volume, data.fade_time, data.stop_when_complete);
                log::info!("Adjust: cue {} → {:.0}% over {:.1}s{}", target_num, data.volume * 100.0,
                    data.fade_time, if data.stop_when_complete { " then stop" } else { "" });
            } else {
                log::warn!("Adjust: target cue {:.1} not found or not playing", target_num);
            }
        } else {
            // Global: ramp the sound master
            if data.fade_time <= 0.0 {
                self.ui_state.sound_master = data.volume;
                if data.stop_when_complete {
                    self.audio_playback.stop_all();
                }
                log::info!("Adjust: snap master to {:.0}%{}", data.volume * 100.0,
                    if data.stop_when_complete { " + stop all" } else { "" });
            } else {
                self.sound_fade = Some(SoundFadeState {
                    start_volume: self.ui_state.sound_master,
                    target_volume: data.volume,
                    fade_time: data.fade_time,
                    start: std::time::Instant::now(),
                    stop_when_complete: data.stop_when_complete,
                    trigger_cue_id: adjust_cue_id,
                });
                log::info!("Adjust: fade master to {:.0}% over {:.1}s{}", data.volume * 100.0,
                    data.fade_time, if data.stop_when_complete { " then stop all" } else { "" });
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

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.ui_state.theme_initialized {
            Self::configure_cobalt_theme(ctx);
            self.ui_state.theme_initialized = true;
            log::info!("Theme reapplied in update()");
        }

        // Suppress hotkeys while any text field (label editors, property boxes, etc.) has focus.
        // Ctrl+R (record) is safe to allow regardless.
        let text_focused = ctx.memory(|m| m.focused().is_some());
        let (go, stop, record) = ctx.input(|i| (
            i.key_pressed(egui::Key::Space) && !i.modifiers.any() && !text_focused,
            i.key_pressed(egui::Key::S)     && !i.modifiers.any() && !text_focused,
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

        if let Some(universe) = self.universes.first_mut() {
            self.playback.update(universe);
        }

        // Keep VirtualIntensity state in sync with whatever the playback engine wrote to the
        // universe this frame, so that intensity reads in the UI panels are never stale.
        if self.playback.is_playing() {
            if let Some(universe) = self.universes.first() {
                let patches: Vec<_> = self.fixtures.patch_list().patches().to_vec();
                for patch in &patches {
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
            self.switch_to_virtual();
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
        if let Some(path) = &self.current_file_path {
            storage.set_string("last_file", path.to_string_lossy().to_string());
        }
        log::info!("Saved UI layout");
    }
}
