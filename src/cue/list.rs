//! Cue list management

use crate::cue::Cue;
use anyhow::Result;

/// Manages a list of cues
#[derive(Debug, Clone)]
pub struct CueList {
    cues: Vec<Cue>,
    current_index: Option<usize>,
    next_id: u32,
}

impl Default for CueList {
    fn default() -> Self {
        Self::new()
    }
}

impl CueList {
    /// Create a new empty cue list
    pub fn new() -> Self {
        Self {
            cues: Vec::new(),
            current_index: None,
            next_id: 1,
        }
    }

    /// Add a cue to the list, assigning a stable ID if the cue has none (id == 0)
    pub fn add_cue(&mut self, mut cue: Cue) {
        if cue.id == 0 {
            cue.id = self.next_id;
            self.next_id += 1;
        } else {
            self.next_id = self.next_id.max(cue.id + 1);
        }
        // Insert in sorted order by cue number
        let insert_pos = self.cues
            .binary_search_by(|c| c.number.partial_cmp(&cue.number).unwrap())
            .unwrap_or_else(|e| e);
        self.cues.insert(insert_pos, cue);
    }

    /// Look up a cue by its stable ID
    pub fn find_by_id(&self, id: u32) -> Option<&Cue> {
        self.cues.iter().find(|c| c.id == id)
    }

    /// Current value of the ID counter (used when saving the show file)
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Advance the counter to at least `id`, used after loading a show file
    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = self.next_id.max(id);
    }

    /// Remove a cue by index
    pub fn remove_cue(&mut self, index: usize) -> Result<Cue> {
        if index >= self.cues.len() {
            anyhow::bail!("Cue index {} out of range", index);
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

    /// Get a cue by index
    pub fn get_cue(&self, index: usize) -> Option<&Cue> {
        self.cues.get(index)
    }

    /// Get a mutable reference to a cue by index
    pub fn get_cue_mut(&mut self, index: usize) -> Option<&mut Cue> {
        self.cues.get_mut(index)
    }

    /// Get all cues
    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }

    /// Get the number of cues
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

    /// Clear all cues
    pub fn clear(&mut self) {
        self.cues.clear();
        self.current_index = None;
    }
}
