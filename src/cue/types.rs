//! Cue data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single cue containing fixture states and timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    /// Cue number (e.g., 1.0, 1.5, 2.0)
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub number: f32,
    /// Optional text label
    pub label: String,
    /// Fade up time in seconds
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_up: f32,
    /// Fade down time in seconds (for intensity)
    #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
    pub fade_down: f32,
    /// Channel intensity values (0-100, channel number -> intensity)
    /// Only stores non-zero channels to save space
    pub channel_values: HashMap<u16, u8>,
    
    /// Optional audio cue number to trigger when this lighting cue executes (Phase 4)
    #[serde(default, serialize_with = "crate::serde_helpers::round_option_f32_2")]
    pub triggers_audio_cue: Option<f32>,
}

impl Cue {
    /// Create a new empty cue
    pub fn new(number: f32) -> Self {
        Self {
            number,
            label: String::new(),
            fade_up: 3.0,  // Default 3 second fade
            fade_down: 3.0,
            channel_values: HashMap::new(),
            triggers_audio_cue: None,
        }
    }

    /// Create a cue with a label
    pub fn with_label(number: f32, label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::new(number)
        }
    }

    /// Set a channel value
    pub fn set_channel(&mut self, channel: u16, value: u8) {
        if value > 0 {
            self.channel_values.insert(channel, value);
        } else {
            self.channel_values.remove(&channel);
        }
    }

    /// Get a channel value (returns 0 if not set)
    pub fn get_channel(&self, channel: u16) -> u8 {
        self.channel_values.get(&channel).copied().unwrap_or(0)
    }
}

/// Current state of cue playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CueState {
    /// Not playing
    Stopped,
    /// Currently fading in/out
    Fading {
        /// Progress from 0.0 to 1.0
        progress: f32,
    },
    /// Fade complete, holding
    Active,
}

impl Default for CueState {
    fn default() -> Self {
        Self::Stopped
    }
}
