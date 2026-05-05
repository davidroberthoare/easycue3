//! Show file management and persistence
//!
//! Handles saving/loading show files with cues, fixtures, and settings.
//! Old show files (pre-Phase B) used separate "cues" and "audio_cues" lists with a flat
//! format; load() auto-migrates them into the unified format.

use anyhow::Result;
use crate::cue::Cue;
use crate::fixtures::Patch;
use crate::magic_sheet::MagicSheet;
use serde::{Deserialize, Serialize};

/// Show file format — unified cue list (lighting + audio together)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowFile {
    pub title: String,
    pub description: String,
    pub created: String,
    pub modified: String,

    /// Next cue ID to assign — ensures IDs are never reused across save/load cycles
    #[serde(default)]
    pub next_cue_id: u32,

    /// Unified cue list (lighting and audio cues interleaved, sorted by number)
    pub cues: Vec<Cue>,

    /// Fixture patch
    #[serde(default)]
    pub patch: Vec<Patch>,

    /// Magic sheet canvas layout
    #[serde(default)]
    pub magic_sheet: MagicSheet,

    /// Legacy audio cues field — only populated in old (pre-Phase B) show files.
    /// load() migrates these into `cues` and this field is always empty on save.
    #[cfg(feature = "audio")]
    #[serde(default)]
    pub audio_cues: Vec<crate::audio::AudioCue>,

    #[cfg(not(feature = "audio"))]
    #[serde(skip)]
    audio_cues: Vec<()>,
}

impl ShowFile {
    /// Create a new empty show
    pub fn new(title: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            title: title.into(),
            description: String::new(),
            created: now.clone(),
            modified: now,
            next_cue_id: 1,
            cues: Vec::new(),
            patch: Vec::new(),
            magic_sheet: MagicSheet::default(),
            #[cfg(feature = "audio")]
            audio_cues: Vec::new(),
            #[cfg(not(feature = "audio"))]
            audio_cues: Vec::new(),
        }
    }

    /// Save to a JSON file
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let modified = chrono::Utc::now().to_rfc3339();
        let mut doc = serde_json::to_value(self)?;
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("modified".to_string(), serde_json::Value::String(modified));
        }
        let json = serde_json::to_string_pretty(&doc)?;
        std::fs::write(path, json)?;
        log::info!("Saved show to {:?}", path);
        Ok(())
    }

    /// Load from a JSON file, auto-migrating old format if needed
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let raw: serde_json::Value = serde_json::from_str(&json)?;

        // Detect old format: cues in the file don't have a "kind" field
        let needs_migration = raw["cues"]
            .as_array()
            .and_then(|arr| arr.first())
            .map(|c| c.get("kind").is_none())
            .unwrap_or(false);

        let raw = if needs_migration {
            log::info!("Migrating show file from old format to unified cue format");
            migrate_to_unified(raw)?
        } else {
            raw
        };

        let mut show: ShowFile = serde_json::from_value(raw)?;
        show.repair_cue_ids();
        log::info!("Loaded show from {:?}", path);
        Ok(show)
    }

    /// Assign stable IDs to any cues that are missing one (id == 0).
    fn repair_cue_ids(&mut self) {
        let max_existing = self.cues.iter().map(|c| c.id).max().unwrap_or(0);
        let mut next = max_existing.max(self.next_cue_id);
        if next == 0 {
            next = 1;
        }
        for cue in &mut self.cues {
            if cue.id == 0 {
                cue.id = next;
                next += 1;
            }
        }
        self.next_cue_id = next;
    }
}

/// Convert a pre-Phase B show file JSON into the unified cue format.
/// Old show files had flat lighting cues in "cues" and separate "audio_cues".
fn migrate_to_unified(mut raw: serde_json::Value) -> Result<serde_json::Value> {
    use serde_json::json;

    // Wrap each old lighting cue in the new CueKind::Lighting envelope
    let old_lighting = raw["cues"].as_array().cloned().unwrap_or_default();
    let mut all_cues: Vec<serde_json::Value> = old_lighting
        .into_iter()
        .map(|c| {
            json!({
                "id": c["id"],
                "number": c["number"],
                "label": c["label"],
                "kind": {
                    "type": "Lighting",
                    "data": {
                        "fade_up": c["fade_up"],
                        "fade_down": c["fade_down"],
                        "channel_values": c["channel_values"],
                        "triggers_audio_cue": c["triggers_audio_cue"],
                    }
                }
            })
        })
        .collect();

    // Wrap each old audio cue in the new CueKind::Audio envelope
    #[cfg(feature = "audio")]
    {
        let old_audio = raw["audio_cues"].as_array().cloned().unwrap_or_default();
        let migrated_audio: Vec<serde_json::Value> = old_audio
            .into_iter()
            .map(|c| {
                json!({
                    "id": c["id"],
                    "number": c["number"],
                    "label": c["label"],
                    "kind": {
                        "type": "Audio",
                        "data": {
                            "audio_path": c["audio_path"],
                            "volume": c["volume"],
                            "fade_in": c["fade_in"],
                            "fade_out": c["fade_out"],
                            "notes": c["notes"],
                            "triggers_lighting_cue": c["triggers_lighting_cue"],
                        }
                    }
                })
            })
            .collect();
        all_cues.extend(migrated_audio);
    }

    // Sort by cue number so the list is ordered after merge
    all_cues.sort_by(|a, b| {
        let na = a["number"].as_f64().unwrap_or(0.0);
        let nb = b["number"].as_f64().unwrap_or(0.0);
        na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
    });

    raw["cues"] = serde_json::Value::Array(all_cues);
    raw["audio_cues"] = serde_json::Value::Array(vec![]);

    Ok(raw)
}
