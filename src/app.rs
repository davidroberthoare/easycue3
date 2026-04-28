//! Main application state and logic

use crate::cue::{CueList, PlaybackEngine};
use crate::dmx::{Universe, backends::{DmxBackend, VirtualBackend}};
use crate::media::MediaManager;
use crate::fixtures::FixtureLibrary;

/// UI state flags
#[derive(Default)]
pub struct UiState {
    pub show_fixture_panel: bool,
    pub show_media_panel: bool,
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

        Self {
            universes,
            dmx_backend,
            cue_list: CueList::new(),
            playback: PlaybackEngine::new(),
            media: MediaManager::new(),
            fixtures: FixtureLibrary::new(),
            ui_state: UiState::default(),
        }
    }
}

impl eframe::App for EasyCueApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
