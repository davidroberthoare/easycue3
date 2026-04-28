//! Main application state and logic

use crate::cue::{Cue, CueList, PlaybackEngine};
use crate::dmx::{Universe, backends::{DmxBackend, VirtualBackend}};
use crate::media::MediaManager;
use crate::fixtures::FixtureLibrary;
use crate::show::ShowFile;

/// UI state flags and dialog state
#[derive(Default)]
pub struct UiState {
    pub show_fixture_panel: bool,
    pub show_media_panel: bool,
    /// Index of the currently selected cue (for editing)
    pub selected_cue_index: Option<usize>,
    /// Whether the save-show dialog is open
    pub show_save_dialog: bool,
    /// Whether the open-show dialog is open
    pub show_open_dialog: bool,
    /// Text input buffer for file path dialogs
    pub file_path_input: String,
    /// Text input buffer for show title
    pub show_title_input: String,
    /// Status message to display to the user
    pub status_message: String,
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
    /// Current show title
    pub show_title: String,
}

impl EasyCueApp {
    /// Create a new application instance
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize with 2 universes (configurable later)
        let universes = vec![
            Universe::new(0),
            Universe::new(1),
        ];

        // Use virtual backend by default (no hardware required)
        let dmx_backend = Box::new(VirtualBackend::new(true)) as Box<dyn DmxBackend>;

        log::info!("EasyCue3 application initialized");
        log::info!("DMX Backend: {}", dmx_backend.name());

        let mut app = Self {
            universes,
            dmx_backend,
            cue_list: CueList::new(),
            playback: PlaybackEngine::new(),
            media: MediaManager::new(),
            fixtures: FixtureLibrary::new(),
            ui_state: UiState {
                show_title_input: "My Show".to_string(),
                file_path_input: "shows/my_show.json".to_string(),
                ..Default::default()
            },
            show_title: "Example Show".to_string(),
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

    /// Load a show file and populate the cue list
    pub fn load_show(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let show = ShowFile::load(path)?;
        self.cue_list.clear();
        for cue in show.cues {
            self.cue_list.add_cue(cue);
        }
        self.show_title = show.title.clone();
        self.ui_state.show_title_input = show.title;
        self.ui_state.selected_cue_index = None;
        self.ui_state.status_message = format!("Loaded show from {:?}", path);
        log::info!("Loaded show: {}", self.show_title);
        Ok(())
    }

    /// Save the current cue list to a show file
    pub fn save_show(&self, path: &std::path::Path, title: &str) -> anyhow::Result<()> {
        let mut show = ShowFile::new(title);
        show.cues = self.cue_list.cues().to_vec();
        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        show.save(path)?;
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
        // Handle keyboard shortcuts (checked before UI to avoid consuming events)
        let (go, back, stop, save, open, record) = ctx.input(|i| (
            i.key_pressed(egui::Key::Space) && !i.modifiers.any(),
            i.key_pressed(egui::Key::B)     && !i.modifiers.any(),
            i.key_pressed(egui::Key::S)     && !i.modifiers.any(),
            i.key_pressed(egui::Key::S)     && i.modifiers.ctrl && !i.modifiers.shift,
            i.key_pressed(egui::Key::O)     && i.modifiers.ctrl,
            i.key_pressed(egui::Key::R)     && i.modifiers.ctrl,
        ));

        if go  { self.playback.go(&mut self.cue_list); }
        if back { self.playback.back(&mut self.cue_list); }
        if stop { self.playback.stop(); }
        if save { self.ui_state.show_save_dialog = true; }
        if open { self.ui_state.show_open_dialog = true; }
        if record {
            let idx = self.record_cue();
            self.ui_state.selected_cue_index = Some(idx);
        }

        // Update playback engine and apply to first universe
        if let Some(universe) = self.universes.first_mut() {
            self.playback.update(universe);

            // Send DMX output
            if let Err(e) = self.dmx_backend.send_universe(universe) {
                log::error!("DMX output error: {}", e);
            }
        }

        // Render UI
        crate::ui::render(ctx, self);

        // Request continuous repaint for smooth fades
        if self.playback.is_playing() {
            ctx.request_repaint();
        }
    }
}
