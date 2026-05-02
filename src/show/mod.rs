//! Show file management and persistence
//!
//! Handles saving/loading show files with cues, fixtures, and settings.

use anyhow::Result;
use crate::cue::Cue;
use crate::fixtures::Patch;
use serde::{Deserialize, Serialize};

#[cfg(feature = "audio")]
use crate::audio::AudioCue;

/// Show file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowFile {
    /// Show metadata
    pub title: String,
    pub description: String,
    pub created: String,
    pub modified: String,

    /// Next cue ID to assign — ensures IDs are never reused across save/load cycles
    #[serde(default)]
    pub next_cue_id: u32,

    /// Cue list
    pub cues: Vec<Cue>,

    /// Fixture patch
    #[serde(default)]
    pub patch: Vec<Patch>,
    
    /// Audio cues (Phase 4)
    #[cfg(feature = "audio")]
    #[serde(default)]
    pub audio_cues: Vec<AudioCue>,
    
    #[cfg(not(feature = "audio"))]
    #[serde(skip)]
    audio_cues: Vec<()>,  // Placeholder for non-audio builds

    // TODO: Media references (video, images)
    // TODO: Settings
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
            #[cfg(feature = "audio")]
            audio_cues: Vec::new(),
            #[cfg(not(feature = "audio"))]
            audio_cues: Vec::new(),
        }
    }

    /// Save to a JSON file
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let modified = chrono::Utc::now().to_rfc3339();
        // Build JSON manually with updated timestamp to avoid cloning the whole struct
        let mut doc = serde_json::to_value(self)?;
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("modified".to_string(), serde_json::Value::String(modified));
        }
        let json = serde_json::to_string_pretty(&doc)?;
        std::fs::write(path, json)?;
        log::info!("Saved show to {:?}", path);
        Ok(())
    }

    /// Load from a JSON file
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut show: ShowFile = serde_json::from_str(&json)?;
        show.repair_cue_ids();
        log::info!("Loaded show from {:?}", path);
        Ok(show)
    }

    /// Assign stable IDs to any cues that are missing one (id == 0).
    /// Guarantees all cues have a non-zero ID and next_cue_id is ahead of all of them.
    fn repair_cue_ids(&mut self) {
        // Find the highest ID already present across all cue types
        let max_lighting = self.cues.iter().map(|c| c.id).max().unwrap_or(0);
        #[cfg(feature = "audio")]
        let max_audio = self.audio_cues.iter().map(|c| c.id).max().unwrap_or(0);
        #[cfg(not(feature = "audio"))]
        let max_audio = 0u32;

        let mut next = max_lighting.max(max_audio).max(self.next_cue_id);
        if next == 0 {
            next = 1;
        }

        for cue in &mut self.cues {
            if cue.id == 0 {
                cue.id = next;
                next += 1;
            }
        }

        #[cfg(feature = "audio")]
        for cue in &mut self.audio_cues {
            if cue.id == 0 {
                cue.id = next;
                next += 1;
            }
        }

        self.next_cue_id = next;
    }
}
