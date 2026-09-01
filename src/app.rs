//! Main application state and logic

use crate::command::CommandContext;
use crate::cue::{Cue, CueList, PlaybackEngine};
#[cfg(feature = "usb")]
use crate::dmx::backends::EnttecUsbProBackend;
use crate::dmx::{
    backends::{DmxBackend, VirtualBackend},
    Universe,
};
use crate::fixtures::FixtureLibrary;
use crate::media::MediaManager;
use crate::show::{CueColorSettings, RgbaColor, ShowFile};
use egui_dock::DockState;
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};

#[cfg(feature = "audio")]
use crate::audio::{AudioPlaybackEngine, AudioPlayer};

/// Panel types for the docking system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TabKind {
    Channels,
    Cues, // unified lighting + audio cue list
    Patching,
    Groups,
    Properties,
    InstrumentProperties,
    MagicSheet,
    Effects,
    Submasters,
    ScriptViewer,
    Hotkeys, // Ctrl+0…9 cue hotkeys
    // Legacy variants kept for saved dock state deserialization — never shown
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for TabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabKind::Channels => write!(f, "Fixtures"),
            TabKind::Cues => write!(f, "Cues"),
            TabKind::Patching => write!(f, "Patching"),
            TabKind::Groups => write!(f, "Groups"),
            TabKind::Properties => write!(f, "Cue Properties"),
            TabKind::InstrumentProperties => write!(f, "Fixture Properties"),
            TabKind::MagicSheet => write!(f, "Magic Sheet"),
            TabKind::Effects => write!(f, "Effects"),
            TabKind::Submasters => write!(f, "Submasters"),
            TabKind::ScriptViewer => write!(f, "Script Viewer"),
            TabKind::Hotkeys => write!(f, "Hotkeys"),
            TabKind::Unknown => write!(f, "?"),
        }
    }
}

/// The top-row digit key for `d` (0→Num0 … 9→Num9).
fn digit_key(d: usize) -> egui::Key {
    match d {
        0 => egui::Key::Num0,
        1 => egui::Key::Num1,
        2 => egui::Key::Num2,
        3 => egui::Key::Num3,
        4 => egui::Key::Num4,
        5 => egui::Key::Num5,
        6 => egui::Key::Num6,
        7 => egui::Key::Num7,
        8 => egui::Key::Num8,
        9 => egui::Key::Num9,
        _ => unreachable!("digit key index {d} out of range"),
    }
}

/// What the magic sheet autonumbering tool assigns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonumberTarget {
    /// Assign fixture numbers (patch IDs) to shapes.
    Fixtures,
    /// Assign group numbers to shapes.
    Groups,
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
    /// Autonumbering tool: when enabled, clicking a shape assigns the next
    /// fixture (Fixtures) or group (Groups) number instead of selecting it.
    pub autonumber_enabled: bool,
    /// What the autonumbering tool assigns.
    pub autonumber_target: AutonumberTarget,
    /// The next number the autonumbering tool will place.
    pub autonumber_next: usize,
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
            autonumber_enabled: false,
            autonumber_target: AutonumberTarget::Fixtures,
            autonumber_next: 1,
        }
    }
}

/// Ephemeral per-session state for the submasters panel.
pub struct SubmasterPanelState {
    pub edit_mode: bool,
}

impl Default for SubmasterPanelState {
    fn default() -> Self {
        Self { edit_mode: false }
    }
}

/// Which field of a lighting cue's properties the UI should activate for editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueEditField {
    /// The cue "Label" text field.
    Label,
    /// The cue "Fade Up" text field.
    FadeUp,
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
    /// Recent frame timestamps (egui `input.time`) used by the debug overlay to
    /// report the *actual* repaint rate over the last second. `stable_dt` is a
    /// predicted timestep (usually 1/60) and can't show that.
    pub debug_frame_times: std::collections::VecDeque<f64>,
    pub theme_initialized: bool,

    // Dialog states
    pub pending_delete_cue_id: Option<u32>,
    pub pending_update_cue_id: Option<u32>,
    pub show_quit_confirmation: bool,
    /// Allows the native close request after the user confirms the quit dialog.
    pub quit_confirmed: bool,
    pub show_device_selector: bool,
    pub show_colour_settings: bool,
    pub show_fixture_editor: bool,
    pub show_help_shortcuts: bool,
    pub show_help_about: bool,
    pub show_update_dialog: bool,
    pub show_remote_settings: bool,
    /// QR texture cache for the remote settings dialog: (encoded URL, texture).
    #[cfg(feature = "remote")]
    pub remote_qr: Option<(String, egui::TextureHandle)>,
    pub show_autosave_prompt: bool,
    pub autosave_path: Option<std::path::PathBuf>,
    pub selected_usb_port: String,
    pub selected_open_dmx_port: String,

    /// Re-number Cues dialog state (see the Edit menu).
    pub show_renumber_cues: bool,
    /// True = renumber all cues; false = renumber the number range below.
    pub renumber_all: bool,
    pub renumber_from: f32,
    pub renumber_to: f32,
    pub renumber_start: f32,
    pub renumber_step: f32,
    /// One-shot: focus the dialog's Apply button on its next render so Enter
    /// applies immediately, without stealing focus back from the number fields.
    pub renumber_focus_pending: bool,

    /// On-deck cue override: cue number typed by operator. Empty = use the default next cue.
    pub go_cue_input: String,

    /// Edit buffer for the script viewer's page-jump text field (persists across
    /// frames while the operator is typing a page number).
    pub page_jump_input: String,

    // Art-Net configuration UI state
    pub artnet_target_ip: String,
    pub artnet_universe: u16,

    /// True while the operator is in Ctrl+G goto mode (typing a cue number to jump to).
    pub goto_mode: bool,

    /// Edit buffer for the Adjust cue "Target Cue" text field (persists across frames while typing).
    #[cfg(feature = "audio")]
    pub adjust_target_edit: String,

    /// Cue-property field to activate & select-all on the next frame it's rendered.
    /// Set by record and by the `l`/`i`/`q<n>l`/`q<n>i` commands; consumed by the
    /// properties panel once the field has been focused.
    pub focus_cue_edit: Option<(u32, CueEditField)>,
    /// Edit buffer for the lighting cue "Fade Up" text field.
    pub fade_up_edit: String,
    /// Edit buffer for the lighting cue "Fade Down" text field.
    pub fade_down_edit: String,

    /// HSV colour wheel widget state (shared across single- and multi-fixture panels).
    pub color_wheel: crate::ui::ColorWheel,
    /// Which single fixture the wheel was last synced from; None when multi-select was active.
    pub last_wheel_fixture_id: Option<usize>,

    /// Effect selected in the Effects panel.
    pub selected_effect_id: Option<u32>,
    /// Effect chosen in the Cue Properties "add effect action" combo.
    pub cue_props_effect_choice: Option<u32>,

    /// One-shot: scroll the cue-list table so the on-deck row (index) is kept in
    /// view the next time the Cues panel renders. Set whenever the play head
    /// moves (GO / BACK / goto / arrows) so the operator always sees what's next.
    pub pending_cue_scroll: Option<usize>,

    /// When a *fading* cue fires, this holds the `PlaybackEngine` fade id to wait
    /// for before advancing the script viewer to the on-deck cue's page. `None`
    /// means no follow-up is armed (or the cue had no fade and followed at once).
    pub script_follow_on_fade_complete: Option<u64>,
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
            quit_confirmed: false,
            show_device_selector: false,
            show_colour_settings: false,
            show_fixture_editor: false,
            show_help_shortcuts: false,
            show_help_about: false,
            show_update_dialog: false,
            show_remote_settings: false,
            #[cfg(feature = "remote")]
            remote_qr: None,
            show_autosave_prompt: false,
            autosave_path: None,
            selected_usb_port: String::new(),
            selected_open_dmx_port: String::new(),
            show_renumber_cues: false,
            renumber_all: true,
            renumber_from: 1.0,
            renumber_to: 1.0,
            renumber_start: 1.0,
            renumber_step: 1.0,
            renumber_focus_pending: false,
            go_cue_input: String::new(),
            page_jump_input: String::new(),
            goto_mode: false,
            artnet_target_ip: "255.255.255.255".to_string(),
            artnet_universe: 0,
            #[cfg(feature = "audio")]
            adjust_target_edit: String::new(),
            focus_cue_edit: None,
            fade_up_edit: String::new(),
            fade_down_edit: String::new(),
            #[cfg(feature = "audio")]
            audio_file_cache: HashMap::new(),
            show_debug_ui: false,
            debug_frame_times: std::collections::VecDeque::new(),
            color_wheel: crate::ui::ColorWheel::new(),
            last_wheel_fixture_id: None,
            selected_effect_id: None,
            cue_props_effect_choice: None,
            pending_cue_scroll: None,
            script_follow_on_fade_complete: None,
        }
    }
}

