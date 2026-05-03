//! Cue data structures — unified lighting and audio cues

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "audio")]
use std::path::PathBuf;

/// Data for a lighting cue: channel intensities and fade timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingData {
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_up: f32,
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_down: f32,
    /// Channel intensity values (0-100). Only non-zero channels stored.
    #[serde(deserialize_with = "crate::serde_helpers::deserialize_channel_map")]
    pub channel_values: HashMap<u16, u8>,
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub triggers_audio_cue: Option<f32>,
}

impl Default for LightingData {
    fn default() -> Self {
        Self {
            fade_up: 3.0,
            fade_down: 3.0,
            channel_values: HashMap::new(),
            triggers_audio_cue: None,
        }
    }
}

impl LightingData {
    pub fn set_channel(&mut self, channel: u16, value: u8) {
        if value > 0 {
            self.channel_values.insert(channel, value);
        } else {
            self.channel_values.remove(&channel);
        }
    }

    pub fn get_channel(&self, channel: u16) -> u8 {
        self.channel_values.get(&channel).copied().unwrap_or(0)
    }
}

/// Data for an audio cue: file path and playback settings
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// Path to audio file. Relative paths are resolved from "media/" directory.
    pub audio_path: PathBuf,
    #[serde(default = "default_audio_volume", serialize_with = "crate::serde_helpers::round_f32_2")]
    pub volume: f32,
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_in: f32,
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_out: f32,
    #[serde(default)]
    pub notes: String,
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub triggers_lighting_cue: Option<f32>,
    /// Fixed playback duration in seconds. When elapsed the cue fades out (or stops if fade_out==0).
    /// None = play until the file ends.
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub length: Option<f32>,
}

#[cfg(feature = "audio")]
fn default_audio_volume() -> f32 {
    0.8
}

#[cfg(feature = "audio")]
impl AudioData {
    pub fn new(path: PathBuf) -> Self {
        Self {
            audio_path: path,
            volume: 0.8,
            fade_in: 0.0,
            fade_out: 0.0,
            notes: String::new(),
            triggers_lighting_cue: None,
            length: None,
        }
    }

    pub fn filename(&self) -> String {
        self.audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string()
    }

    /// Resolve to the actual filesystem path, falling back to "media/" directory
    pub fn resolved_path(&self) -> PathBuf {
        if self.audio_path.is_absolute() || self.audio_path.exists() {
            return self.audio_path.clone();
        }
        let media_path = PathBuf::from("media").join(&self.audio_path);
        if media_path.exists() {
            return media_path;
        }
        self.audio_path.clone()
    }

    /// Set path, stripping "media/" prefix so show files store just the filename
    pub fn set_path(&mut self, path: PathBuf) {
        if let Ok(stripped) = path.strip_prefix("media") {
            self.audio_path = stripped.to_path_buf();
        } else {
            self.audio_path = path;
        }
    }
}

/// The payload of a cue — what kind it is and its type-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CueKind {
    Lighting(LightingData),
    #[cfg(feature = "audio")]
    Audio(AudioData),
}

/// A single cue — identity, display number, label, and type-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    /// Stable identity — assigned once, never changes. 0 means unassigned (set by CueList::add_cue).
    #[serde(default)]
    pub id: u32,
    /// Display number (e.g. 1.0, 1.5, 2.0) — not used as identity
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub number: f32,
    /// Optional text label
    pub label: String,
    /// Seconds after this cue fires to automatically fire the next sequential cue. None = manual only.
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub autofollow: Option<f32>,
    /// Cue type and its data
    pub kind: CueKind,
}

impl Cue {
    pub fn new_lighting(number: f32) -> Self {
        Self {
            id: 0,
            number,
            label: String::new(),
            autofollow: None,
            kind: CueKind::Lighting(LightingData::default()),
        }
    }

    #[cfg(feature = "audio")]
    pub fn new_audio(number: f32, path: PathBuf) -> Self {
        Self {
            id: 0,
            number,
            label: String::new(),
            autofollow: None,
            kind: CueKind::Audio(AudioData::new(path)),
        }
    }

    pub fn is_lighting(&self) -> bool {
        matches!(self.kind, CueKind::Lighting(_))
    }

    #[cfg(feature = "audio")]
    pub fn is_audio(&self) -> bool {
        matches!(self.kind, CueKind::Audio(_))
    }

    pub fn lighting_data(&self) -> Option<&LightingData> {
        match &self.kind {
            CueKind::Lighting(data) => Some(data),
            #[cfg(feature = "audio")]
            _ => None,
        }
    }

    pub fn lighting_data_mut(&mut self) -> Option<&mut LightingData> {
        match &mut self.kind {
            CueKind::Lighting(data) => Some(data),
            #[cfg(feature = "audio")]
            _ => None,
        }
    }

    #[cfg(feature = "audio")]
    pub fn audio_data(&self) -> Option<&AudioData> {
        match &self.kind {
            CueKind::Audio(data) => Some(data),
            _ => None,
        }
    }

    #[cfg(feature = "audio")]
    pub fn audio_data_mut(&mut self) -> Option<&mut AudioData> {
        match &mut self.kind {
            CueKind::Audio(data) => Some(data),
            _ => None,
        }
    }

    /// Set a channel value (no-op for non-lighting cues)
    pub fn set_channel(&mut self, channel: u16, value: u8) {
        if let Some(data) = self.lighting_data_mut() {
            data.set_channel(channel, value);
        }
    }

    /// Get a channel value (returns 0 for non-lighting cues)
    pub fn get_channel(&self, channel: u16) -> u8 {
        self.lighting_data().map(|d| d.get_channel(channel)).unwrap_or(0)
    }
}

/// Current state of lighting cue playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CueState {
    Stopped,
    Fading { progress: f32 },
    Active,
}

impl Default for CueState {
    fn default() -> Self {
        Self::Stopped
    }
}
