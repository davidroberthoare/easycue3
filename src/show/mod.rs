//! Show file management and persistence
//!
//! Handles saving/loading show files with cues, fixtures, and settings.
//! Old show files (pre-Phase B) used separate "cues" and "audio_cues" lists with a flat
//! format; load() auto-migrates them into the unified format.

use anyhow::Result;
use crate::cue::Cue;
use crate::fixtures::Patch;
use crate::groups::GroupList;
use crate::magic_sheet::MagicSheet;
use serde::{Deserialize, Serialize};

/// RGBA color for show-file persisted UI settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// User-configurable cue colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueColorSettings {
    /// Bright color used while a cue is actively fading/changing.
    pub status_fading: RgbaColor,
    /// Active color for light/sound cues that are currently playing.
    pub status_active: RgbaColor,
    /// On-deck color for the cue that GO/Space will fire next.
    pub status_on_deck: RgbaColor,
    /// Idle/base color for lighting cues.
    pub base_lighting: RgbaColor,
    /// Idle/base color for audio cues.
    pub base_audio: RgbaColor,
    /// Idle/base color for adjust cues.
    pub base_adjust: RgbaColor,
}

impl Default for CueColorSettings {
    fn default() -> Self {
        Self {
            // Hardcoded fallbacks for missing/older show files.
            status_fading:  RgbaColor::rgb(200, 160, 30),
            status_active:  RgbaColor::rgb(175, 45, 45),
            status_on_deck: RgbaColor::rgb(45, 135, 45),
            base_lighting:  RgbaColor::rgb(150, 85, 170),
            base_audio:     RgbaColor::rgb(65, 95, 190),
            base_adjust:    RgbaColor::rgb(100, 100, 110),
        }
    }
}

/// Show file format — unified cue list (lighting + audio together)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowFile {
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

    /// Lighting groups (fixture selection shortcuts)
    #[serde(default)]
    pub groups: GroupList,

    /// Magic sheet canvas layout
    #[serde(default)]
    pub magic_sheet: MagicSheet,

    /// UI color settings for cue status/base colors.
    #[serde(default)]
    pub cue_colors: CueColorSettings,

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
    pub fn new() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            description: String::new(),
            created: now.clone(),
            modified: now,
            next_cue_id: 1,
            cues: Vec::new(),
            patch: Vec::new(),
            groups: GroupList::default(),
            magic_sheet: MagicSheet::default(),
            cue_colors: CueColorSettings::default(),
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

    /// Load from a JSON file
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut show: ShowFile = serde_json::from_str(&json)?;
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