impl UiState {
    pub fn update_command_context(&mut self) {
        self.command_context = match self.active_pane {
            Some(TabKind::Channels) | Some(TabKind::Cues) | Some(TabKind::MagicSheet) | Some(TabKind::Submasters) => {
                CommandContext::Lighting
            }
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

/// Optional per-frame instrumentation used by the run-over-run benchmark.
/// It is inert unless EASYCUE_PERF_LOG is set; writes remain buffered until
/// shutdown so the measurement does not flush I/O on every frame.
struct PerfLogger {
    started: std::time::Instant,
    writer: BufWriter<std::fs::File>,
    records: u32,
}

impl PerfLogger {
    fn from_env() -> Option<Self> {
        let setting = std::env::var("EASYCUE_PERF_LOG").ok()?;
        if setting.is_empty() || setting == "0" {
            return None;
        }
        let path = if setting == "1" {
            std::env::temp_dir().join(format!("easycue3-perf-{}.csv", std::process::id()))
        } else {
            std::path::PathBuf::from(setting)
        };
        match std::fs::File::create(&path) {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                if writeln!(
                    writer,
                    "timestamp_ms,frame_time_ms,ui_render_ms,dmx_send_ms,update_cpu_ms"
                )
                .is_err()
                {
                    log::warn!("[perf] Could not initialize log file {:?}", path);
                    return None;
                }
                log::info!("[perf] Writing frame timings to {:?}", path);
                Some(Self {
                    started: std::time::Instant::now(),
                    writer,
                    records: 0,
                })
            }
            Err(e) => {
                log::warn!("[perf] Could not create log file {:?}: {}", path, e);
                None
            }
        }
    }

    /// `ui_render_ms` / `dmx_send_ms` are the CPU-side times for the UI pass and
    /// DMX output, logged so a frame-time spike can be attributed to real CPU
    /// work vs. present/wait time (frame_time - ui_render - dmx).
    fn record(&mut self, stable_dt: f32, ui_render_ms: f32, dmx_send_ms: f32, update_cpu_ms: f32) {
        let timestamp_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(
            self.writer,
            "{timestamp_ms:.3},{:.6},{:.6},{:.6},{:.6}",
            stable_dt * 1000.0,
            ui_render_ms,
            dmx_send_ms,
            update_cpu_ms
        );
        self.records += 1;
        if self.records % 64 == 0 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }
}

impl Drop for PerfLogger {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Continuous-repaint cadence while the output animates (see the scheduling
/// block in `update`). Defaults to 33ms (~30fps): the UI has no fast animation
/// (only sliders, PDF reading, colour pickers), so ~30fps keeps a show running
/// at a fraction of the CPU cost of 60fps while still looking smooth. The DMX
/// hardware senders (USB Pro / Open DMX / Art-Net) run on their own threads at
/// their own rates (40/30/40Hz), so this only bounds how often the *values*
/// are refreshed — fades get ~30 distinct steps/second, which is visually fine.
/// `EASYCUE_REPAINT_MS` overrides (e.g. 20 for 60fps on fast machines, 9 for a
/// 120Hz panel).
fn repaint_cadence() -> std::time::Duration {
    let ms = std::env::var("EASYCUE_REPAINT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(33);
    std::time::Duration::from_millis(ms.clamp(1, 1000))
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
    /// Persisted submaster snapshots.
    pub submasters: Vec<crate::submasters::Submaster>,
    /// Ephemeral submaster panel state (not saved).
    pub submaster_state: SubmasterPanelState,
    /// Script viewer: persisted annotations + runtime PDF/texture state.
    pub script_viewer: crate::scriptviewer::ScriptViewer,
    pub show_title: String,
    pub current_file_path: Option<std::path::PathBuf>,
    pub dock_state: DockState<TabKind>,
    /// Design-mode dock layout (the full workspace), persisted to eframe storage.
    pub design_dock_state: DockState<TabKind>,
    /// Show-mode dock layout (operator-safe subset), persisted to eframe storage.
    pub show_dock_state: DockState<TabKind>,
    /// "Show Mode": only GO / BACK / STOP / goto / cuelist scrolling / script
    /// page-turning are possible; the workspace swaps to `show_dock_state` and
    /// the Cues + Script Viewer toolbars are simplified. Persisted.
    pub show_mode: bool,
    pub cue_colors: CueColorSettings,

    /// Hotkey assignments (Ctrl+0…9 → cue + trigger mode), saved with the show.
    pub hotkeys: crate::hotkeys::HotkeyMap,
    /// Runtime hold/latch engagement state for the hotkeys (never persisted).
    pub hotkey_runtime: crate::hotkeys::HotkeyRuntime,

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

    /// Running remote-control server (None when disabled).
    #[cfg(feature = "remote")]
    pub remote: Option<crate::remote::RemoteServer>,
    /// Remote-control settings (persisted to eframe storage).
    #[cfg(feature = "remote")]
    pub remote_settings: crate::remote::RemoteSettings,
    /// Last remote settings written to persistent storage.
    #[cfg(feature = "remote")]
    last_persisted_remote_settings: crate::remote::RemoteSettings,

    /// Last saved file path to persistent storage (avoid redundant saves).
    last_persisted_file_path: Option<std::path::PathBuf>,
    /// Set when the workspace layout/mode changed in a way that should be
    /// flushed to persistent storage promptly (mode toggle), even if nothing
    /// else is dirty.
    layout_persist_dirty: bool,
    /// Operator-selected DMX backend to restore on the next launch.
    preferred_dmx_backend: PersistedDmxBackend,
    /// Last DMX preference written to persistent storage.
    last_persisted_dmx_backend: PersistedDmxBackend,
    /// Whether a DMX backend preference existed in persistent storage at startup.
    startup_had_saved_dmx_backend: bool,
    /// Script-viewer zoom to restore on launch / show / New Show (persisted).
    script_viewer_zoom: f32,
    /// Script-viewer dark mode (inverted page) to restore on launch (persisted).
    script_viewer_dark_mode: bool,

    /// Result of the most recent "check for updates" call (never persisted).
    pub update_state: crate::update::UpdateCheckState,
    /// Pending background update check, polled non-blockingly each frame.
    update_check_rx: Option<std::sync::mpsc::Receiver<crate::update::UpdateCheckState>>,
    /// When we last checked for updates, persisted to throttle the automatic startup check.
    last_update_check: Option<chrono::DateTime<chrono::Utc>>,

    /// Optional benchmark-only frame timing logger.
    perf_logger: Option<PerfLogger>,

    /// Auto-reconnect state for a lost DMX hardware device (None = not trying).
    /// After the hardware backend reports loss, the app falls back to Virtual
    /// but keeps re-attempting to open the saved device in the background and
    /// swaps back in when it reappears (e.g. after sleep/resume).
    #[cfg(feature = "usb")]
    dmx_reconnect: Option<DmxReconnect>,
}

/// Tracks a timed fade of the sound master, driven by an Adjust cue.
#[cfg(feature = "audio")]
pub struct SoundFadeState {
    pub start_volume: f32,
    pub target_volume: f32,
    pub fade_time: f32,
    pub start: std::time::Instant,
    pub stop_when_complete: bool,
}

/// Auto-reconnect attempt for a lost DMX hardware device.
///
/// When a USB backend reports loss the app falls back to Virtual (so the show
/// keeps "outputting") but keeps retrying the saved hardware config in the
/// background; when the device reappears (sleep/resume, re-plug) the hardware
/// backend is swapped back in automatically.
#[cfg(feature = "usb")]
struct DmxReconnect {
    /// The backend config we're trying to restore (snapshot of the saved
    /// preference at the time of the loss — not dereferenced live, so the
    /// operator's manual choices don't fight the retry).
    target: PersistedDmxBackend,
    /// Receiver for the background open attempt's result.
    rx: std::sync::mpsc::Receiver<anyhow::Result<Box<dyn DmxBackend>>>,
    /// When the next attempt may start (throttle/backoff).
    next_attempt: std::time::Instant,
    /// Consecutive failures, for exponential-ish backoff.
    consecutive_failures: u32,
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

    /// Kicks off a fresh background check for a newer release, ignoring the
    /// daily throttle (used by the manual "Check for Updates" menu action).
    pub fn trigger_update_check(&mut self, ctx: &egui::Context) {
        self.update_state = crate::update::UpdateCheckState::Checking;
        self.update_check_rx = Some(crate::update::spawn_check(ctx.clone()));
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

        // Benchmark override: EASYCUE_PIXELS_PER_POINT scales the whole UI
        // (fewer fragments on a fill-bound GPU). No-op when unset.
        if let Ok(ppp) = std::env::var("EASYCUE_PIXELS_PER_POINT") {
            if let Ok(ppp) = ppp.parse::<f32>() {
                if (0.25..=3.0).contains(&ppp) {
                    ctx.set_pixels_per_point(ppp);
                    log::info!("[perf] EASYCUE_PIXELS_PER_POINT={ppp}");
                }
            }
        }

        ctx.set_style(style);
        log::info!("Applied cobalt dark theme");
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let app_init_start = std::time::Instant::now();
        log::info!("[startup] EasyCueApp::new begin");
        let perf_logger = PerfLogger::from_env();

        let theme_start = std::time::Instant::now();
        Self::configure_cobalt_theme(&cc.egui_ctx);
        log::info!(
            "[startup] Theme configured in {:.2}ms",
            theme_start.elapsed().as_secs_f64() * 1000.0
        );

        let font_start = std::time::Instant::now();
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        log::info!(
            "[startup] Fonts configured in {:.2}ms",
            font_start.elapsed().as_secs_f64() * 1000.0
        );

        let universe_start = std::time::Instant::now();
        // Create 8 universes (1-based IDs 1–8). Only those referenced by patched
        // fixtures will carry any output; the rest stay at zero and cost nothing.
        let universes: Vec<Universe> = (1..=8).map(Universe::new).collect();
        log::info!(
            "[startup] Universes created in {:.2}ms",
            universe_start.elapsed().as_secs_f64() * 1000.0
        );

        let saved_dmx_backend = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, "preferred_dmx_backend"));
        let had_saved_dmx_backend = saved_dmx_backend.is_some();

        let dmx_init_start = std::time::Instant::now();
        let dmx_backend: Box<dyn DmxBackend> = Box::new(VirtualBackend::new(true));
        log::info!(
            "[startup] DMX backend selected in {:.2}ms",
            dmx_init_start.elapsed().as_secs_f64() * 1000.0
        );

        log::info!("EasyCue3 application initialized");
        log::info!("DMX Backend: {}", dmx_backend.name());

        let dock_load_start = std::time::Instant::now();
        // Two persisted dock layouts — design (the full workspace, backwards
        // compatible with the pre-show-mode "dock_state" key) and show (the
        // operator-safe subset). The active one is loaded per the saved mode.
        let design_dock_state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, "dock_state")
                .unwrap_or_else(|| Self::create_default_dock_layout())
        } else {
            Self::create_default_dock_layout()
        };
        let show_dock_state = if let Some(storage) = cc.storage {
            eframe::get_value(storage, "show_dock_state")
                .unwrap_or_else(Self::create_default_show_layout)
        } else {
            Self::create_default_show_layout()
        };
        let show_mode: bool = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, "show_mode"))
            .unwrap_or(false);
        let dock_state = if show_mode {
            show_dock_state.clone()
        } else {
            design_dock_state.clone()
        };
        log::info!(
            "[startup] Dock layout restored in {:.2}ms (show_mode={})",
            dock_load_start.elapsed().as_secs_f64() * 1000.0,
            show_mode
        );

