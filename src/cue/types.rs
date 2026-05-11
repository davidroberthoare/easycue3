//! Cue data structures — unified lighting and audio cues

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "audio")]
use std::path::PathBuf;

/// Encode (1-based universe, 1-based channel 1–512) into the u16 key stored in
/// `LightingData::channel_values`.  Universe 1, channels 1–512 map to keys 1–512,
/// preserving full backwards compatibility with existing show files.
pub fn universe_key(universe: u16, channel: u16) -> u16 {
    (universe.saturating_sub(1)) * 512 + channel
}

/// Decode a `channel_values` key back to `(universe, channel)` (both 1-based).
pub fn decode_universe_key(key: u16) -> (u16, u16) {
    let z = key.saturating_sub(1);
    (z / 512 + 1, z % 512 + 1)
}

/// Data for a lighting cue: channel intensities and fade timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingData {
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_up: f32,
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_down: f32,
    /// Channel intensity values (0-100). Only non-zero channels stored.
    /// Keys are encoded via `universe_key(universe, channel)` so universe 1
    /// keys are identical to raw channel numbers (backwards compatible).
    #[serde(deserialize_with = "crate::serde_helpers::deserialize_channel_map")]
    pub channel_values: HashMap<u16, u8>,
}

impl Default for LightingData {
    fn default() -> Self {
        Self {
            fade_up: 3.0,
            fade_down: 3.0,
            channel_values: HashMap::new(),
        }
    }
}

impl LightingData {
    /// Set a channel in universe 1 (backwards-compatible shorthand).
    pub fn set_channel(&mut self, channel: u16, value: u8) {
        self.set_channel_in_universe(1, channel, value);
    }

    /// Get a channel from universe 1.
    pub fn get_channel(&self, channel: u16) -> u8 {
        self.get_channel_in_universe(1, channel)
    }

    /// Set a channel value in a specific universe (1-based, channel 1–512).
    pub fn set_channel_in_universe(&mut self, universe: u16, channel: u16, value: u8) {
        let key = universe_key(universe, channel);
        if value > 0 {
            self.channel_values.insert(key, value);
        } else {
            self.channel_values.remove(&key);
        }
    }

    /// Get a channel value from a specific universe (1-based).
    pub fn get_channel_in_universe(&self, universe: u16, channel: u16) -> u8 {
        self.channel_values
            .get(&universe_key(universe, channel))
            .copied()
            .unwrap_or(0)
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
        crate::paths::resolve_media_path(&self.audio_path)
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

/// Data for an adjust cue: ramps the sound master to a target volume, then optionally stops audio.
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustData {
    /// Cue number of the specific audio cue to target. None = affect global sound master.
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub target_audio_cue: Option<f32>,
    /// Target volume level (0.0–1.0)
    #[serde(default = "default_audio_volume", serialize_with = "crate::serde_helpers::round_f32_2")]
    pub volume: f32,
    /// Seconds to reach the target (0 = instant)
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_time: f32,
    /// Stop the targeted stream (or all streams) when the fade completes
    #[serde(default)]
    pub stop_when_complete: bool,
}

#[cfg(feature = "audio")]
impl AdjustData {
    pub fn new() -> Self {
        Self { target_audio_cue: None, volume: 0.8, fade_time: 2.0, stop_when_complete: false }
    }
}

#[cfg(feature = "audio")]
impl Default for AdjustData {
    fn default() -> Self { Self::new() }
}

/// The payload of a cue — what kind it is and its type-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CueKind {
    Lighting(LightingData),
    #[cfg(feature = "audio")]
    Audio(AudioData),
    /// Ramps the sound master to a new level; optionally stops all audio when done.
    #[cfg(feature = "audio")]
    Adjust(AdjustData),
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

    #[cfg(feature = "audio")]
    pub fn new_adjust(number: f32) -> Self {
        Self {
            id: 0,
            number,
            label: String::new(),
            autofollow: None,
            kind: CueKind::Adjust(AdjustData::new()),
        }
    }

    pub fn is_lighting(&self) -> bool {
        matches!(self.kind, CueKind::Lighting(_))
    }

    #[cfg(feature = "audio")]
    pub fn is_audio(&self) -> bool {
        matches!(self.kind, CueKind::Audio(_))
    }

    #[cfg(feature = "audio")]
    pub fn is_adjust(&self) -> bool {
        matches!(self.kind, CueKind::Adjust(_))
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

    #[cfg(feature = "audio")]
    pub fn adjust_data(&self) -> Option<&AdjustData> {
        match &self.kind {
            CueKind::Adjust(data) => Some(data),
            _ => None,
        }
    }

    #[cfg(feature = "audio")]
    pub fn adjust_data_mut(&mut self) -> Option<&mut AdjustData> {
        match &mut self.kind {
            CueKind::Adjust(data) => Some(data),
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
