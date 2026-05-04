//! Audio types — AudioCueState for playback engine + legacy AudioCue for show file migration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Legacy audio cue struct — kept only for deserializing old show files.
/// New code uses crate::cue::AudioData inside a CueKind::Audio variant.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCue {
    #[serde(default)]
    pub id: u32,
    pub number: f32,
    pub label: String,
    pub audio_path: PathBuf,
    pub volume: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub notes: String,
    #[serde(default)]
    pub triggers_lighting_cue: Option<u32>,
}

/// Current state of audio cue playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCueState {
    Stopped,
    FadingIn { progress: f32 },
    Playing,
    FadingOut { progress: f32 },
}

impl Default for AudioCueState {
    fn default() -> Self {
        Self::Stopped
    }
}
