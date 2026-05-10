//! Lighting cue playback engine with crossfade support

use crate::cue::{Cue, CueState};
use crate::dmx::Universe;
use std::time::Instant;

/// Manages lighting cue playback and crossfades.
/// Navigation (which cue is next) is the caller's responsibility;
/// this engine only starts, updates, and stops fades.
pub struct PlaybackEngine {
    state: CueState,
    current_cue_id: Option<u32>,
    fade_start: Option<Instant>,
    fade_duration: f32,
    previous_values: [u8; 512],
    target_values: [u8; 512],
}

impl PlaybackEngine {
    pub fn new() -> Self {
        Self {
            state: CueState::Stopped,
            current_cue_id: None,
            fade_start: None,
            fade_duration: 0.0,
            previous_values: [0; 512],
            target_values: [0; 512],
        }
    }

    /// Start fading to the given lighting cue. The caller has already decided which cue to fire.
    pub fn start(&mut self, cue: &Cue, universe: &Universe) {
        let Some(data) = cue.lighting_data() else { return };

        for channel in 1..=512 {
            self.previous_values[(channel - 1) as usize] = universe.get_channel(channel).unwrap_or(0);
        }

        self.target_values.fill(0);
        for (&channel, &value) in &data.channel_values {
            if channel >= 1 && channel <= 512 {
                self.target_values[(channel - 1) as usize] = value;
            }
        }

        self.fade_duration = data.fade_up;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        self.current_cue_id = Some(cue.id);

        log::info!("Starting cue {}: {} (fade: {}s)", cue.number, cue.label, data.fade_up);
    }

    pub fn stop(&mut self) {
        self.state = CueState::Stopped;
        self.fade_start = None;
    }

    /// Fade all DMX channels from current universe values to zero over `fade_seconds`.
    pub fn start_fade_to_black(&mut self, universe: &Universe, fade_seconds: f32) {
        for channel in 1..=512 {
            self.previous_values[(channel - 1) as usize] = universe.get_channel(channel).unwrap_or(0);
        }
        self.target_values.fill(0);
        self.fade_duration = fade_seconds;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        self.current_cue_id = None;
        log::info!("Fading to black over {:.1}s", fade_seconds);
    }

    /// Update playback state and write interpolated values to universe.
    pub fn update(&mut self, universe: &mut Universe) {
        match self.state {
            CueState::Fading { .. } => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    let progress = if self.fade_duration > 0.0 {
                        (elapsed / self.fade_duration).min(1.0)
                    } else {
                        1.0
                    };

                    for channel in 1..=512 {
                        let prev = self.previous_values[(channel - 1) as usize] as f32;
                        let target = self.target_values[(channel - 1) as usize] as f32;
                        let _ = universe.set_channel(channel, (prev + (target - prev) * progress) as u8);
                    }

                    if progress >= 1.0 {
                        self.state = CueState::Active;
                        self.previous_values = self.target_values;
                        log::debug!("Fade complete");
                    } else {
                        self.state = CueState::Fading { progress };
                    }
                }
            }
            CueState::Active | CueState::Stopped => {}
        }
    }

    pub fn state(&self) -> CueState {
        self.state
    }

    pub fn is_playing(&self) -> bool {
        !matches!(self.state, CueState::Stopped)
    }

    /// The stable ID of the cue currently active or fading. Used for row coloring in UI.
    pub fn current_cue_id(&self) -> Option<u32> {
        if matches!(self.state, CueState::Stopped) {
            None
        } else {
            self.current_cue_id
        }
    }

    /// Fade progress [0,1] if currently fading, None otherwise.
    pub fn fade_progress(&self) -> Option<f32> {
        match self.state {
            CueState::Fading { progress } => Some(progress),
            _ => None,
        }
    }
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}
