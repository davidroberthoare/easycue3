//! Audio cue list management

use crate::audio::AudioCue;
use anyhow::Result;

/// Manages a list of audio cues
#[derive(Debug, Clone, Default)]
pub struct AudioCueList {
    cues: Vec<AudioCue>,
    current_index: Option<usize>,
}

impl AudioCueList {
    /// Create a new empty audio cue list
    pub fn new() -> Self {
        Self {
            cues: Vec::new(),
            current_index: None,
        }
    }
    
    /// Add an audio cue to the list
    pub fn add_cue(&mut self, cue: AudioCue) {
        // Insert in sorted order by cue number
        let insert_pos = self.cues
            .binary_search_by(|c| c.number.partial_cmp(&cue.number).unwrap())
            .unwrap_or_else(|e| e);
        self.cues.insert(insert_pos, cue);
    }
    
    /// Remove an audio cue by index
    pub fn remove_cue(&mut self, index: usize) -> Result<AudioCue> {
        if index >= self.cues.len() {
            anyhow::bail!("Audio cue index {} out of range", index);
        }
        
        // Adjust current index if needed
        if let Some(current) = self.current_index {
            if index < current {
                self.current_index = Some(current - 1);
            } else if index == current {
                self.current_index = None;
            }
        }
        
        Ok(self.cues.remove(index))
    }
    
    /// Get an audio cue by index
    pub fn get_cue(&self, index: usize) -> Option<&AudioCue> {
        self.cues.get(index)
    }
    
    /// Get a mutable reference to an audio cue by index
    pub fn get_cue_mut(&mut self, index: usize) -> Option<&mut AudioCue> {
        self.cues.get_mut(index)
    }
    
    /// Get all audio cues
    pub fn cues(&self) -> &[AudioCue] {
        &self.cues
    }
    
    /// Get the number of audio cues
    pub fn len(&self) -> usize {
        self.cues.len()
    }
    
    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }
    
    /// Get the current cue index
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    
    /// Set the current cue index
    pub fn set_current_index(&mut self, index: Option<usize>) {
        self.current_index = index;
    }
    
    /// Get the next cue index (for GO command)
    pub fn next_index(&self) -> Option<usize> {
        match self.current_index {
            None if !self.cues.is_empty() => Some(0),
            Some(idx) if idx + 1 < self.cues.len() => Some(idx + 1),
            _ => None,
        }
    }
    
    /// Get the previous cue index (for BACK command)
    pub fn previous_index(&self) -> Option<usize> {
        match self.current_index {
            Some(idx) if idx > 0 => Some(idx - 1),
            _ => None,
        }
    }
    
    /// Clear all audio cues
    pub fn clear(&mut self) {
        self.cues.clear();
        self.current_index = None;
    }
    
    /// Load cues from a vector (used when loading show files)
    pub fn load_cues(&mut self, cues: Vec<AudioCue>) {
        self.cues = cues;
        // Sort by cue number
        self.cues.sort_by(|a, b| a.number.partial_cmp(&b.number).unwrap());
        self.current_index = None;
    }
}
