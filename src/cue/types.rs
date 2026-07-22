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
    /// Effect actions executed when this cue fires. Tracking-style: a started
    /// effect keeps running through later cues until one stops it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effect_actions: Vec<crate::effects::EffectAction>,
}

impl Default for LightingData {
    fn default() -> Self {
        Self {
            fade_up: 3.0,
            fade_down: 3.0,
            channel_values: HashMap::new(),
            effect_actions: Vec::new(),
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

/// A single output-device route for an audio cue.
///
/// The cue plays simultaneously on each route; `volume` scales the cue's base
/// volume for that specific device (1.0 = same as the cue level).
/// `device_name` empty string means "default output device".
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutputRoute {
    /// Output device name as returned by the OS (empty = default device).
    #[serde(default)]
    pub device_name: String,
    /// Per-route volume scale (0.0–1.0); multiplied with the cue's base volume.
    #[serde(default = "default_route_volume", serialize_with = "crate::serde_helpers::round_f32_2")]
    pub volume: f32,
    /// Stereo pan position: -1.0 = full left, 0.0 = center, 1.0 = full right.
    /// Uses constant-power pan law.
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub pan: f32,
    /// First channel (0-based) of the target stereo pair on a multi-channel
    /// device: 0 = outputs 1-2, 2 = outputs 3-4, ...  Always 0 for plain
    /// stereo devices, so the field is omitted from show files in that case.
    #[serde(default, skip_serializing_if = "channel_offset_is_zero")]
    pub channel_offset: u16,
}

#[cfg(feature = "audio")]
fn default_route_volume() -> f32 { 1.0 }

#[cfg(feature = "audio")]
fn channel_offset_is_zero(v: &u16) -> bool { *v == 0 }

#[cfg(feature = "audio")]
impl Default for AudioOutputRoute {
    fn default() -> Self {
        Self { device_name: String::new(), volume: 1.0, pan: 0.0, channel_offset: 0 }
    }
}

/// A per-output volume and/or pan fade issued by an Adjust cue.
///
/// Fades the volume and/or pan of a specific output device route on the targeted
/// audio stream.  Use `output_fades` to move sound between outputs (fade one
/// device to 0.0, another to 1.0) or to sweep pan position over `fade_time`.
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFade {
    /// Device to target. Empty = default device.
    pub device_name: String,
    /// Volume to fade to (0.0–1.0).
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub target_volume: f32,
    /// Pan position to fade to (-1.0 = full left, 0.0 = center, 1.0 = full right).
    /// None = do not fade pan (volume only).
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub target_pan: Option<f32>,
    /// First channel (0-based) of the target stereo pair — see
    /// `AudioOutputRoute::channel_offset`.
    #[serde(default, skip_serializing_if = "channel_offset_is_zero")]
    pub channel_offset: u16,
}

/// Data for an audio cue: file path and playback settings
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// Path to audio file. Relative paths are resolved from "media/" directory.
    pub audio_path: PathBuf,
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
    /// Output routing: one entry per physical device. Volume and pan are absolute per-device levels.
    /// Defaults to a single default-device route at full volume.
    #[serde(default = "default_output_routes")]
    pub output_routes: Vec<AudioOutputRoute>,
}

#[cfg(feature = "audio")]
fn default_output_routes() -> Vec<AudioOutputRoute> {
    vec![AudioOutputRoute::default()]
}

#[cfg(feature = "audio")]
impl AudioData {
    pub fn new(path: PathBuf) -> Self {
        Self {
            audio_path: path,
            fade_in: 0.0,
            fade_out: 0.0,
            notes: String::new(),
            length: None,
            output_routes: vec![AudioOutputRoute::default()],
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

/// Data for an adjust cue: fades per-device volume/pan on a targeted audio stream.
#[cfg(feature = "audio")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustData {
    /// Cue number of the specific audio cue to target. None = all playing streams.
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub target_audio_cue: Option<f32>,
    /// Seconds to reach the target (0 = instant)
    #[serde(default, serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_time: f32,
    /// Stop the targeted stream when the fade completes
    #[serde(default)]
    pub stop_when_complete: bool,
    /// Per-output-device volume/pan fades. Always at least one entry.
    #[serde(default = "default_output_fades")]
    pub output_fades: Vec<OutputFade>,
}

#[cfg(feature = "audio")]
fn default_output_fades() -> Vec<OutputFade> {
    vec![OutputFade::default()]
}

#[cfg(feature = "audio")]
impl Default for OutputFade {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            target_volume: 1.0,
            target_pan: None,
            channel_offset: 0,
        }
    }
}

#[cfg(feature = "audio")]
impl AdjustData {
    pub fn new() -> Self {
        Self {
            target_audio_cue: None,
            fade_time: 2.0,
            stop_when_complete: false,
            output_fades: vec![OutputFade::default()],
        }
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
#[derive(Debug, Clone)]
pub struct Cue {
    /// Stable identity — assigned once, never changes. 0 means unassigned (set by CueList::add_cue).
    pub id: u32,
    /// Display number (e.g. 1.0, 1.5, 2.0) — not used as identity
    pub number: f32,
    /// Optional text label
    pub label: String,
    /// Seconds after this cue fires to automatically fire the next sequential cue. None = manual only.
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

impl serde::Serialize for Cue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("autofollow", &self.autofollow.map(|v| (v * 100.0).round() / 100.0))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("label", &self.label)?;
        map.serialize_entry("number", &((self.number * 100.0).round() / 100.0))?;
        match &self.kind {
            CueKind::Lighting(data) => {
                map.serialize_entry("type", "Lighting")?;
                map.serialize_entry("data", data)?;
            }
            #[cfg(feature = "audio")]
            CueKind::Audio(data) => {
                map.serialize_entry("type", "Audio")?;
                map.serialize_entry("data", data)?;
            }
            #[cfg(feature = "audio")]
            CueKind::Adjust(data) => {
                map.serialize_entry("type", "Adjust")?;
                map.serialize_entry("data", data)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Cue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CueVisitor;
        impl<'de> serde::de::Visitor<'de> for CueVisitor {
            type Value = Cue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a cue object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Cue, A::Error> {
                let mut id: Option<u32> = None;
                let mut number: Option<f32> = None;
                let mut label: Option<String> = None;
                let mut autofollow: Option<Option<f32>> = None;
                let mut cue_type: Option<String> = None;
                let mut data: Option<serde_json::Value> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id"         => id         = Some(map.next_value()?),
                        "number"     => number     = Some(map.next_value()?),
                        "label"      => label      = Some(map.next_value()?),
                        "autofollow" => autofollow = Some(map.next_value()?),
                        "type"       => cue_type   = Some(map.next_value()?),
                        "data"       => data       = Some(map.next_value()?),
                        _            => { let _ = map.next_value::<serde_json::Value>()?; }
                    }
                }
                let cue_type = cue_type.ok_or_else(|| serde::de::Error::missing_field("type"))?;
                let data     = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;
                let kind = match cue_type.as_str() {
                    "Lighting" => CueKind::Lighting(
                        serde_json::from_value(data).map_err(serde::de::Error::custom)?
                    ),
                    #[cfg(feature = "audio")]
                    "Audio" => CueKind::Audio(
                        serde_json::from_value(data).map_err(serde::de::Error::custom)?
                    ),
                    #[cfg(feature = "audio")]
                    "Adjust" => CueKind::Adjust(
                        serde_json::from_value(data).map_err(serde::de::Error::custom)?
                    ),
                    other => return Err(serde::de::Error::unknown_variant(
                        other, &["Lighting", "Audio", "Adjust"],
                    )),
                };
                Ok(Cue {
                    id: id.unwrap_or(0),
                    number: number.ok_or_else(|| serde::de::Error::missing_field("number"))?,
                    label: label.ok_or_else(|| serde::de::Error::missing_field("label"))?,
                    autofollow: autofollow.unwrap_or(None),
                    kind,
                })
            }
        }
        deserializer.deserialize_map(CueVisitor)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_data_loads_from_pre_effects_show_files() {
        // A cue exactly as older versions saved it — no effect_actions field.
        let json = r#"{"fade_up": 3.0, "fade_down": 2.0, "channel_values": {"10": 59}}"#;
        let data: LightingData = serde_json::from_str(json).unwrap();
        assert!(data.effect_actions.is_empty());
        assert_eq!(data.get_channel(10), 59);
    }

    #[test]
    fn lighting_data_without_effects_serializes_without_the_field() {
        // Keeps untouched show files byte-identical (and autosave comparison stable).
        let json = serde_json::to_string(&LightingData::default()).unwrap();
        assert!(!json.contains("effect_actions"));
    }

    #[test]
    fn cue_round_trips_effect_actions() {
        let mut cue = Cue::new_lighting(1.0);
        if let Some(d) = cue.lighting_data_mut() {
            d.effect_actions.push(crate::effects::EffectAction::Start {
                effect_id: 2,
                fixtures: vec![1, 3],
            });
            d.effect_actions.push(crate::effects::EffectAction::StopAll);
        }
        let json = serde_json::to_string(&cue).unwrap();
        let back: Cue = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.lighting_data().unwrap().effect_actions,
            cue.lighting_data().unwrap().effect_actions
        );
    }

    #[cfg(feature = "audio")]
    #[test]
    fn audio_route_loads_from_pre_multichannel_show_files() {
        // A route exactly as older versions saved it — no channel_offset field.
        let json = r#"{"device_name": "Rubix24", "volume": 0.8, "pan": 0.0}"#;
        let route: AudioOutputRoute = serde_json::from_str(json).unwrap();
        assert_eq!(route.channel_offset, 0);

        let json = r#"{"device_name": "Rubix24", "target_volume": 1.0}"#;
        let fade: OutputFade = serde_json::from_str(json).unwrap();
        assert_eq!(fade.channel_offset, 0);
    }

    #[cfg(feature = "audio")]
    #[test]
    fn zero_channel_offset_is_omitted_from_show_files() {
        // Keeps untouched show files byte-identical.
        let json = serde_json::to_string(&AudioOutputRoute::default()).unwrap();
        assert!(!json.contains("channel_offset"));

        let route = AudioOutputRoute { channel_offset: 2, ..Default::default() };
        let json = serde_json::to_string(&route).unwrap();
        assert!(json.contains("\"channel_offset\":2"));
    }
}
