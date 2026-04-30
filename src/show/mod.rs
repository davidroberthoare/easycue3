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
        let show: ShowFile = serde_json::from_str(&json)?;
        log::info!("Loaded show from {:?}", path);
        Ok(show)
    }
}
