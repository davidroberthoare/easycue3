//! Cue playback engine with crossfade support

use crate::cue::{Cue, CueList, CueState};
use crate::dmx::Universe;
use std::time::Instant;

/// Manages cue playback and crossfades
pub struct PlaybackEngine {
    /// Current playback state
    state: CueState,
    /// When the current fade started
    fade_start: Option<Instant>,
    /// Duration of the current fade in seconds
    fade_duration: f32,
    /// Previous cue values (for crossfade from)
    previous_values: [u8; 512],
    /// Target cue values (for crossfade to)
    target_values: [u8; 512],
    /// Autofollow timer: when cue should automatically advance to next
    autofollow_time: Option<Instant>,
}

impl PlaybackEngine {
    /// Create a new playback engine
    pub fn new() -> Self {
        Self {
            state: CueState::Stopped,
            fade_start: None,
            fade_duration: 0.0,
            previous_values: [0; 512],
            target_values: [0; 512],
            autofollow_time: None,
        }
    }

    /// Start playing a cue (GO command)
    pub fn go(&mut self, cue_list: &mut CueList, universe: &Universe) -> bool {
        if let Some(next_idx) = cue_list.next_index() {
            if let Some(cue) = cue_list.get_cue(next_idx) {
                self.start_cue(cue, universe);
                cue_list.set_current_index(Some(next_idx));
                return true;
            }
        }
        false
    }

    /// Go back to previous cue (BACK command)
    pub fn back(&mut self, cue_list: &mut CueList, universe: &Universe) -> bool {
        if let Some(prev_idx) = cue_list.previous_index() {
            if let Some(cue) = cue_list.get_cue(prev_idx) {
                self.start_cue(cue, universe);
                cue_list.set_current_index(Some(prev_idx));
                return true;
            }
        }
        false
    }

    /// Jump to a specific cue by index
    pub fn go_to_cue(&mut self, cue_list: &CueList, cue_index: usize, universe: &Universe) -> bool {
        if let Some(cue) = cue_list.get_cue(cue_index) {
            self.start_cue(cue, universe);
            // Note: We can't mutate cue_list here since we only have &CueList
            // The caller will need to update current_index separately
            return true;
        }
        false
    }

    /// Stop playback
    pub fn stop(&mut self) {
        self.state = CueState::Stopped;
        self.fade_start = None;
        self.autofollow_time = None;
    }

    /// Start playing a specific cue
    fn start_cue(&mut self, cue: &Cue, universe: &Universe) {
        // Capture current live output as starting point for fade
        // This prevents snapping when interrupting an existing fade
        for channel in 1..=512 {
            let value = universe.get_channel(channel).unwrap_or(0);
            self.previous_values[(channel - 1) as usize] = value;
        }
        
        // Set target values from cue
        self.target_values.fill(0);
        for (&channel, &value) in &cue.channel_values {
            if channel >= 1 && channel <= 512 {
                self.target_values[(channel - 1) as usize] = value;
            }
        }

        // Start fade
        self.fade_duration = cue.fade_up;
        self.fade_start = Some(Instant::now());
        self.state = CueState::Fading { progress: 0.0 };
        
        // Set up autofollow timer if configured
        if let Some(autofollow_delay) = cue.autofollow {
            self.autofollow_time = Some(Instant::now() + std::time::Duration::from_secs_f32(autofollow_delay));
            log::info!("Starting cue {}: {} (fade: {}s, autofollow: {}s)", cue.number, cue.label, cue.fade_up, autofollow_delay);
        } else {
            self.autofollow_time = None;
            log::info!("Starting cue {}: {} (fade: {}s)", cue.number, cue.label, cue.fade_up);
        }
    }

    /// Update the playback state and apply to universe
    /// Returns true if autofollow should trigger the next cue
    pub fn update(&mut self, universe: &mut Universe) -> bool {
        let mut should_autofollow = false;
        
        match self.state {
            CueState::Fading { .. } => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    let progress = if self.fade_duration > 0.0 {
                        (elapsed / self.fade_duration).min(1.0)
                    } else {
                        1.0
                    };

                    // Linear crossfade (TODO: support other curves)
                    for channel in 1..=512 {
                        let prev = self.previous_values[(channel - 1) as usize] as f32;
                        let target = self.target_values[(channel - 1) as usize] as f32;
                        let current = prev + (target - prev) * progress;
                        let _ = universe.set_channel(channel, current as u8);
                    }

                    // Update state
                    if progress >= 1.0 {
                        self.state = CueState::Active;
                        self.previous_values = self.target_values;
                        log::debug!("Fade complete");
                    } else {
                        self.state = CueState::Fading { progress };
                    }
                }
            }
            CueState::Active => {
                // Holding steady - check for autofollow
                if let Some(autofollow_time) = self.autofollow_time {
                    if Instant::now() >= autofollow_time {
                        should_autofollow = true;
                        self.autofollow_time = None;
                        log::info!("Autofollow triggered");
                    }
                }
            }
            CueState::Stopped => {
                // Not playing
            }
        }
        
        should_autofollow
    }

    /// Get the current playback state
    pub fn state(&self) -> CueState {
        self.state
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        !matches!(self.state, CueState::Stopped)
    }
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}
