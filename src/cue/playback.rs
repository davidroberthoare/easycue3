//! Lighting cue playback engine with crossfade support

use crate::cue::{Cue, CueState, decode_universe_key};
use crate::dmx::Universe;
use std::collections::HashMap;
use std::time::Instant;

/// Manages lighting cue playback and crossfades across multiple DMX universes.
/// Navigation (which cue is next) is the caller's responsibility;
/// this engine only starts, updates, and stops fades.
pub struct PlaybackEngine {
    state: CueState,
    current_cue_id: Option<u32>,
    fade_start: Option<Instant>,
    fade_duration: f32,
    /// One 512-channel block per universe (0-indexed; block 0 = universe 1).
    previous_values: Vec<[u8; 512]>,
    target_values: Vec<[u8; 512]>,
}

impl PlaybackEngine {
    pub fn new() -> Self {
        Self {
            state: CueState::Stopped,
            current_cue_id: None,
            fade_start: None,
            fade_duration: 0.0,
            previous_values: vec![[0; 512]],
            target_values: vec![[0; 512]],
        }
    }

    fn ensure_capacity(&mut self, num_universes: usize) {
        while self.previous_values.len() < num_universes {
            self.previous_values.push([0; 512]);
            self.target_values.push([0; 512]);
        }
    }

    /// Start fading to the given lighting cue. The caller has already decided which cue to fire.
    pub fn start(&mut self, cue: &Cue, universes: &[Universe]) {
        let Some(data) = cue.lighting_data() else { return };

        self.ensure_capacity(universes.len());

        // Snapshot current universe state as the starting values for the crossfade.
        for (ui, universe) in universes.iter().enumerate() {
            for ch in 1..=512u16 {
                self.previous_values[ui][(ch - 1) as usize] = universe.get_channel(ch).unwrap_or(0);
            }
        }

        // Tracking mode: start targets from current live state, then overlay only
        // the channels explicitly set in this cue. Unspecified channels hold.
        for i in 0..self.target_values.len() {
            self.target_values[i] = self.previous_values[i];
        }
        for (&key, &value) in &data.channel_values {
            let (universe_1based, channel) = decode_universe_key(key);
            let ui = (universe_1based - 1) as usize;
            if ui < self.target_values.len() && channel >= 1 && channel <= 512 {
                self.target_values[ui][(channel - 1) as usize] = value;
            }
        }

        self.fade_duration = data.fade_up;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        self.current_cue_id = Some(cue.id);

        log::info!("Starting cue {}: {} (fade: {}s)", cue.number, cue.label, data.fade_up);
    }

    /// Fade from the current live universe output to a pre-computed full channel state.
    /// Used for cue jumps: the caller passes the tracked state accumulated across all
    /// cues up to the target, so channels that were never in the jumped-to cue still
    /// land at their correct tracked values.
    pub fn start_to_state(
        &mut self,
        state: &HashMap<u16, u8>,
        fade_time: f32,
        cue_id: Option<u32>,
        universes: &[Universe],
    ) {
        self.ensure_capacity(universes.len());
        for (ui, universe) in universes.iter().enumerate() {
            for ch in 1..=512u16 {
                self.previous_values[ui][(ch - 1) as usize] = universe.get_channel(ch).unwrap_or(0);
            }
        }
        for arr in self.target_values.iter_mut() {
            arr.fill(0);
        }
        for (&key, &value) in state {
            let (universe_1based, channel) = decode_universe_key(key);
            let ui = (universe_1based - 1) as usize;
            if ui < self.target_values.len() && channel >= 1 && channel <= 512 {
                self.target_values[ui][(channel - 1) as usize] = value;
            }
        }
        self.fade_duration = fade_time;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        self.current_cue_id = cue_id;
        log::info!("Jump: fading to tracked state ({:.1}s)", fade_time);
    }

    pub fn stop(&mut self) {
        self.state = CueState::Stopped;
        self.fade_start = None;
    }

    /// Freeze the current cue in place.
    /// * If fading: halts the fade at the current interpolated values (already written
    ///   to the universe on the last `update()` frame) and transitions to Active.
    /// * If already Active: channels are static; this is a no-op for lighting
    ///   (the caller still fades out audio).
    pub fn freeze(&mut self) {
        match self.state {
            CueState::Fading { .. } => {
                self.fade_start = None;
                self.state = CueState::Active;
                log::debug!("Lighting: fade frozen in place");
            }
            CueState::Active => {
                log::debug!("Lighting: pause — cue already active, channels hold");
            }
            CueState::Stopped => {}
        }
    }

    /// Fade all DMX channels across all universes to zero over `fade_seconds`.
    pub fn start_fade_to_black(&mut self, universes: &[Universe], fade_seconds: f32) {
        self.ensure_capacity(universes.len());
        for (ui, universe) in universes.iter().enumerate() {
            for ch in 1..=512u16 {
                self.previous_values[ui][(ch - 1) as usize] = universe.get_channel(ch).unwrap_or(0);
            }
        }
        for arr in self.target_values.iter_mut() {
            arr.fill(0);
        }
        self.fade_duration = fade_seconds;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        self.current_cue_id = None;
        log::info!("Fading to black over {:.1}s", fade_seconds);
    }

    /// Update playback state and write interpolated values to all universes.
    pub fn update(&mut self, universes: &mut [Universe]) {
        match self.state {
            CueState::Fading { .. } => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    let progress = if self.fade_duration > 0.0 {
                        (elapsed / self.fade_duration).min(1.0)
                    } else {
                        1.0
                    };

                    for (ui, universe) in universes.iter_mut().enumerate() {
                        if ui >= self.previous_values.len() {
                            break;
                        }
                        for ch in 1..=512u16 {
                            let prev = self.previous_values[ui][(ch - 1) as usize] as f32;
                            let target = self.target_values[ui][(ch - 1) as usize] as f32;
                            let _ = universe.set_channel(ch, (prev + (target - prev) * progress) as u8);
                        }
                    }

                    if progress >= 1.0 {
                        self.state = CueState::Active;
                        for (i, target) in self.target_values.iter().enumerate() {
                            if let Some(prev) = self.previous_values.get_mut(i) {
                                *prev = *target;
                            }
                        }
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
