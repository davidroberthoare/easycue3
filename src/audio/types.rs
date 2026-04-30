//! Audio cue data structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single audio cue containing playback settings and timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCue {
    /// Cue number (e.g., 1.0, 1.5, 2.0) - matches lighting cue numbering
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub number: f32,
    
    /// Optional text label
    pub label: String,
    
    /// Path to audio file
    /// - If relative (e.g., "sample1.mp3"), automatically resolved from the "media/" directory
    /// - If absolute, used as-is
    /// - When saving, paths in the media/ directory are automatically simplified to just the filename
    pub audio_path: PathBuf,
    
    /// Volume level (0.0 to 1.0)
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub volume: f32,
    
    /// Fade in time in seconds
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_in: f32,
    
    /// Fade out time in seconds
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_out: f32,
    
    /// Notes/description
    pub notes: String,
    
    /// Optional lighting cue number to trigger when this audio cue starts
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub triggers_lighting_cue: Option<f32>,
}

impl AudioCue {
    /// Create a new audio cue with default settings
    pub fn new(number: f32, audio_path: PathBuf) -> Self {
        Self {
            number,
            label: String::new(),
            audio_path,
            volume: 0.8,  // Default 80% volume
            fade_in: 0.0,  // No fade by default
            fade_out: 0.0,
            notes: String::new(),
            triggers_lighting_cue: None,
        }
    }
    
    /// Create an audio cue with a label
    pub fn with_label(number: f32, audio_path: PathBuf, label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::new(number, audio_path)
        }
    }
    
    /// Get the filename for display
    pub fn filename(&self) -> String {
        self.audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string()
    }
    
    /// Canonicalize the audio path for saving
    /// Strips the "media/" prefix if present, so files in the media directory
    /// are saved as just the filename (e.g., "sample1.mp3" instead of "media/sample1.mp3")
    pub fn canonical_path(&self) -> PathBuf {
        // Check if path starts with "media/"
        if let Ok(stripped) = self.audio_path.strip_prefix("media") {
            stripped.to_path_buf()
        } else {
            self.audio_path.clone()
        }
    }
    
    /// Resolve the audio path to an actual filesystem path
    /// Falls back to the "media/" directory if the path doesn't exist as-is
    pub fn resolved_path(&self) -> PathBuf {
        // If path is absolute or exists as-is, use it
        if self.audio_path.is_absolute() || self.audio_path.exists() {
            return self.audio_path.clone();
        }
        
        // Try prepending "media/" directory
        let media_path = PathBuf::from("media").join(&self.audio_path);
        if media_path.exists() {
            return media_path;
        }
        
        // Fall back to original path
        self.audio_path.clone()
    }
    
    /// Set the audio path from a full path, automatically simplifying if it's in the media directory
    pub fn set_path(&mut self, path: PathBuf) {
        // If the path is in the media directory, strip the prefix for cleaner show files
        if let Ok(stripped) = path.strip_prefix("media") {
            self.audio_path = stripped.to_path_buf();
        } else {
            self.audio_path = path;
        }
    }
}

/// Current state of audio cue playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCueState {
    /// Not playing
    Stopped,
    
    /// Currently fading in
    FadingIn {
        /// Progress from 0.0 to 1.0
        progress: f32,
    },
    
    /// Playing at full volume
    Playing,
    
    /// Currently fading out
    FadingOut {
        /// Progress from 0.0 to 1.0 (0.0 = full volume, 1.0 = silent)
        progress: f32,
    },
}

impl Default for AudioCueState {
    fn default() -> Self {
        Self::Stopped
    }
}
