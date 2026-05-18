//! Cue list management — unified lighting and audio cues

use crate::cue::{Cue, CueKind};
use anyhow::Result;
use std::collections::HashMap;

/// Manages the unified cue list with a single shared play head
#[derive(Debug, Clone)]
pub struct CueList {
    cues: Vec<Cue>,
    /// Index of the last-fired cue (any kind); both lighting and audio GO advance from here
    current: Option<usize>,
    next_id: u32,
}

impl Default for CueList {
    fn default() -> Self {
        Self::new()
    }
}

impl CueList {
    pub fn new() -> Self {
        Self {
            cues: Vec::new(),
            current: None,
            next_id: 1,
        }
    }

    /// Add a cue, assigning a stable ID if id == 0. Inserts in sorted order by number.
    pub fn add_cue(&mut self, mut cue: Cue) {
        if cue.id == 0 {
            cue.id = self.next_id;
            self.next_id += 1;
        } else {
            self.next_id = self.next_id.max(cue.id + 1);
        }
        let insert_pos = self.cues
            .binary_search_by(|c| c.number.partial_cmp(&cue.number).unwrap())
            .unwrap_or_else(|e| e);
        self.cues.insert(insert_pos, cue);

        if let Some(cur) = self.current {
            if insert_pos <= cur {
                self.current = Some(cur + 1);
            }
        }
    }

    /// Look up a cue by its stable ID
    pub fn find_by_id(&self, id: u32) -> Option<&Cue> {
        self.cues.iter().find(|c| c.id == id)
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = self.next_id.max(id);
    }

    pub fn remove_cue(&mut self, index: usize) -> Result<Cue> {
        if index >= self.cues.len() {
            anyhow::bail!("Cue index {} out of range", index);
        }
        if let Some(cur) = self.current {
            if index < cur {
                self.current = Some(cur - 1);
            } else if index == cur {
                self.current = None;
            }
        }
        Ok(self.cues.remove(index))
    }

    pub fn get_cue(&self, index: usize) -> Option<&Cue> {
        self.cues.get(index)
    }

    pub fn get_cue_mut(&mut self, index: usize) -> Option<&mut Cue> {
        self.cues.get_mut(index)
    }

    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }

    pub fn len(&self) -> usize {
        self.cues.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }

    // --- Play head ---

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn set_current_index(&mut self, index: Option<usize>) {
        self.current = index;
    }

    // --- Kind-filtered navigation (all share the single play head) ---

    /// Next lighting cue after current (searches forward in unified list)
    pub fn next_lighting_index(&self) -> Option<usize> {
        let start = self.current.map(|i| i + 1).unwrap_or(0);
        self.cues[start..]
            .iter()
            .enumerate()
            .find(|(_, c)| c.is_lighting())
            .map(|(i, _)| start + i)
    }

    /// Previous lighting cue before current
    pub fn previous_lighting_index(&self) -> Option<usize> {
        let end = self.current?;
        if end == 0 {
            return None;
        }
        self.cues[..end]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| c.is_lighting())
            .map(|(i, _)| i)
    }

    /// Next audio cue after current (searches forward in unified list)
    #[cfg(feature = "audio")]
    pub fn next_audio_index(&self) -> Option<usize> {
        let start = self.current.map(|i| i + 1).unwrap_or(0);
        self.cues[start..]
            .iter()
            .enumerate()
            .find(|(_, c)| c.is_audio())
            .map(|(i, _)| start + i)
    }

    /// Previous audio cue before current
    #[cfg(feature = "audio")]
    pub fn previous_audio_index(&self) -> Option<usize> {
        let end = self.current?;
        if end == 0 {
            return None;
        }
        self.cues[..end]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| c.is_audio())
            .map(|(i, _)| i)
    }

    // --- Unified navigation (any kind) ---

    /// Next cue of any kind after current (sequential list order)
    pub fn next_any_index(&self) -> Option<usize> {
        let start = self.current.map(|i| i + 1).unwrap_or(0);
        if start < self.cues.len() { Some(start) } else { None }
    }

    /// Previous cue of any kind before current
    pub fn previous_any_index(&self) -> Option<usize> {
        let end = self.current?;
        if end > 0 { Some(end - 1) } else { None }
    }

    /// Change the number of a cue by stable ID, reject duplicates, re-sort the list.
    pub fn renumber_cue(&mut self, cue_id: u32, new_number: f32) -> Result<()> {
        if self.cues.iter().any(|c| c.id != cue_id && (c.number - new_number).abs() < 0.005) {
            anyhow::bail!("Cue number {:.1} is already in use", new_number);
        }
        let Some(cue) = self.cues.iter_mut().find(|c| c.id == cue_id) else {
            anyhow::bail!("Cue id {} not found", cue_id);
        };
        cue.number = new_number;

        let current_id = self.current.and_then(|i| self.cues.get(i)).map(|c| c.id);
        self.cues.sort_by(|a, b| a.number.partial_cmp(&b.number).unwrap_or(std::cmp::Ordering::Equal));
        self.current = current_id.and_then(|id| self.cues.iter().position(|c| c.id == id));
        Ok(())
    }

    // --- Tracking ---

    /// Replay all lighting cues from 0 through `idx` (inclusive) to produce the
    /// full tracked channel state at that point in the list.
    /// A channel explicitly stored as 0 in a cue means "turn this off".
    /// Channels absent from a cue track through unchanged from prior cues.
    pub fn tracked_state_up_to(&self, idx: usize) -> HashMap<u16, u8> {
        let mut state: HashMap<u16, u8> = HashMap::new();
        for cue in self.cues.iter().take(idx + 1) {
            if let CueKind::Lighting(data) = &cue.kind {
                for (&key, &value) in &data.channel_values {
                    if value == 0 {
                        state.remove(&key);
                    } else {
                        state.insert(key, value);
                    }
                }
            }
        }
        state
    }

    // --- Utility ---

    pub fn clear(&mut self) {
        self.cues.clear();
        self.current = None;
    }
}
