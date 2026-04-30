//! Audio cue data structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single audio cue containing playback settings and timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCue {
    /// Cue number (e.g., 1.0, 1.5, 2.0) - matches lighting cue numbering
    pub number: f32,
    
    /// Optional text label
    pub label: String,
    
    /// Path to audio file (relative to show file directory)
    pub audio_path: PathBuf,
    
    /// Volume level (0.0 to 1.0)
    pub volume: f32,
    
    /// Fade in time in seconds
    pub fade_in: f32,
    
    /// Fade out time in seconds
    pub fade_out: f32,
    
    /// Notes/description
    pub notes: String,
    
    /// Optional lighting cue number to trigger when this audio cue starts
    #[serde(default)]
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
