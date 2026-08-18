//! Hotkey assignments — fire cues from the keyboard with Ctrl+0…Ctrl+9.
//!
//! Each of the ten keys can be assigned to a single existing cue (lighting,
//! sound, or adjust) with one of three trigger modes:
//!
//! - **Trigger** — the cue runs exactly as if GO was pressed (its fade timing is
//!   respected), but the play head / on-deck cue is left untouched and no
//!   autofollow is armed.
//! - **Hold** — the cue plays for as long as the key is held down and stops when
//!   it's released, using the cue's fade up/down times.
//! - **Latch** — the same as Hold, but the key doesn't need to stay held: the
//!   first press starts the cue, the second press stops it.
//!
//! The assignments themselves are saved with the show file (`ShowFile::hotkeys`);
//! the per-key *runtime* hold/latch state lives in [`HotkeyRuntime`] and is never
//! persisted.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How a hotkey triggers its assigned cue.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// Fire the cue with its normal timing — no play-head movement, no autofollow.
    #[default]
    Trigger,
    /// Play while held down; fade out when released.
    Hold,
    /// First press starts the cue, second press stops it.
    Latch,
}

fn is_trigger(mode: &HotkeyMode) -> bool {
    *mode == HotkeyMode::Trigger
}

/// A single key's assignment: which cue it fires and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyAssignment {
    /// Stable ID of the cue to fire (lighting, sound, or adjust). 0 = unassigned.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cue_id: u32,
    #[serde(default, skip_serializing_if = "is_trigger")]
    pub mode: HotkeyMode,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl HotkeyAssignment {
    pub fn is_empty(&self) -> bool {
        self.cue_id == 0
    }
}

impl Default for HotkeyAssignment {
    fn default() -> Self {
        Self {
            cue_id: 0,
            mode: HotkeyMode::Trigger,
        }
    }
}

/// Table of ten hotkeys. Index 0 = Ctrl+0 … index 9 = Ctrl+9.
///
/// Deserializes from show files that predate the feature (serde default) and
/// normalizes a hand-edited shorter vec back to ten entries (missing keys are
/// unassigned).
#[derive(Debug, Clone, Serialize)]
pub struct HotkeyMap {
    pub keys: Vec<HotkeyAssignment>,
}

impl<'de> Deserialize<'de> for HotkeyMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            keys: Vec<HotkeyAssignment>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let mut keys = raw.keys;
        keys.resize(10, HotkeyAssignment::default());
        Ok(HotkeyMap { keys })
    }
}

impl Default for HotkeyMap {
    fn default() -> Self {
        Self {
            keys: (0..10).map(|_| HotkeyAssignment::default()).collect(),
        }
    }
}

impl HotkeyMap {
    pub fn get(&self, index: usize) -> Option<&HotkeyAssignment> {
        self.keys.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut HotkeyAssignment> {
        self.keys.get_mut(index)
    }

    /// True when every key is unassigned — used to omit the field from show files.
    pub fn is_empty(&self) -> bool {
        self.keys.iter().all(|a| a.is_empty())
    }
}

/// Runtime hold/latch state for the hotkeys (never persisted).
#[derive(Debug, Default)]
pub struct HotkeyRuntime {
    /// Whether the physical key is currently down. Used for edge detection so
    /// OS key auto-repeat can't re-fire a hold or toggle a latch repeatedly.
    pub key_down: [bool; 10],
    /// Whether a hold/latch engagement is currently active for each key.
    pub engaged: [bool; 10],
    /// Per-key pre-press lighting snapshot (in `universe_key` -> value form) so
    /// releasing a held/latched lighting cue fades back to where the stage was
    /// before the key went down. `None` when the key isn't holding lighting.
    pub light_before: [Option<HashMap<u16, u8>>; 10],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_is_empty_and_has_ten_keys() {
        let map = HotkeyMap::default();
        assert_eq!(map.keys.len(), 10);
        assert!(map.is_empty());
        for a in &map.keys {
            assert!(a.is_empty());
            assert_eq!(a.mode, HotkeyMode::Trigger);
        }
    }

    #[test]
    fn empty_assignments_omit_fields_in_json() {
        let json = serde_json::to_string(&HotkeyAssignment::default()).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn assignment_round_trips_through_json() {
        let mut a = HotkeyAssignment::default();
        a.cue_id = 7;
        a.mode = HotkeyMode::Hold;
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"cue_id\":7"));
        assert!(json.contains("\"mode\":\"hold\""));
        let back: HotkeyAssignment = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn map_loads_when_section_missing_or_shorter() {
        // Older show files have no "hotkeys" field.
        let map: HotkeyMap = serde_json::from_str("{}").unwrap();
        assert!(map.is_empty());
        // A hand-edited shorter vec still deserializes; missing keys are unassigned.
        let map: HotkeyMap =
            serde_json::from_str(r#"{"keys":[{"cue_id":3,"mode":"latch"}]}"#).unwrap();
        assert_eq!(map.get(0).unwrap().cue_id, 3);
        assert_eq!(map.get(0).unwrap().mode, HotkeyMode::Latch);
        assert!(map.get(1).unwrap().is_empty());
        assert!(map.get(9).unwrap().is_empty());
    }
}
