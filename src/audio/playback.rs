//! Audio cue playback engine with fade support

use crate::audio::{AudioCue, AudioCueList, AudioCueState, AudioPlayer};
use std::time::Instant;

/// Manages audio cue playback and crossfades
pub struct AudioPlaybackEngine {
    /// Current playback state
    state: AudioCueState,
    
    /// When the current fade started
    fade_start: Option<Instant>,
    
    /// Fade in duration in seconds
    fade_in_duration: f32,
    
    /// Fade out duration in seconds
    fade_out_duration: f32,
    
    /// Base volume for current cue (before fade is applied)
    base_volume: f32,
    
    /// Optional lighting cue to trigger when audio starts
    pending_lighting_trigger: Option<f32>,
}

impl AudioPlaybackEngine {
    /// Create a new audio playback engine
    pub fn new() -> Self {
        Self {
            state: AudioCueState::Stopped,
            fade_start: None,
            fade_in_duration: 0.0,
            fade_out_duration: 0.0,
            base_volume: 1.0,
            pending_lighting_trigger: None,
        }
    }
    
    /// Start playing an audio cue (GO command)
    pub fn go(&mut self, cue_list: &mut AudioCueList, player: &mut AudioPlayer) -> bool {
        if let Some(next_idx) = cue_list.next_index() {
            if let Some(cue) = cue_list.get_cue(next_idx) {
                if self.start_cue(cue, player) {
                    cue_list.set_current_index(Some(next_idx));
                    return true;
                }
            }
        }
        false
    }
    
    /// Go back to previous audio cue (BACK command)
    pub fn back(&mut self, cue_list: &mut AudioCueList, player: &mut AudioPlayer) -> bool {
        if let Some(prev_idx) = cue_list.previous_index() {
            if let Some(cue) = cue_list.get_cue(prev_idx) {
                if self.start_cue(cue, player) {
                    cue_list.set_current_index(Some(prev_idx));
                    return true;
                }
            }
        }
        false
    }
    
    /// Jump to a specific audio cue by index
    pub fn go_to_cue(&mut self, cue_list: &AudioCueList, cue_index: usize, player: &mut AudioPlayer) -> bool {
        if let Some(cue) = cue_list.get_cue(cue_index) {
            self.start_cue(cue, player)
        } else {
            false
        }
    }
    
    /// Stop playback
    pub fn stop(&mut self, player: &mut AudioPlayer) {
        player.stop();
        self.state = AudioCueState::Stopped;
        self.fade_start = None;
        self.pending_lighting_trigger = None;
    }
    
    /// Start playing a specific audio cue
    fn start_cue(&mut self, cue: &AudioCue, player: &mut AudioPlayer) -> bool {
        // Attempt to play the audio file
        match player.play(&cue.audio_path, 0.0) {
            Ok(_) => {
                self.base_volume = cue.volume;
                self.fade_in_duration = cue.fade_in;
                self.fade_out_duration = cue.fade_out;
                self.pending_lighting_trigger = cue.triggers_lighting_cue;
                
                // Set initial state
                if cue.fade_in > 0.0 {
                    self.state = AudioCueState::FadingIn { progress: 0.0 };
                    self.fade_start = Some(Instant::now());
                    player.set_volume(0.0);
                } else {
                    self.state = AudioCueState::Playing;
                    player.set_volume(self.base_volume);
                }
                
                log::info!("Starting audio cue {}: {} (volume: {:.0}%, fade in: {}s)", 
                          cue.number, cue.label, cue.volume * 100.0, cue.fade_in);
                true
            }
            Err(e) => {
                log::error!("Failed to play audio cue {}: {}", cue.number, e);
                self.state = AudioCueState::Stopped;
                false
            }
        }
    }
    
    /// Update the playback state and apply fades (called each frame)
    pub fn update(&mut self, player: &mut AudioPlayer) {
        match self.state {
            AudioCueState::FadingIn { progress: _ } => {
                if let Some(start_time) = self.fade_start {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let new_progress = (elapsed / self.fade_in_duration).clamp(0.0, 1.0);
                    
                    // Apply fade curve (linear for now)
                    let fade_volume = self.base_volume * new_progress;
                    player.set_volume(fade_volume);
                    
                    if new_progress >= 1.0 {
                        // Fade in complete
                        self.state = AudioCueState::Playing;
                        self.fade_start = None;
                        log::debug!("Audio fade in complete");
                    } else {
                        self.state = AudioCueState::FadingIn { progress: new_progress };
                    }
                }
            }
            
            AudioCueState::Playing => {
                // Check if playback has finished
                if player.is_finished() {
                    self.state = AudioCueState::Stopped;
                    self.pending_lighting_trigger = None;
                    log::debug!("Audio playback finished");
                }
            }
            
            AudioCueState::FadingOut { progress: _ } => {
                if let Some(start_time) = self.fade_start {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let new_progress = (elapsed / self.fade_out_duration).clamp(0.0, 1.0);
                    
                    // Apply fade curve (linear for now)
                    let fade_volume = self.base_volume * (1.0 - new_progress);
                    player.set_volume(fade_volume);
                    
                    if new_progress >= 1.0 {
                        // Fade out complete, stop playback
                        player.stop();
                        self.state = AudioCueState::Stopped;
                        self.fade_start = None;
                        self.pending_lighting_trigger = None;
                        log::debug!("Audio fade out complete");
                    } else {
                        self.state = AudioCueState::FadingOut { progress: new_progress };
                    }
                }
            }
            
            AudioCueState::Stopped => {
                // Nothing to update
            }
        }
    }
    
    /// Initiate a fade out
    pub fn fade_out(&mut self, fade_duration: f32) {
        if matches!(self.state, AudioCueState::Playing | AudioCueState::FadingIn { .. }) {
            self.fade_out_duration = fade_duration;
            self.fade_start = Some(Instant::now());
            self.state = AudioCueState::FadingOut { progress: 0.0 };
            log::debug!("Starting audio fade out ({}s)", fade_duration);
        }
    }
    
    /// Get the current playback state
    pub fn state(&self) -> AudioCueState {
        self.state
    }
    
    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        !matches!(self.state, AudioCueState::Stopped)
    }
    
    /// Take any pending lighting trigger (returns Some(cue_number) once, then None)
    pub fn take_pending_lighting_trigger(&mut self) -> Option<f32> {
        self.pending_lighting_trigger.take()
    }
}

impl Default for AudioPlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}
