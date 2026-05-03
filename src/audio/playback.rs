//! Audio cue playback engine with fade support

use crate::audio::{AudioCueState, AudioPlayer};
use crate::cue::Cue;
use std::time::Instant;

/// Manages audio cue playback and fades.
/// Navigation (which cue is next) is the caller's responsibility;
/// this engine only starts, updates, and stops playback.
pub struct AudioPlaybackEngine {
    state: AudioCueState,
    current_cue_id: Option<u32>,
    fade_start: Option<Instant>,
    fade_in_duration: f32,
    fade_out_duration: f32,
    base_volume: f32,
    pending_lighting_trigger: Option<f32>,
}

impl AudioPlaybackEngine {
    pub fn new() -> Self {
        Self {
            state: AudioCueState::Stopped,
            current_cue_id: None,
            fade_start: None,
            fade_in_duration: 0.0,
            fade_out_duration: 0.0,
            base_volume: 1.0,
            pending_lighting_trigger: None,
        }
    }

    /// Start playing the given audio cue. The caller has already decided which cue to fire.
    pub fn start(&mut self, cue: &Cue, player: &mut AudioPlayer) -> bool {
        let Some(data) = cue.audio_data() else { return false };
        match player.play(&data.audio_path, 0.0) {
            Ok(_) => {
                self.base_volume = data.volume;
                self.fade_in_duration = data.fade_in;
                self.fade_out_duration = data.fade_out;
                self.pending_lighting_trigger = data.triggers_lighting_cue;
                self.current_cue_id = Some(cue.id);

                if data.fade_in > 0.0 {
                    self.state = AudioCueState::FadingIn { progress: 0.0 };
                    self.fade_start = Some(Instant::now());
                    player.set_volume(0.0);
                } else {
                    self.state = AudioCueState::Playing;
                    player.set_volume(self.base_volume);
                }

                log::info!("Starting audio cue {:.2}: {} (volume: {:.0}%, fade in: {}s)",
                    cue.number, cue.label, data.volume * 100.0, data.fade_in);
                true
            }
            Err(e) => {
                log::error!("Failed to play audio cue {:.2}: {}", cue.number, e);
                self.state = AudioCueState::Stopped;
                false
            }
        }
    }

    pub fn stop(&mut self, player: &mut AudioPlayer) {
        player.stop();
        self.state = AudioCueState::Stopped;
        self.fade_start = None;
        self.pending_lighting_trigger = None;
    }

    /// Update fade state each frame. Returns the base volume (caller multiplies by sound_master).
    pub fn update(&mut self, player: &mut AudioPlayer) -> f32 {
        match self.state {
            AudioCueState::FadingIn { .. } => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    let progress = (elapsed / self.fade_in_duration).clamp(0.0, 1.0);
                    let fade_volume = self.base_volume * progress;
                    if progress >= 1.0 {
                        self.state = AudioCueState::Playing;
                        self.fade_start = None;
                        log::debug!("Audio fade in complete");
                    } else {
                        self.state = AudioCueState::FadingIn { progress };
                    }
                    fade_volume
                } else {
                    self.base_volume
                }
            }
            AudioCueState::Playing => {
                if player.is_finished() {
                    self.state = AudioCueState::Stopped;
                    self.pending_lighting_trigger = None;
                    log::debug!("Audio playback finished");
                }
                self.base_volume
            }
            AudioCueState::FadingOut { .. } => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    let progress = (elapsed / self.fade_out_duration).clamp(0.0, 1.0);
                    let fade_volume = self.base_volume * (1.0 - progress);
                    if progress >= 1.0 {
                        player.stop();
                        self.state = AudioCueState::Stopped;
                        self.fade_start = None;
                        self.pending_lighting_trigger = None;
                        log::debug!("Audio fade out complete");
                    } else {
                        self.state = AudioCueState::FadingOut { progress };
                    }
                    fade_volume
                } else {
                    0.0
                }
            }
            AudioCueState::Stopped => 0.0,
        }
    }

    pub fn fade_out(&mut self, fade_duration: f32) {
        if matches!(self.state, AudioCueState::Playing | AudioCueState::FadingIn { .. }) {
            self.fade_out_duration = fade_duration;
            self.fade_start = Some(Instant::now());
            self.state = AudioCueState::FadingOut { progress: 0.0 };
        }
    }

    pub fn state(&self) -> AudioCueState {
        self.state
    }

    pub fn is_playing(&self) -> bool {
        !matches!(self.state, AudioCueState::Stopped)
    }

    /// The stable ID of the audio cue currently playing or fading. Used for row coloring in UI.
    pub fn current_cue_id(&self) -> Option<u32> {
        if matches!(self.state, AudioCueState::Stopped) {
            None
        } else {
            self.current_cue_id
        }
    }

    /// Take any pending lighting trigger (consumed once, then None)
    pub fn take_pending_lighting_trigger(&mut self) -> Option<f32> {
        self.pending_lighting_trigger.take()
    }
}

impl Default for AudioPlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}