        #[cfg(feature = "audio")]
        let (audio_player, audio_playback) = {
            let audio_init_start = std::time::Instant::now();
            log::info!("[startup][audio] Initializing audio subsystem");
            let mut player = AudioPlayer::new().unwrap_or_else(|e| {
                panic!("Could not open default audio output: {}", e);
            });
            player.open_all_outputs();
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

        #[cfg(feature = "remote")]
        let remote_settings: crate::remote::RemoteSettings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, "remote_settings"))
            .unwrap_or_default();

        let last_update_check: Option<chrono::DateTime<chrono::Utc>> = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, "last_update_check"));

        // Script-viewer zoom persists across launches as a UI preference.
        // Clamped to the panel's zoom range so a hand-edited storage value can't
        // produce a degenerate view (see MIN_ZOOM / MAX_ZOOM in ui/script_viewer.rs).
        let script_viewer_zoom: f32 = cc
            .storage
            .and_then(|storage| eframe::get_value::<f32>(storage, "script_viewer_zoom"))
            .unwrap_or(1.0)
            .clamp(0.05, 8.0);
        let script_viewer_dark_mode: bool = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "script_viewer_dark_mode"))
            .unwrap_or(false);

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
            submasters: Vec::new(),
            submaster_state: SubmasterPanelState::default(),
            script_viewer: crate::scriptviewer::ScriptViewer::default(),
            show_title: "Example Show".to_string(),
            current_file_path: None,
            dock_state,
            design_dock_state,
            show_dock_state,
            show_mode,
            cue_colors: CueColorSettings::default(),
            hotkeys: crate::hotkeys::HotkeyMap::default(),
            hotkey_runtime: crate::hotkeys::HotkeyRuntime::default(),
            audio_player,
            audio_playback,
            autofollow_timer: None,
            #[cfg(feature = "audio")]
            sound_fade: None,
            #[cfg(feature = "remote")]
            remote: None,
            #[cfg(feature = "remote")]
            remote_settings: remote_settings.clone(),
            #[cfg(feature = "remote")]
            last_persisted_remote_settings: remote_settings,
            last_persisted_file_path: None,
            layout_persist_dirty: false,
            preferred_dmx_backend: saved_dmx_backend.clone().unwrap_or_default(),
            last_persisted_dmx_backend: saved_dmx_backend.unwrap_or_default(),
            startup_had_saved_dmx_backend: had_saved_dmx_backend,
            script_viewer_zoom,
            script_viewer_dark_mode,
            update_state: crate::update::UpdateCheckState::Unknown,
            update_check_rx: None,
            last_update_check,
            perf_logger,
            #[cfg(feature = "usb")]
            dmx_reconnect: None,
        };
        app.script_viewer.zoom = script_viewer_zoom;
        app.script_viewer.dark_mode = script_viewer_dark_mode;

        app.restore_startup_dmx_backend();

        // Auto-check for updates at most once per day, fully in the background.
        let should_auto_check_updates = app
            .last_update_check
            .map(|last| chrono::Utc::now() - last > chrono::Duration::hours(24))
            .unwrap_or(true);
        if should_auto_check_updates {
            app.update_state = crate::update::UpdateCheckState::Checking;
            app.update_check_rx = Some(crate::update::spawn_check(cc.egui_ctx.clone()));
        }

        #[cfg(feature = "remote")]
        {
            // Automation override: EASYCUE3_REMOTE=<port>[:<pin>] force-enables the
            // remote server for this run only (not persisted; port 0 = ephemeral).
            if let Ok(spec) = std::env::var("EASYCUE3_REMOTE") {
                let (port_str, pin) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
                match port_str.parse::<u16>() {
                    Ok(port) => {
                        app.remote_settings = crate::remote::RemoteSettings {
                            enabled: true,
                            port,
                            pin: pin.to_string(),
                        };
                        app.last_persisted_remote_settings = app.remote_settings.clone();
                    }
                    Err(_) => log::warn!("EASYCUE3_REMOTE: invalid port in '{}'", spec),
                }
            }
            if app.remote_settings.enabled {
                app.apply_remote_settings(&cc.egui_ctx);
            }
        }

        let startup_show_load_start = std::time::Instant::now();
        let last_file = cc
            .storage
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
            if let Some(default_path) =
                crate::paths::find_resource_file(std::path::Path::new("shows/default_show.json"))
            {
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

        // Benchmark-only hook (no-op when unset): EASYCUE_PERF_LAYOUT=all opens
        // every panel — including the Script Viewer with a real PDF — so the
        // frame-time harness can measure the worst-case multi-view layout.
        if std::env::var("EASYCUE_PERF_LAYOUT").as_deref() == Ok("all") {
            app.dock_state
                .main_surface_mut()
                .push_to_focused_leaf(TabKind::ScriptViewer);
            app.script_viewer.data.pdf_path = Some(std::path::PathBuf::from("lorem.pdf"));
            log::info!("[perf] EASYCUE_PERF_LAYOUT=all: opened Script Viewer (media/lorem.pdf)");
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

    /// The operator-safe layout used in Show Mode: just the Cue list and the
    /// Script Viewer, side by side. Everything else is hidden so a clumsy click
    /// can't land on an editing panel.
    fn create_default_show_layout() -> DockState<TabKind> {
        let mut dock_state = DockState::new(vec![TabKind::Cues]);
        let tree = dock_state.main_surface_mut();
        let _ = tree.split_right(
            egui_dock::NodeIndex::root(),
            0.55,
            vec![TabKind::ScriptViewer],
        );
        dock_state
    }

    pub fn reset_dock_layout(&mut self) {
        self.dock_state = if self.show_mode {
            Self::create_default_show_layout()
        } else {
            Self::create_default_dock_layout()
        };
        log::info!(
            "Reset {} UI layout to default",
            if self.show_mode { "show" } else { "design" }
        );
    }

    /// Toggle between Show Mode (operator-safe) and Design Mode. Stashes the
    /// current workspace layout into the slot it belongs to, then swaps in the
    /// other mode's saved layout. Both layouts persist independently.
    pub fn set_show_mode(&mut self, show: bool) {
        if self.show_mode == show {
            return;
        }
        // Save the layout we're leaving into its own slot before swapping.
        if self.show_mode {
            self.show_dock_state = self.dock_state.clone();
        } else {
            self.design_dock_state = self.dock_state.clone();
        }
        self.show_mode = show;
        self.dock_state = if show {
            self.show_dock_state.clone()
        } else {
            self.design_dock_state.clone()
        };
        // Clear transient UI state that shouldn't leak across modes.
        self.ui_state.command_input.clear();
        self.ui_state.goto_mode = false;
        self.ui_state.script_follow_on_fade_complete = None;
        // Leaving edit mode means the script can't stay in marker-editing mode.
        if show {
            self.script_viewer.edit_mode = false;
            self.submaster_state.edit_mode = false;
        }
        self.layout_persist_dirty = true;
        log::info!("Switched to {} mode", if show { "SHOW" } else { "DESIGN" });
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
                        patch.label,
                        patch.profile_id,
                        patch.universe,
                        patch.start_address
                    ),
                    Err(e) => log::warn!("Failed to load patch '{}': {}", patch.label, e),
                }
            } else {
                log::warn!(
                    "Skipping patch '{}': profile '{}' not found",
                    patch.label,
                    patch.profile_id
                );
            }
        }

        self.effect_engine.clear();
        self.effect_list =
            crate::effects::EffectList::from_parts(show.effects, show.next_effect_id);
        self.ui_state.selected_effect_id = None;
        self.ui_state.cue_props_effect_choice = None;

        self.groups = show.groups;
        self.magic_sheet = show.magic_sheet;
        self.submasters = show.submasters;
        self.submaster_state = SubmasterPanelState::default();
        self.cue_colors = show.cue_colors;
        self.hotkeys = show.hotkeys;
        self.hotkey_runtime = crate::hotkeys::HotkeyRuntime::default();
        // Script viewer: restore persisted annotations. The PDF itself is not
        // loaded here (no egui context for textures) — the panel lazily loads
        // it on first render if `pdf_path` is set and no document is loaded.
        // If the new show references a different script, drop the currently
        // loaded document so the panel re-loads the right one.
        if self.script_viewer.data.pdf_path != show.script_viewer.pdf_path {
            self.script_viewer.reset_runtime();
        }
        self.script_viewer.data = show.script_viewer;
        self.script_viewer.selected_marker = None;
        self.script_viewer.selected_note = None;
        self.script_viewer.pending_add = None;
        self.script_viewer.drag_marker = None;
        self.script_viewer.drag_note = None;
        self.magic_sheet_state = MagicSheetState {
            canvas_offset: egui::Vec2::new(
                self.magic_sheet.canvas_offset[0],
                self.magic_sheet.canvas_offset[1],
            ),
            canvas_zoom: self.magic_sheet.canvas_zoom,
            ..MagicSheetState::default()
        };
        self.show_title = path
            .file_stem()
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
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");

        let mut show = ShowFile::new();
        show.next_cue_id = self.cue_list.next_id();
        show.cues = self.cue_list.cues().to_vec();
        show.patch = self.fixtures.patch_list().patches().to_vec();
        show.groups = self.groups.clone();
        show.magic_sheet = self.magic_sheet.clone();
        show.submasters = self.submasters.clone();
        show.cue_colors = self.cue_colors.clone();
        show.effects = self.effect_list.effects().to_vec();
        show.next_effect_id = self.effect_list.next_id();
        show.script_viewer = self.script_viewer.data.clone();
        show.hotkeys = self.hotkeys.clone();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        show.save(path)?;
        self.current_file_path = Some(path.to_path_buf());
        self.show_title = title.to_string();
        log::info!(
            "Saved show: {} ({} cues, {} fixtures)",
            title,
            show.cues.len(),
            show.patch.len()
        );
        Ok(())
    }

    /// Apply lighting master and blackout before DMX output.
    /// Returns a cloned Vec of universes with masters applied, ready to send.
    pub fn apply_masters(&self, universes: &[Universe]) -> Vec<Universe> {
        universes
            .iter()
            .map(|universe| {
                let mut output = universe.clone();
                if self.ui_state.blackout_active {
                    output.clear();
                    return output;
                }
                if self.ui_state.lighting_master < 1.0 {
                    for ch in 1..=512u16 {
                        if let Ok(value) = universe.get_channel(ch) {
                            if value > 0 {
                                let scaled =
                                    (value as f32 * self.ui_state.lighting_master).round() as u8;
                                let _ = output.set_channel(ch, scaled);
                            }
                        }
                    }
                }
                output
            })
            .collect()
    }

    /// Apply all submasters after effects and before the lighting master. Each
    /// submaster contributes its captured level scaled by its fader, and channel
    /// output uses highest-takes-precedence over the current look.
    pub fn apply_submasters(&self, universes: &mut [Universe]) {
        for submaster in &self.submasters {
            submaster.apply_to(universes);
        }
    }

    /// Capture the clean live cue-stage state into a submaster. Output-only
    /// effects and existing submaster contributions are deliberately excluded.
    pub fn record_submaster(&mut self, index: usize) -> bool {
        let values = crate::submasters::Submaster::capture(&self.universes);
        let Some(submaster) = self.submasters.get_mut(index) else {
            return false;
        };
        submaster.channel_values = values;
        self.ui_state.status_message = format!("Recorded {}", submaster.name);
        true
    }

    pub fn add_submaster(&mut self) {
        let number = self.submasters.len() + 1;
        self.submasters
            .push(crate::submasters::Submaster::new(number));
        self.ui_state.status_message = format!("Added Sub {}", number);
    }

    pub fn switch_to_virtual(&mut self) {
        self.activate_virtual_backend();
        self.preferred_dmx_backend = PersistedDmxBackend::Virtual;
        #[cfg(feature = "usb")]
        self.cancel_dmx_reconnect();
        log::info!("Switched to Virtual DMX backend");
    }

    #[cfg(feature = "usb")]
    pub fn switch_to_enttec(&mut self, port: &str) -> anyhow::Result<()> {
        self.cancel_dmx_reconnect();
        self.activate_virtual_backend();
        let backend = EnttecUsbProBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        self.ui_state.selected_usb_port = port.to_string();
        self.preferred_dmx_backend = PersistedDmxBackend::UsbPro {
            port: port.to_string(),
        };
        log::info!("Switched to Enttec USB Pro at {}", port);
        Ok(())
    }

    #[cfg(feature = "usb")]
    pub fn switch_to_open_dmx(&mut self, port: &str) -> anyhow::Result<()> {
        use crate::dmx::backends::{EnttecOpenDmxBackend, VirtualBackend};
        self.cancel_dmx_reconnect();
        // Drop the current backend before opening the new port. If the current backend
        // is an Open DMX (or Pro), its Drop impl joins the output thread, which releases
        // the serial port FD — otherwise the open() below would fail with EBUSY.
        self.dmx_backend = Box::new(VirtualBackend::default());
        let backend = EnttecOpenDmxBackend::new(port)?;
        self.dmx_backend = Box::new(backend);
        self.ui_state.selected_open_dmx_port = port.to_string();
        self.preferred_dmx_backend = PersistedDmxBackend::OpenDmx {
            port: port.to_string(),
        };
        log::info!("Switched to Enttec Open DMX USB at {}", port);
        Ok(())
    }

    /// Switch to Art-Net UDP output. `target` is the destination IP (or broadcast).
    /// `universe` is the Art-Net universe number (0-based, 0–32767).
    pub fn switch_to_artnet(&mut self, target: &str, universe: u16) -> anyhow::Result<()> {
        use crate::dmx::backends::ArtNetBackend;
        #[cfg(feature = "usb")]
        self.cancel_dmx_reconnect();
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

    #[cfg(feature = "usb")]
    fn cancel_dmx_reconnect(&mut self) {
        if self.dmx_reconnect.is_some() {
            log::info!("DMX reconnect: cancelled by manual backend switch");
        }
        self.dmx_reconnect = None;
    }

    fn activate_virtual_backend(&mut self) {
        self.dmx_backend = Box::new(VirtualBackend::new(true));
    }

    /// Begin auto-reconnect for a lost DMX hardware device: spawn a background
    /// thread that tries to reopen the saved hardware config.  The show keeps
    /// running on Virtual meanwhile; `service_dmx_reconnect` swaps the real
    /// backend back in when the open succeeds.
    #[cfg(feature = "usb")]
    fn begin_dmx_reconnect(&mut self) {
        if self.dmx_reconnect.is_some() {
            return; // already retrying
        }
        let target = self.preferred_dmx_backend.clone();
        let is_hardware = matches!(
            target,
            PersistedDmxBackend::UsbPro { .. }
                | PersistedDmxBackend::OpenDmx { .. }
                | PersistedDmxBackend::ArtNet { .. }
        );
        if !is_hardware {
            return; // nothing to reconnect (Virtual was the chosen backend)
        }

        log::info!("DMX reconnect: background attempt started for {:?}", target);
        self.dmx_reconnect = Some(DmxReconnect {
            rx: Self::spawn_dmx_reconnect_attempt(&target),
            target,
            next_attempt: std::time::Instant::now(),
            consecutive_failures: 0,
        });
    }

    /// Spawn a background thread that tries to open `target` as a DmxBackend,
    /// returning a receiver for its result.  Serial opens can take ~100ms on a
    /// missing device, so this must never run on the UI thread.
    #[cfg(feature = "usb")]
    fn spawn_dmx_reconnect_attempt(
        target: &PersistedDmxBackend,
    ) -> std::sync::mpsc::Receiver<anyhow::Result<Box<dyn DmxBackend>>> {
        use crate::dmx::backends::{ArtNetBackend, EnttecOpenDmxBackend, EnttecUsbProBackend};

        let (tx, rx) = std::sync::mpsc::channel();
        let thread_target = target.clone();
        std::thread::spawn(move || {
            let result: anyhow::Result<Box<dyn DmxBackend>> = match &thread_target {
                PersistedDmxBackend::UsbPro { port } => {
                    EnttecUsbProBackend::new(port).map(|b| Box::new(b) as Box<dyn DmxBackend>)
                }
                PersistedDmxBackend::OpenDmx { port } => {
                    EnttecOpenDmxBackend::new(port).map(|b| Box::new(b) as Box<dyn DmxBackend>)
                }
                PersistedDmxBackend::ArtNet { target, universe } => {
                    ArtNetBackend::new(target, *universe)
                        .map(|b| Box::new(b) as Box<dyn DmxBackend>)
                }
                PersistedDmxBackend::Virtual => anyhow::Result::<Box<dyn DmxBackend>>::Err(
                    anyhow::anyhow!("Virtual has nothing to reconnect"),
                ),
            };
            let _ = tx.send(result);
        });
        rx
    }

    /// Poll the in-flight reconnect attempt and, when the device is back, swap
    /// the hardware backend in place of Virtual.  On failure, schedule another
    /// attempt with backoff.  Called every frame from `update`.
    #[cfg(feature = "usb")]
    fn service_dmx_reconnect(&mut self) {
        let Some(rec) = &mut self.dmx_reconnect else {
            return;
        };

        // Not yet due for a new attempt?
        if std::time::Instant::now() < rec.next_attempt {
            return;
        }

        match rec.rx.try_recv() {
            Ok(Ok(backend)) => {
                let name = backend.name().to_string();
                // Drop the Virtual fallback and install the real hardware
                // backend (its Drop frees the old serial FD before this one
                // becomes active — same ordering as switch_to_open_dmx).
                self.dmx_backend = backend;
                self.dmx_reconnect = None;
                self.ui_state.status_message = format!("✓ DMX device reconnected ({})", name);
                log::info!("DMX reconnect: hardware backend restored ({})", name);
            }
            Ok(Err(e)) => {
                rec.consecutive_failures += 1;
                let backoff = Self::dmx_reconnect_backoff(rec.consecutive_failures);
                log::debug!(
                    "DMX reconnect attempt {} failed ({}); retrying in {:.1}s",
                    rec.consecutive_failures,
                    e,
                    backoff.as_secs_f32()
                );
                // Launch the next attempt in the background so the UI thread
                // never blocks on a slow serial open.
                let next_rx = Self::spawn_dmx_reconnect_attempt(&rec.target);
                rec.rx = next_rx;
                rec.next_attempt = std::time::Instant::now() + backoff;
            }
            Err(_) => {
                // Still in flight — check again shortly.
                rec.next_attempt =
                    std::time::Instant::now() + std::time::Duration::from_millis(250);
            }
        }
    }

    /// Retry backoff after `failures` consecutive reconnect attempts.
    #[cfg(feature = "usb")]
    fn dmx_reconnect_backoff(failures: u32) -> std::time::Duration {
        let secs = 2_u64.saturating_mul(1 << failures.min(5));
        std::time::Duration::from_secs(secs.min(30))
    }

    /// Whether the DMX hardware reconnect loop is currently active.
    #[cfg(feature = "usb")]
    pub fn dmx_reconnecting(&self) -> bool {
        self.dmx_reconnect.is_some()
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
            PersistedDmxBackend::ArtNet { target, universe } => {
                self.switch_to_artnet(target, *universe)
            }
        };

        if let Err(error) = restore_result {
            log::warn!(
                "[startup][dmx] Could not restore saved DMX backend {:?}: {}. Falling back to Virtual DMX",
                self.preferred_dmx_backend,
                error
            );
            self.activate_virtual_backend();
            self.ui_state.status_message =
                format!("Saved DMX device unavailable — using Virtual DMX instead");
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

    /// Start/stop/restart the remote server to match `remote_settings`.
    /// On failure the enabled flag is reset so the UI reflects reality.
    #[cfg(feature = "remote")]
    pub fn apply_remote_settings(&mut self, ctx: &egui::Context) {
        self.remote = None; // dropping the handle stops any running server
        if !self.remote_settings.enabled {
            self.ui_state.status_message = "Remote control stopped".to_string();
            return;
        }
        match crate::remote::RemoteServer::start(
            self.remote_settings.port,
            &self.remote_settings.pin,
            ctx.clone(),
        ) {
            Ok(server) => {
                self.ui_state.status_message =
                    format!("Remote control running on port {}", server.port);
                self.remote = Some(server);
            }
            Err(e) => {
                self.remote_settings.enabled = false;
                self.ui_state.status_message = format!("Remote control failed: {}", e);
                log::error!("[remote] failed to start: {:#}", e);
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
                log::warn!(
                    "Effect fixture #{}: profile '{}' missing — skipping",
                    id,
                    patch.profile_id
                );
                continue;
            };
            let universe_idx = (patch.universe as usize).saturating_sub(1);
            if universe_idx >= self.universes.len() {
                log::warn!(
                    "Effect fixture #{}: universe {} not available — skipping",
                    id,
                    patch.universe
                );
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
                intensity_ch: profile
                    .get_parameter_offset(&FixtureParameter::Intensity)
                    .map(abs),
                color_chs: profile
                    .color_parameters()
                    .iter()
                    .map(|p| abs(p.channel_offset))
                    .collect(),
                rgb_chs,
                pan_ch: profile
                    .get_parameter_offset(&FixtureParameter::Pan)
                    .map(abs),
                tilt_ch: profile
                    .get_parameter_offset(&FixtureParameter::Tilt)
                    .map(abs),
            });
        }
        resolved
    }

    /// Run a cue's effect actions: starts ramp in over the cue's fade-up,
    /// stops ramp out over its fade-down.
    fn execute_effect_actions(
        &mut self,
        actions: &[crate::effects::EffectAction],
        fade_up: f32,
        fade_down: f32,
    ) {
        use crate::effects::EffectAction;
        for action in actions {
            match action {
                EffectAction::Start {
                    effect_id,
                    fixtures,
                } => {
                    if self.effect_list.find(*effect_id).is_none() {
                        log::warn!("Cue references missing effect {} — skipping", effect_id);
                        continue;
                    }
                    let resolved = self.resolve_effect_fixtures(fixtures);
                    self.effect_engine
                        .start(*effect_id, fixtures.clone(), resolved, fade_up);
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
        let running_ids: Vec<u32> = self
            .effect_engine
            .running()
            .iter()
            .map(|r| r.effect_id())
            .collect();
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
            let needs_start = match self
                .effect_engine
                .running()
                .iter()
                .find(|r| r.effect_id() == id)
            {
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
        let Some(next_idx) = self.cue_list.next_any_index() else {
            return false;
        };
        let cue = self.cue_list.get_cue(next_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                self.playback.start(&cue, &self.universes);
                self.execute_effect_actions(&data.effect_actions, data.fade_up, data.fade_down);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => self.audio_playback.start(&cue, &self.audio_player),
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
            self.follow_cue_in_script_view(cue.id);
            self.ui_state.pending_cue_scroll = self.cue_list.next_any_index();
            self.arm_script_follow_on_deck(&cue);
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
        let Some(prev_idx) = self.cue_list.previous_any_index() else {
            return false;
        };
        let cue = self.cue_list.get_cue(prev_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                let tracked = self.cue_list.tracked_state_up_to(prev_idx);
                let fade_time = data.fade_up;
                self.playback
                    .start_to_state(&tracked, fade_time, Some(cue.id), &self.universes);
                self.sync_effects_to_index(prev_idx, fade_time);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => self.audio_playback.start(&cue, &self.audio_player),
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(data) => {
                self.fire_adjust_cue(cue.id, data.clone());
                true
            }
        };
        if fired {
            self.cue_list.set_current_index(Some(prev_idx));
            log::info!("BACK → cue {:.1} '{}'", cue.number, cue.label);
            self.follow_cue_in_script_view(cue.id);
            self.ui_state.pending_cue_scroll = self.cue_list.next_any_index();
            self.arm_script_follow_on_deck(&cue);
        }
        fired
    }

    /// Advance to the next lighting cue and start its fade. Returns true if a cue fired.
    #[allow(dead_code)]
    pub fn go_lighting(&mut self) -> bool {
        let Some(next_idx) = self.cue_list.next_lighting_index() else {
            return false;
        };
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
        let Some(prev_idx) = self.cue_list.previous_lighting_index() else {
            return false;
        };
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
        let Some(next_idx) = self.cue_list.next_audio_index() else {
            return false;
        };
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
        let Some(prev_idx) = self.cue_list.previous_audio_index() else {
            return false;
        };
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
        self._go_to_cue(abs_idx, false)
    }

    /// Make the cue at `abs_idx` the active cue instantly, without a fade.
    pub fn jump_to_cue(&mut self, abs_idx: usize) -> bool {
        self._go_to_cue(abs_idx, true)
    }

    fn _go_to_cue(&mut self, abs_idx: usize, instant: bool) -> bool {
        self.autofollow_timer = None;
        let cue = self.cue_list.get_cue(abs_idx).cloned();
        let Some(cue) = cue else { return false };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                let tracked = self.cue_list.tracked_state_up_to(abs_idx);
                let fade_time = if instant { 0.0 } else { data.fade_up };
                self.playback
                    .start_to_state(&tracked, fade_time, Some(cue.id), &self.universes);
                self.sync_effects_to_index(abs_idx, fade_time);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => self.audio_playback.start(&cue, &self.audio_player),
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
            self.follow_cue_in_script_view(cue.id);
            self.ui_state.pending_cue_scroll = self.cue_list.next_any_index();
            // Instant jumps (record, etc.) don't arm the deferred on-deck follow.
            if !instant {
                self.arm_script_follow_on_deck(&cue);
            }
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
        let idx = self
            .cue_list
            .cues()
            .iter()
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
        self.playback
            .start_fade_to_black(&self.universes, fade_seconds);
        // With the base fading to 0, a running intensity effect would keep
        // flashing 0→size in the black — Cue 0 stops effects with the fade.
        self.effect_engine.stop_all(fade_seconds);
        #[cfg(feature = "audio")]
        self.audio_playback.stop_all();
        self.autofollow_timer = None;
        self.cue_list.set_current_index(None);
        log::info!("Cue 0: blackout ({:.1}s fade)", fade_seconds);
    }

    /// Fire the cue referenced by a script-viewer marker (playback mode).
    /// Same behaviour as firing it from the cue list — a normal GO with fade.
    /// Returns true if the cue was found and fired.
    pub fn fire_cue_by_id(&mut self, cue_id: u32) -> bool {
        if let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == cue_id) {
            self.go_to_cue(idx)
        } else {
            self.ui_state.status_message = format!("Script marker: cue #{} not found", cue_id);
            log::warn!("Script marker references missing cue #{}", cue_id);
            false
        }
    }

    // --- Hotkey playback ---

    /// Fire the cue referenced by a hotkey in Trigger mode: the cue runs with
    /// its normal fade timing exactly as if GO was pressed, but the play head /
    /// on-deck cue is left untouched and no autofollow is armed.
    pub fn hotkey_trigger(&mut self, cue_id: u32) -> bool {
        let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == cue_id) else {
            self.ui_state.status_message = format!("Hotkey: cue #{} not found", cue_id);
            return false;
        };
        let Some(cue) = self.cue_list.get_cue(idx).cloned() else {
            return false;
        };
        let fired = match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                self.playback.start(&cue, &self.universes);
                self.execute_effect_actions(&data.effect_actions, data.fade_up, data.fade_down);
                true
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => self.audio_playback.start(&cue, &self.audio_player),
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(data) => {
                self.fire_adjust_cue(cue.id, data.clone());
                true
            }
        };
        if fired {
            self.follow_cue_in_script_view(cue.id);
            self.ui_state.status_message = format!("Hotkey → Q{:.1} {}", cue.number, cue.label);
            log::info!("Hotkey trigger → cue {:.1} '{}'", cue.number, cue.label);
        }
        fired
    }

    /// Begin a hotkey Hold/Latch engagement for key `key_idx`. Lighting cues
    /// snapshot the current stage so releasing can fade back to it; audio cues
    /// start normally (fading in with their `fade_in`). Adjust cues have no
    /// hold semantics — they just fire once.
    pub fn hotkey_engage(&mut self, key_idx: usize, cue_id: u32) {
        let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == cue_id) else {
            return;
        };
        let Some(cue) = self.cue_list.get_cue(idx).cloned() else {
            return;
        };
        match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                self.hotkey_runtime.engaged[key_idx] = true;
                self.hotkey_runtime.light_before[key_idx] = Some(self.snapshot_universes());
                self.playback.start(&cue, &self.universes);
                self.execute_effect_actions(&data.effect_actions, data.fade_up, data.fade_down);
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.hotkey_runtime.engaged[key_idx] = true;
                self.audio_playback.start(&cue, &self.audio_player);
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(_) => {
                self.hotkey_trigger(cue_id);
                return;
            }
        }
        self.ui_state.status_message =
            format!("Hotkey {} → Q{:.1} {}", key_idx, cue.number, cue.label);
        log::info!(
            "Hotkey {} engage → cue {:.1} '{}'",
            key_idx,
            cue.number,
            cue.label
        );
    }

    /// End a hotkey Hold/Latch engagement for key `key_idx`. A held lighting
    /// cue fades back (with `fade_down`) to the stage state captured on engage;
    /// a held audio cue fades out with its `fade_out` and stops.
    pub fn hotkey_disengage(&mut self, key_idx: usize) {
        if !self.hotkey_runtime.engaged[key_idx] {
            return;
        }
        self.hotkey_runtime.engaged[key_idx] = false;
        let snapshot = self.hotkey_runtime.light_before[key_idx].take();
        let Some(assignment) = self.hotkeys.get(key_idx).copied() else {
            return;
        };
        let Some(idx) = self
            .cue_list
            .cues()
            .iter()
            .position(|c| c.id == assignment.cue_id)
        else {
            return;
        };
        let Some(cue) = self.cue_list.get_cue(idx) else {
            return;
        };
        match &cue.kind {
            crate::cue::CueKind::Lighting(data) => {
                let target = snapshot.unwrap_or_default();
                self.playback
                    .start_to_state(&target, data.fade_down, None, &self.universes);
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Audio(_) => {
                self.audio_playback
                    .stop_cue_with_fade(assignment.cue_id, 0.5);
            }
            #[cfg(feature = "audio")]
            crate::cue::CueKind::Adjust(_) => {}
        }
        self.ui_state.status_message = format!(
            "Hotkey {} release → Q{:.1} {}",
            key_idx, cue.number, cue.label
        );
        log::info!(
            "Hotkey {} disengage → cue {:.1} '{}'",
            key_idx,
            cue.number,
            cue.label
        );
    }

    /// Snapshot every universe as a `universe_key -> value` map (the format
    /// `PlaybackEngine::start_to_state` accepts) for hotkey hold restoration.
    fn snapshot_universes(&self) -> std::collections::HashMap<u16, u8> {
        let mut state = std::collections::HashMap::new();
        for (uni_idx, universe) in self.universes.iter().enumerate() {
            let universe_num = (uni_idx + 1) as u16;
            for ch in 1u16..=512 {
                if let Ok(v) = universe.get_channel(ch) {
                    if v > 0 {
                        state.insert(crate::cue::universe_key(universe_num, ch), v);
                    }
                }
            }
        }
        state
    }

    /// If the fired cue has a script-viewer marker, record it as the pending
    /// focus target. The panel decides whether to actually jump (it skips the
    /// jump if the marker is already visible on screen). Called after any cue
    /// fires (GO / BACK / goto / autofollow), so a running board op always
    /// sees where in the script the show is.
    fn follow_cue_in_script_view(&mut self, cue_id: u32) {
        if !self.script_viewer.is_loaded() {
            return;
        }
        let marker = self
            .script_viewer
            .data
            .markers
            .iter()
            .find(|m| m.cue_id == cue_id)
            .copied();
        if let Some(m) = marker {
            self.script_viewer.pending_focus = Some((m.page_index, m.x, m.y));
            log::debug!(
                "[scriptviewer] cue #{} has marker on page {} — pending focus",
                cue_id,
                m.page_index + 1
            );
        }
    }

    /// Point the script viewer at the on-deck cue's marker page. Called once a
    /// fired cue's fade completes (see [`Self::arm_script_follow_on_deck`]) so
    /// the page always advances to where the show is heading next.
    fn follow_on_deck_cue_in_script_view(&mut self) {
        if !self.script_viewer.is_loaded() {
            return;
        }
        if let Some(idx) = self.cue_list.next_any_index() {
            if let Some(cue) = self.cue_list.get_cue(idx) {
                self.follow_cue_in_script_view(cue.id);
            }
        }
    }

    /// After a cue fires, decide when the script viewer should advance to the
    /// on-deck cue's page:
    ///
    /// - Cues that fade (lighting with `fade_up > 0`) defer the advance until
    ///   their fade actually completes — the script keeps showing where the
    ///   fired cue landed until the fade finishes.
    /// - Instant / audio / adjust cues advance immediately (there's no fade to
    ///   wait for).
    ///
    /// Must be called *after* the playback engine has started the cue's fade, so
    /// `current_fade_id()` refers to the fade we just began.
    fn arm_script_follow_on_deck(&mut self, cue: &crate::cue::Cue) {
        let fades = cue
            .lighting_data()
            .map(|d| d.fade_up > 0.0)
            .unwrap_or(false);
        if fades {
            self.ui_state.script_follow_on_fade_complete = Some(self.playback.current_fade_id());
        } else {
            self.follow_on_deck_cue_in_script_view();
        }
    }

    /// Create a new cue inline from the script viewer's add-cue popup and add it
    /// to the cue list. Returns the new cue's stable ID, or None on failure.
    /// The created cue is numbered by its position on the script page (between
    /// the bracketing markers — see [`Self::script_insert_number`]) and follows
    /// the same auto-targeting conventions as the equivalent buttons in the
    /// Cues panel.
    pub fn add_cue_of_kind(
        &mut self,
        kind: crate::scriptviewer::NewCueKind,
        page_index: usize,
        y: f32,
    ) -> Option<u32> {
        let next_number = self.script_insert_number(page_index, y);
        let id = self.cue_list.next_id();

        match kind {
            crate::scriptviewer::NewCueKind::Note => {
                // Notes aren't cues — created directly by the script viewer popup.
                return None;
            }
            crate::scriptviewer::NewCueKind::Lighting => {
                let mut cue = Cue::new_lighting(next_number);
                cue.label = format!("Cue {:.1}", next_number);
                self.cue_list.add_cue(cue);
            }
            #[cfg(feature = "audio")]
            crate::scriptviewer::NewCueKind::Sound => {
                let mut cue = Cue::new_audio(next_number, std::path::PathBuf::new());
                cue.label = format!("Sound {:.1}", next_number);
                self.cue_list.add_cue(cue);
            }
            #[cfg(feature = "audio")]
            crate::scriptviewer::NewCueKind::Adjustment => {
                // Auto-target the most recent audio cue, matching the Cues panel.
                let prev_audio_num: Option<f32> = self
                    .cue_list
                    .cues()
                    .iter()
                    .rev()
                    .find_map(|c| c.audio_data().map(|_| c.number));
                let mut cue = Cue::new_adjust(next_number);
                if let crate::cue::CueKind::Adjust(ref mut d) = cue.kind {
                    d.target_audio_cue = prev_audio_num;
                }
                cue.label = format!("Adjust {:.1}", next_number);
                self.cue_list.add_cue(cue);
            }
            #[cfg(not(feature = "audio"))]
            crate::scriptviewer::NewCueKind::Sound
            | crate::scriptviewer::NewCueKind::Adjustment => {
                return None;
            }
        }

        #[cfg(feature = "audio")]
        self.ui_state.audio_file_cache.clear();
        self.ui_state.status_message = format!("Added cue {:.1}", next_number);
        Some(id)
    }

    /// Execute an Adjust cue: fade per-device volume/pan on the targeted audio stream.
    /// `target_audio_cue = None` targets all playing streams.
    #[cfg(feature = "audio")]
    fn fire_adjust_cue(&mut self, _adjust_cue_id: u32, data: crate::cue::AdjustData) {
        let target_id: u32 = if let Some(target_num) = data.target_audio_cue {
            self.cue_list
                .cues()
                .iter()
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
                fade.channel_offset,
                fade.target_volume,
                fade.target_pan,
                data.fade_time,
                data.stop_when_complete,
                &self.audio_player,
            );
        }

        log::info!(
            "Adjust: {} fade(s) on {} over {:.1}s{}",
            data.output_fades.len(),
            data.target_audio_cue
                .map(|n| format!("Q{:.1}", n))
                .unwrap_or_else(|| "all".into()),
            data.fade_time,
            if data.stop_when_complete {
                " then stop"
            } else {
                ""
            },
        );
    }

    /// Index of the cue that new cues should be numbered relative to: the
    /// selected cue (takes precedence) or the active play-head cue. `None` when
    /// neither exists.
    fn insert_anchor_index(&self) -> Option<usize> {
        if let Some(id) = self.ui_state.selected_cue_id {
            if let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == id) {
                return Some(idx);
            }
        }
        self.cue_list.current_index()
    }

    /// Number for the next cue added from the Cues panel: the midpoint between
    /// the selected/active cue and the cue after it (so it slots in close to
    /// where the operator is working), or the end-of-list default.
    pub fn next_cue_insert_number(&self) -> f32 {
        match self.insert_anchor_index() {
            Some(i) => self.cue_list.number_for_insert_after(i),
            None => self.cue_list.end_insert_number(),
        }
    }

    /// Number for a cue created from the script viewer at the given page
    /// position. Brackets the position among all markers (ordered by page,
    /// then y — top of page first) and takes the midpoint of the two
    /// neighbouring cues' numbers, so a cue dropped between two script cues
    /// slots between them. Falls back to the end-of-list default when the
    /// position isn't bracketed by markers (or they reference missing cues).
    fn script_insert_number(&self, page_index: usize, y: f32) -> f32 {
        use crate::scriptviewer::CueMarker;
        let mut markers: Vec<&CueMarker> = self.script_viewer.data.markers.iter().collect();
        markers.sort_by(|a, b| {
            (a.page_index, a.y)
                .partial_cmp(&(b.page_index, b.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let click = (page_index, y);
        let before = markers.iter().rev().find(|m| (m.page_index, m.y) <= click);
        let after = markers.iter().find(|m| click < (m.page_index, m.y));
        match (before, after) {
            (Some(b), Some(a)) => {
                let nums: Vec<f32> = [b.cue_id, a.cue_id]
                    .iter()
                    .filter_map(|id| {
                        self.cue_list
                            .cues()
                            .iter()
                            .find(|c| c.id == *id)
                            .map(|c| c.number)
                    })
                    .collect();
                match nums.as_slice() {
                    [bn, an] if (*an - *bn).abs() > 0.005 => {
                        crate::cue::list::midpoint_insert_number(*bn, *an)
                    }
                    _ => self.cue_list.end_insert_number(),
                }
            }
            _ => self.cue_list.end_insert_number(),
        }
    }

    /// Record a new lighting cue from the current universe state.
    /// Returns the stable ID assigned to the new cue.
    pub fn record_cue(&mut self) -> u32 {
        let anchor_idx = self.insert_anchor_index();
        let next_number = match anchor_idx {
            Some(i) => self.cue_list.number_for_insert_after(i),
            None => self.cue_list.end_insert_number(),
        };

        let mut cue = Cue::new_lighting(next_number);
        cue.label = format!("Cue {:.1}", next_number);

        // The ID that will be assigned by add_cue (cue.id is 0 → next_id is used)
        let assigned_id = self.cue_list.next_id();

        // Tracking mode: only record channels that differ from the accumulated state
        // of all existing cues. A channel going to 0 that was non-zero must be stored
        // explicitly so the next cue knows to fade it out. The baseline is the state
        // at the insertion point (right after the anchor) — or the full end-of-list
        // state when appending at the end.
        let tracked = match anchor_idx {
            Some(i) => self.cue_list.tracked_state_up_to(i),
            None => {
                if self.cue_list.is_empty() {
                    std::collections::HashMap::new()
                } else {
                    self.cue_list.tracked_state_up_to(self.cue_list.len() - 1)
                }
            }
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

        let channel_count = cue
            .lighting_data()
            .map(|d| d.channel_values.len())
            .unwrap_or(0);
        self.cue_list.add_cue(cue);
        self.ui_state.status_message = format!("Recorded cue {:.1}", next_number);
        log::info!(
            "Recorded cue {:.1} with {} channels",
            next_number,
            channel_count
        );
        assigned_id
    }

    /// Duplicate the cue with the given stable ID. The copy is inserted right
    /// after the original in the list, numbered between the original and the
    /// next cue (or the next whole number at the end of the list). Returns the
    /// new cue's stable ID, or None if `id` isn't a cue in the list.
    pub fn duplicate_cue(&mut self, id: u32) -> Option<u32> {
        let anchor_idx = self.cue_list.cues().iter().position(|c| c.id == id)?;
        let number = self.cue_list.number_for_insert_after(anchor_idx);
        let mut dup = self.cue_list.get_cue(anchor_idx).cloned()?;
        dup.id = 0;
        dup.number = number;
        let new_id = self.cue_list.next_id();
        self.cue_list.add_cue(dup);
        #[cfg(feature = "audio")]
        self.ui_state.audio_file_cache.clear();
        log::info!("Duplicated cue id {} as {:.1} (id {})", id, number, new_id);
        Some(new_id)
    }

    /// Select a cue by stable ID and keep the legacy per-kind selection fields in sync.
    pub fn select_cue(&mut self, id: u32) {
        self.ui_state.selected_cue_id = Some(id);
        let is_lx = self
            .cue_list
            .find_by_id(id)
            .map(|c| c.is_lighting())
            .unwrap_or(false);
        if is_lx {
            self.ui_state.selected_lighting_cue_id = Some(id);
            self.ui_state.selected_audio_cue_id = None;
        } else {
            self.ui_state.selected_audio_cue_id = Some(id);
            self.ui_state.selected_lighting_cue_id = None;
        }
        // Keep the script viewer in step: selecting a cue anywhere also brings
        // its marker into view (no-op when it has no marker, or when it's
        // already visible on screen).
        self.follow_cue_in_script_view(id);
    }

    /// Open the Re-number Cues dialog, defaulting its fields from the current
    /// cue list (whole-list scope, start at the first cue's number, step 1.0).
    pub fn open_renumber_dialog(&mut self) {
        let Some((first, last)) = self
            .cue_list
            .cues()
            .first()
            .zip(self.cue_list.cues().last())
            .map(|(a, b)| (a.number, b.number))
        else {
            self.ui_state.status_message = "No cues to renumber".to_string();
            return;
        };
        self.ui_state.renumber_all = true;
        self.ui_state.renumber_from = first;
        self.ui_state.renumber_to = last;
        self.ui_state.renumber_start = first;
        self.ui_state.renumber_step = 1.0;
        self.ui_state.renumber_focus_pending = true;
        self.ui_state.show_renumber_cues = true;
    }

    /// Apply the Re-number Cues dialog settings to the cue list. Keeps the
    /// dialog open (and reports the error) when the renumber is rejected so the
    /// operator can correct the parameters.
    pub fn apply_renumber_cues(&mut self) {
        let (from, to, start, step, all) = {
            let st = &mut self.ui_state;
            st.show_renumber_cues = false;
            (
                st.renumber_from,
                st.renumber_to,
                st.renumber_start,
                st.renumber_step,
                st.renumber_all,
            )
        };
        let result = if all {
            self.cue_list
                .renumber_range(0, self.cue_list.len().saturating_sub(1), start, step)
        } else {
            self.cue_list
                .renumber_range_for_numbers(from, to, start, step)
        };
        match result {
            Ok(n) => {
                self.ui_state.status_message = format!(
                    "Renumbered {} cue(s) starting at {:.1} step {:.1}",
                    n, start, step
                );
            }
            Err(e) => {
                self.ui_state.status_message = format!("Renumber failed: {}", e);
                self.ui_state.show_renumber_cues = true;
            }
        }
    }

    /// Overwrite the lighting data of the cue at `upd_idx` with the current live
    /// universe levels. Tracking-aware: an absolute cue captures the full state
    /// of every patched channel; a tracking cue stores only the channels that
    /// differ from the tracked state before it. Returns true if a lighting cue was updated.
    pub fn capture_stage_to_cue(&mut self, upd_idx: usize) -> bool {
        let (is_absolute, upd_number) = match self.cue_list.get_cue(upd_idx) {
            Some(c) if c.is_lighting() => (
                c.lighting_data().map(|d| d.absolute).unwrap_or(false),
                c.number,
            ),
            _ => return false,
        };

        // Absolute: snapshot every patched channel; channels at 0 are omitted
        // (absence from an absolute cue means off).
        if is_absolute {
            let patched = self.patched_channel_keys();
            let mut values: HashMap<u16, u8> = HashMap::new();
            for (uni_idx, universe) in self.universes.iter().enumerate() {
                let universe_num = (uni_idx + 1) as u16;
                for ch in 1u16..=512 {
                    let key = crate::cue::universe_key(universe_num, ch);
                    if !patched.contains(&key) {
                        continue;
                    }
                    if let Ok(live_val) = universe.get_channel(ch) {
                        if live_val > 0 {
                            values.insert(key, live_val);
                        }
                    }
                }
            }
            if let Some(c) = self.cue_list.get_cue_mut(upd_idx) {
                if let Some(d) = c.lighting_data_mut() {
                    d.channel_values = values;
                }
            }
            self.ui_state.status_message = format!("Captured stage to cue {:.1}", upd_number);
            return true;
        }

        let prev_tracked = if upd_idx > 0 {
            self.cue_list.tracked_state_up_to(upd_idx - 1)
        } else {
            std::collections::HashMap::new()
        };
        let mut deltas: Vec<(u16, u8)> = Vec::new();
        for (uni_idx, universe) in self.universes.iter().enumerate() {
            let universe_num = (uni_idx + 1) as u16;
            for ch in 1u16..=512 {
                if let Ok(live_val) = universe.get_channel(ch) {
                    let key = crate::cue::universe_key(universe_num, ch);
                    let tracked_val = prev_tracked.get(&key).copied().unwrap_or(0);
                    if live_val != tracked_val {
                        deltas.push((key, live_val));
                    }
                }
            }
        }
        if let Some(c) = self.cue_list.get_cue_mut(upd_idx) {
            if let Some(d) = c.lighting_data_mut() {
                d.channel_values.clear();
                for (key, val) in deltas {
                    d.channel_values.insert(key, val);
                }
            }
        }
        self.ui_state.status_message = format!("Captured stage to cue {:.1}", upd_number);
        true
    }

    /// Keys (`universe_key`) of every DMX channel that belongs to a patched
    /// fixture. Absolute cues snapshot only these channels.
    fn patched_channel_keys(&self) -> HashSet<u16> {
        let counts = self.fixtures.get_channel_counts();
        let mut keys = HashSet::new();
        for patch in self.fixtures.patch_list().patches() {
            let count = counts.get(&patch.profile_id).copied().unwrap_or(1);
            let end = patch.start_address.saturating_add(count).min(513);
            for ch in patch.start_address..end {
                keys.insert(crate::cue::universe_key(patch.universe, ch));
            }
        }
        keys
    }

    /// Convert the lighting cue at `idx` to/from absolute mode, expanding or
    /// collapsing its channel data so the flag change is correct.
    ///
    /// * To absolute: `channel_values` becomes the full tracked state at `idx`,
    ///   restricted to patched channels (non-zero only; absence means 0).
    /// * To tracking: `channel_values` is reduced to the patched channels that
    ///   were non-zero and differ from the tracked state *before* the cue.
    ///   Channels at 0 in the snapshot are dropped (as if they'd never been
    ///   stored) — so lights that were off but on earlier can track back up;
    ///   that's the designer's call to catch at the time.
    pub fn set_cue_absolute(&mut self, idx: usize, absolute: bool) {
        let current_absolute = self
            .cue_list
            .get_cue(idx)
            .and_then(|c| c.lighting_data())
            .map(|d| d.absolute)
            .unwrap_or(false);
        if current_absolute == absolute {
            return;
        }

        let full = self.cue_list.tracked_state_up_to(idx);
        let prev = if idx > 0 {
            self.cue_list.tracked_state_up_to(idx - 1)
        } else {
            HashMap::new()
        };
        let patched = self.patched_channel_keys();

        let values: HashMap<u16, u8> = if absolute {
            full.iter()
                .filter(|(&k, &v)| patched.contains(&k) && v > 0)
                .map(|(&k, &v)| (k, v))
                .collect()
        } else {
            full.iter()
                .filter(|(&k, _)| patched.contains(&k))
                .filter(|(&k, &v)| v != prev.get(&k).copied().unwrap_or(0))
                .map(|(&k, &v)| (k, v))
                .collect()
        };

        if let Some(c) = self.cue_list.get_cue_mut(idx) {
            if let Some(d) = c.lighting_data_mut() {
                d.channel_values = values;
                d.absolute = absolute;
            }
        }
    }

    /// Set the fade-up and fade-down times (seconds, clamped 0–30) of a lighting cue.
    pub fn set_lighting_fade_times(&mut self, id: u32, secs: f32) {
        let secs = secs.clamp(0.0, 30.0);
        if let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == id) {
            if let Some(c) = self.cue_list.get_cue_mut(idx) {
                if let Some(d) = c.lighting_data_mut() {
                    d.fade_up = secs;
                    d.fade_down = secs;
                }
            }
        }
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
    fn check_autosave_recovery(
        loaded_path: Option<&std::path::Path>,
    ) -> Option<std::path::PathBuf> {
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
        match (
            ShowFile::load(&autosave_path),
            loaded_path.and_then(|p| ShowFile::load(p).ok()),
        ) {
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

/// Ordered shutdown body shared by the feature-gated `eframe::App::on_exit`
/// implementations (glow and wgpu builds have different trait signatures).
impl EasyCueApp {
    fn shutdown_sequence(&mut self) {
        let shutdown_start = std::time::Instant::now();
        log::warn!("[shutdown] on_exit invoked; beginning shutdown sequence");
        if let Some(perf_logger) = &mut self.perf_logger {
            perf_logger.flush();
        }

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

        log::info!(
            "[shutdown] Shutdown sequence complete in {:.2}ms total",
            shutdown_start.elapsed().as_secs_f64() * 1000.0
        );

        // NOTE: we deliberately do NOT `std::process::exit` here. eframe's quit
        // path flushes its persisted settings (app.ron) on a background thread and
        // only joins that thread when its FileStorage is dropped, which happens
        // after `on_exit` returns. Calling `exit` here used to race that write and
        // occasionally leave a truncated settings file — which on the next launch
        // silently reset the UI layout and the last-loaded show. Returning lets
        // eframe finish the write, join the thread, and exit the process itself.
    }
}

impl eframe::App for EasyCueApp {
    /// Ordered shutdown: stop audio, auto-save the show, close the DMX backend.
    /// Returns normally so eframe can finish flushing its persisted settings and
    /// tear down the event loop — a forced `process::exit` here raced the
    /// settings write and occasionally corrupted `app.ron`.
    fn on_exit(&mut self) {
        self.shutdown_sequence();
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_cpu_start = std::time::Instant::now();
        if !self.ui_state.theme_initialized {
            Self::configure_cobalt_theme(ctx);
            self.ui_state.theme_initialized = true;
            log::info!("Theme reapplied in update()");
        }

        // Non-blocking poll for a background update-check result, if one is in flight.
        if let Some(rx) = &self.update_check_rx {
            if let Ok(state) = rx.try_recv() {
                self.update_state = state;
                self.last_update_check = Some(chrono::Utc::now());
                self.update_check_rx = None;
            }
        }

        // Suppress hotkeys while any text field has focus, a dropdown/menu popup
        // is open (combo boxes, context menus, colour pickers), or the script
        // viewer's add-cue popup is showing — so e.g. arrow keys navigate that
        // control instead of leaking out and changing the on-deck cue. Ctrl+R
        // (record) is safe to allow regardless.
        let keyboard_busy = ctx.memory(|m| m.focused().is_some())
            || ctx.memory(|m| m.any_popup_open())
            || self.script_viewer.pending_add.is_some();
        let (go, back, stop, record, ctrl_g, escape, arrow_up, arrow_down, update) =
            ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Space) && !i.modifiers.any() && !keyboard_busy,
                    // Shift+Space = BACK (the "reverse GO"). Deliberately Shift and
                    // not Ctrl: Ctrl+Space is grabbed by OS input-method toggles.
                    i.key_pressed(egui::Key::Space)
                        && i.modifiers.shift
                        && !i.modifiers.command
                        && !i.modifiers.ctrl
                        && !i.modifiers.alt
                        && !keyboard_busy,
                    i.key_pressed(egui::Key::S) && !i.modifiers.any() && !keyboard_busy,
                    i.key_pressed(egui::Key::R) && i.modifiers.ctrl,
                    i.key_pressed(egui::Key::G) && i.modifiers.ctrl && !keyboard_busy,
                    // Escape is a safety/pause key — works even when a text field has focus.
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::ArrowUp) && !keyboard_busy,
                    i.key_pressed(egui::Key::ArrowDown) && !keyboard_busy,
                    i.key_pressed(egui::Key::U) && i.modifiers.ctrl && !keyboard_busy,
                )
            });

        if stop {
            self.playback.stop();
            #[cfg(feature = "audio")]
            self.audio_playback.stop_all();
            self.autofollow_timer = None;
        }
        if back {
            if self.go_back() {
                self.ui_state.status_message = "BACK".to_string();
            }
        }
        if record {
            let id = self.record_cue();
            // Jump into the new cue as the actively running cue.
            if let Some(idx) = self.cue_list.cues().iter().position(|c| c.id == id) {
                self.jump_to_cue(idx);
            }
            // go_to_cue clears the selection; re-select so the properties panel shows it.
            self.select_cue(id);
            // There is no cue after the newest one — blank the on-deck box.
            self.ui_state.go_cue_input.clear();
            // Activate the new cue's label field so the operator can rename it.
            self.ui_state.focus_cue_edit = Some((id, CueEditField::Label));
        }
        // Ctrl+U: prompt to update the currently active lighting cue with the live
        // stage levels (same confirmation modal as the "Update from Stage" button).
        if update {
            if let Some(id) = self.playback.current_cue_id() {
                self.ui_state.pending_update_cue_id = Some(id);
            }
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
                let current_sel = self
                    .ui_state
                    .selected_cue_id
                    .and_then(|id| self.cue_list.cues().iter().position(|c| c.id == id))
                    .or_else(|| self.cue_list.next_any_index());
                let new_idx = if arrow_up {
                    current_sel.map(|i| i.saturating_sub(1)).unwrap_or(0)
                } else {
                    current_sel.map(|i| (i + 1).min(cue_count - 1)).unwrap_or(0)
                };
                if let Some(cue) = self.cue_list.get_cue(new_idx) {
                    let num = cue.number;
                    let id = cue.id;
                    // Move play head to just before this cue so next_any_index() points here.
                    let prev_idx = if new_idx > 0 { Some(new_idx - 1) } else { None };
                    self.cue_list.set_current_index(prev_idx);
                    self.select_cue(id);
                    self.ui_state.go_cue_input = format!("{:.1}", num);
                    // Keep the on-deck row in view as the operator arrow-scrolls.
                    self.ui_state.pending_cue_scroll = Some(new_idx);
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
                        self.cue_list
                            .cues()
                            .iter()
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

        // Hotkeys (Ctrl+0…Ctrl+9): fire assigned cues with Trigger / Hold / Latch
        // semantics. Edge-detected against the previous frame's *key state* (not
        // press/release events) so a release that happens while the keyboard is
        // busy (text field focused) is still caught on the next idle frame, and
        // OS key auto-repeat can't re-fire a hold or double-toggle a latch.
        if !keyboard_busy && !self.ui_state.goto_mode {
            let down_now: Vec<bool> = ctx.input(|i| {
                let ctrl = i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt;
                (0..10).map(|d| ctrl && i.key_down(digit_key(d))).collect()
            });
            for (d, now_down) in down_now.into_iter().enumerate() {
                let was_down = self.hotkey_runtime.key_down[d];
                self.hotkey_runtime.key_down[d] = now_down;
                let Some(assignment) = self.hotkeys.get(d).copied() else {
                    continue;
                };
                if assignment.cue_id == 0 {
                    continue;
                }
                let rising = now_down && !was_down;
                let falling = !now_down && was_down;
                if rising {
                    match assignment.mode {
                        crate::hotkeys::HotkeyMode::Trigger => {
                            self.hotkey_trigger(assignment.cue_id);
                        }
                        crate::hotkeys::HotkeyMode::Hold => {
                            self.hotkey_engage(d, assignment.cue_id);
                        }
                        crate::hotkeys::HotkeyMode::Latch => {
                            if self.hotkey_runtime.engaged[d] {
                                self.hotkey_disengage(d);
                            } else {
                                self.hotkey_engage(d, assignment.cue_id);
                            }
                        }
                    }
                } else if falling && assignment.mode == crate::hotkeys::HotkeyMode::Hold {
                    self.hotkey_disengage(d);
                }
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

        // Script viewer "advance to on-deck" follow-up: when a fading cue fires
        // we arm `script_follow_on_fade_complete` with that fade's id. Consume it
        // here as soon as the matching fade actually completes, so the script
        // page moves to where the show is heading next. A completion with a
        // different id (blackout, a frozen fade that was superseded) just clears
        // the arm without jumping.
        if let Some(completed) = self.playback.take_completed_fade() {
            if let Some(expected) = self.ui_state.script_follow_on_fade_complete.take() {
                if completed == expected {
                    self.follow_on_deck_cue_in_script_view();
                }
            }
        }

        // Keep VirtualIntensity state in sync with whatever the playback engine wrote to the
        // universes this frame, so that intensity reads in the UI panels are never stale.
        if self.playback.is_playing() {
            let patches: Vec<_> = self.fixtures.patch_list().patches().to_vec();
            for patch in &patches {
                let uni_idx = (patch.universe as usize).saturating_sub(1);
                if let Some(universe) = self.universes.get(uni_idx) {
                    if let Some(profile) = self.fixtures.get_profile(&patch.profile_id) {
                        if !profile.has_intensity() {
                            self.virtual_intensity
                                .update_from_universe(patch.id, universe, patch, profile);
                        }
                    }
                }
            }
        }

        #[cfg(feature = "audio")]
        {
            // Recover outputs whose stream died (e.g. system sleep). Re-opens the
            // device and re-attaches any active cues routed to it. Throttled
            // internally, so it's cheap to call every frame.
            let recovered = self.audio_player.recover_dead_outputs();
            for name in &recovered {
                self.audio_playback.recover_device(name, &self.audio_player);
            }
            self.audio_playback.update(self.ui_state.sound_master);
        }

        // Remote control: execute queued phone commands and publish state diffs.
        #[cfg(feature = "remote")]
        crate::remote::glue::service_frame(self, ctx);

        // Auto-fallback: if the hardware backend lost the device, switch to Virtual.
        if !self.dmx_backend.is_connected() {
            log::warn!("DMX device lost — falling back to Virtual DMX");
            self.ui_state.status_message = format!(
                "DMX device lost — reconnecting… (was: {})",
                self.dmx_backend.name()
            );
            self.activate_virtual_backend();
            #[cfg(feature = "usb")]
            self.begin_dmx_reconnect();
        }
        // Service any in-flight DMX hardware reconnect attempt (swap the real
        // backend back in when the device reappears).
        #[cfg(feature = "usb")]
        self.service_dmx_reconnect();

        let dmx_send_start = std::time::Instant::now();
        // Effects modulate a clone of the base look at output time only, then
        // masters scale the result — blackout and grand master govern effect
        // output, and the stored universes never see effect values.
        let output_universes = if self.effect_engine.is_active() {
            let mut staged = self.universes.clone();
            let footprint = self.effect_engine.apply(&mut staged, &self.effect_list);
            self.apply_submasters(&mut staged);
            let output = self.apply_masters(&staged);
            // Keep the pre-master staged look for UI readouts (panels always
            // show pre-master values, so FX display matches that convention).
            self.effect_display = Some(crate::effects::EffectDisplay {
                universes: staged,
                footprint,
            });
            output
        } else {
            self.effect_display = None;
            let mut staged = self.universes.clone();
            self.apply_submasters(&mut staged);
            self.apply_masters(&staged)
        };
        if let Err(e) = self.dmx_backend.send_universes(&output_universes) {
            log::error!("DMX output error: {}", e);
        }
        let dmx_send_time = dmx_send_start.elapsed();

        let ui_render_start = std::time::Instant::now();
        crate::ui::render(ctx, self);
        let ui_render_time = ui_render_start.elapsed();

        // PerfLogger is recorded at the end of the frame so it can attribute a
        // frame-time spike to CPU-side UI/DMX work vs. present/wait time.
        if let Some(perf_logger) = &mut self.perf_logger {
            let stable_dt = ctx.input(|input| input.stable_dt);
            perf_logger.record(
                stable_dt,
                ui_render_time.as_secs_f64() as f32 * 1000.0,
                dmx_send_time.as_secs_f64() as f32 * 1000.0,
                frame_cpu_start.elapsed().as_secs_f64() as f32 * 1000.0,
            );
        }

        if self.ui_state.show_debug_ui {
            // Rolling repaint-rate measurement. `stable_dt` is egui's predicted
            // timestep (usually 1/60) even in reactive mode, so it can't show
            // how often we *actually* repaint — count frames over the last second.
            let (repaint_fps, frame_times) = ctx.input(|i| {
                let t = i.time;
                let times = &mut self.ui_state.debug_frame_times;
                times.push_back(t);
                while let Some(&front) = times.front() {
                    if t - front > 1.0 {
                        times.pop_front();
                    } else {
                        break;
                    }
                }
                let fps = match (times.front().copied(), times.back().copied()) {
                    (Some(front), Some(back)) if back > front && times.len() >= 2 => {
                        (times.len() - 1) as f32 / (back - front) as f32
                    }
                    _ => 0.0,
                };
                (fps, times.len())
            });
            let target_ms = repaint_cadence().as_millis();
            egui::Window::new(format!("{} Debug Info", egui_phosphor::regular::BUG))
                .default_pos([10.0, 10.0])
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Repaint rate (1s): {:.1} fps over {} frames",
                        repaint_fps, frame_times
                    ));
                    ui.label(format!(
                        "Repaint target: {:.0}ms ({} fps)",
                        target_ms,
                        1000.0 / target_ms as f32
                    ));
                    ui.label(format!(
                        "stable_dt: {:.2}ms",
                        ctx.input(|i| i.stable_dt * 1000.0)
                    ));
                    ui.label(format!(
                        "unstable_dt: {:.2}ms",
                        ctx.input(|i| i.unstable_dt * 1000.0)
                    ));
                    ui.separator();
                    ui.label(egui::RichText::new("Performance:").strong());
                    ui.label(format!(
                        "  DMX send: {:.2}ms",
                        dmx_send_time.as_secs_f64() * 1000.0
                    ));
                    ui.label(format!(
                        "  UI render: {:.2}ms",
                        ui_render_time.as_secs_f64() * 1000.0
                    ));
                    ui.separator();
                    ui.label(format!("Total cues: {}", self.cue_list.len()));
                    #[cfg(feature = "audio")]
                    {
                        let audio_count =
                            self.cue_list.cues().iter().filter(|c| c.is_audio()).count();
                        let lighting_count = self
                            .cue_list
                            .cues()
                            .iter()
                            .filter(|c| c.is_lighting())
                            .count();
                        ui.label(format!(
                            "  Lighting: {}  Audio: {}",
                            lighting_count, audio_count
                        ));
                        ui.label(format!(
                            "File cache: {} entries",
                            self.ui_state.audio_file_cache.len()
                        ));
                        ui.label(format!(
                            "Audio playing: {}",
                            self.audio_playback.is_playing()
                        ));
                    }
                    ui.label(format!("Lighting playing: {}", self.playback.is_playing()));
                    ui.separator();
                    if ui.button("Clear file cache").clicked() {
                        #[cfg(feature = "audio")]
                        self.ui_state.audio_file_cache.clear();
                    }
                });
        }

        // ── Repaint scheduling ─────────────────────────────────────────────
        // Continuous frames are requested only while something is actually
        // animating: a DMX fade in progress, a running effect, audio playback,
        // a pending autofollow or audio-master ramp, the debug overlay, or the
        // keep-alive cases below. When the output is static the app sleeps and
        // wakes on input — egui repaints on demand, so a static show burns no
        // CPU or GPU.
        //
        // The cadence defaults to 33ms (~30fps) — the UI is mostly static and
        // ~30fps is plenty smooth for sliders/PDFs/colour pickers, at a third
        // of the CPU of 60fps. It also stays above one 60Hz display refresh
        // (16.67ms), so the loop never races ahead of the swapchain and never
        // hits the periodic ~30ms stalls that made 0.8.4 feel like 30-40fps
        // (see docs/PERFORMANCE_AND_BENCHMARKING.md). The DMX/audio output
        // threads are independent and unaffected. EASYCUE_REPAINT_MS overrides.
        let cadence = repaint_cadence();
        let mut next_repaint: Option<std::time::Duration> = None;
        let mut schedule = |d: std::time::Duration| {
            next_repaint = Some(match next_repaint {
                Some(prev) => prev.min(d),
                None => d,
            });
        };
        // Fade in progress: interpolated DMX values change every frame.
        if self.playback.fade_progress().is_some() {
            schedule(cadence);
        }
        // Effects animate the DMX output every frame; without this the app
        // idles and a running effect freezes between input events.
        if self.effect_engine.is_active() {
            schedule(cadence);
        }
        #[cfg(feature = "audio")]
        if self.audio_playback.is_playing() {
            schedule(cadence);
        }
        // A pending autofollow must fire even when the output is static —
        // wake at the remaining delay rather than spinning at the cadence.
        if let Some((start, delay)) = self.autofollow_timer {
            let remaining = (delay - start.elapsed().as_secs_f32()).max(0.001);
            schedule(std::time::Duration::from_secs_f32(remaining));
        }
        #[cfg(feature = "audio")]
        if self.sound_fade.is_some() {
            schedule(cadence);
        }
        // Keep the loop alive while an audio output is dead so the recovery
        // scan re-runs even when no cues are playing or effect is animating.
        // Slower than the playback cadence — recovery is throttled to 2s.
        #[cfg(feature = "audio")]
        if self.audio_player.any_output_failed() {
            schedule(std::time::Duration::from_millis(500));
        }
        // Keep the loop alive while DMX hardware is being reconnected.
        #[cfg(feature = "usb")]
        if self.dmx_reconnecting() {
            schedule(std::time::Duration::from_millis(250));
        }
        if self.ui_state.show_debug_ui {
            schedule(cadence);
        }
        if let Some(delay) = next_repaint {
            ctx.request_repaint_after(delay);
        }

        // Persist file path only if it changed (avoid redundant saves every frame)
        #[cfg(feature = "remote")]
        let remote_settings_dirty = self.remote_settings != self.last_persisted_remote_settings;
        #[cfg(not(feature = "remote"))]
        let remote_settings_dirty = false;
        if self.current_file_path != self.last_persisted_file_path
            || self.preferred_dmx_backend != self.last_persisted_dmx_backend
            || (self.script_viewer.zoom - self.script_viewer_zoom).abs() > 0.001
            || self.script_viewer.dark_mode != self.script_viewer_dark_mode
            || self.layout_persist_dirty
            || remote_settings_dirty
        {
            if let Some(storage) = frame.storage_mut() {
                self.save(storage);
                // Push to disk promptly so a Ctrl+C / crash loses at most the
                // last few frames of UI state, not up to the 30s eframe autosave
                // interval. `flush` only writes when something changed and joins
                // the previous write thread, so writes are serialised.
                storage.flush();
                self.last_persisted_file_path = self.current_file_path.clone();
                self.last_persisted_dmx_backend = self.preferred_dmx_backend.clone();
                self.script_viewer_zoom = self.script_viewer.zoom;
                self.script_viewer_dark_mode = self.script_viewer.dark_mode;
                self.layout_persist_dirty = false;
                #[cfg(feature = "remote")]
                {
                    self.last_persisted_remote_settings = self.remote_settings.clone();
                }
            }
        }
    }

    /// Don't persist egui's internal memory (scroll positions, collapsed states,
    /// transient window sizes). It's the bulk of the app.ron file (~100KB of
    /// ~110KB) and its loss is never missed, but skipping it shrinks the settings
    /// write to a few kilobytes — making it effectively instant and removing the
    /// window in which a killed write could corrupt the file. The app's own
    /// persisted state (dock layout, script zoom, last file, DMX/remote settings)
    /// is unaffected.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Mirror the live workspace into the slot it belongs to so both layouts
        // persist independently (the dock area mutates `dock_state` every frame).
        if self.show_mode {
            self.show_dock_state = self.dock_state.clone();
        } else {
            self.design_dock_state = self.dock_state.clone();
        }
        eframe::set_value(storage, "dock_state", &self.design_dock_state);
        eframe::set_value(storage, "show_dock_state", &self.show_dock_state);
        eframe::set_value(storage, "show_mode", &self.show_mode);
        eframe::set_value(
            storage,
            "preferred_dmx_backend",
            &self.preferred_dmx_backend,
        );
        eframe::set_value(storage, "script_viewer_zoom", &self.script_viewer.zoom);
        eframe::set_value(
            storage,
            "script_viewer_dark_mode",
            &self.script_viewer.dark_mode,
        );
        #[cfg(feature = "remote")]
        eframe::set_value(storage, "remote_settings", &self.remote_settings);
        eframe::set_value(storage, "last_update_check", &self.last_update_check);
        if let Some(path) = &self.current_file_path {
            storage.set_string("last_file", path.to_string_lossy().to_string());
        }
        log::info!("Saved UI layout");
    }
}
