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

    /// Re-number a contiguous slice of the cue list (by current list index, so
    /// "all cues" or a number range) onto `new_start, new_start+step, …`.
    ///
    /// Care is taken with cross-references:
    /// - **Adjust cues** target an audio cue *by cue number*
    ///   (`AdjustData::target_audio_cue`); when a targeted audio cue is
    ///   renumbered, its adjust cues are rewritten to follow it.
    /// - **Script-viewer markers** reference cues by stable ID (`cue_id`), so
    ///   they need no adjustment — they automatically track renumbered cues.
    ///
    /// Collisions with cues outside the slice are rejected (with an error) so
    /// the renumber can't silently merge two cues onto one number.
    ///
    /// Returns the number of cues renumbered. The play head is preserved.
    pub fn renumber_range(&mut self, from_idx: usize, to_idx: usize, new_start: f32, step: f32) -> Result<usize> {
        if from_idx > to_idx || to_idx >= self.cues.len() {
            anyhow::bail!("Invalid renumber range");
        }
        if !new_start.is_finite() || !step.is_finite() || step <= 0.0 {
            anyhow::bail!("Start and step must be positive numbers");
        }

        let count = to_idx - from_idx + 1;
        let new_numbers: Vec<f32> = (0..count).map(|i| new_start + i as f32 * step).collect();

        // Duplicate numbers within the renumbered set (e.g. an oversized step).
        for (i, n) in new_numbers.iter().enumerate() {
            if new_numbers[..i].iter().any(|m| (m - n).abs() < 0.005) {
                anyhow::bail!("Step is too small: numbers collide at {:.1}", n);
            }
        }
        // Collisions with cues left outside the renumbered slice.
        for cue in self.cues.iter().take(from_idx).chain(self.cues.iter().skip(to_idx + 1)) {
            if let Some(n) = new_numbers.iter().find(|n| (cue.number - **n).abs() < 0.005) {
                anyhow::bail!(
                    "New number {:.1} collides with cue {:.1} outside the renumbered range",
                    n,
                    cue.number
                );
            }
        }

        // Rewrite adjust-cue targets: old number → new number for audio cues
        // that were inside the slice.
        #[cfg(feature = "audio")]
        {
            let old_to_new: Vec<(f32, f32)> = self.cues[from_idx..=to_idx]
                .iter()
                .enumerate()
                .filter(|(_, c)| c.is_audio())
                .map(|(i, c)| (c.number, new_numbers[i]))
                .collect();

            for cue in self.cues.iter_mut() {
                if let Some(d) = cue.adjust_data_mut() {
                    if let Some(target) = d.target_audio_cue {
                        if let Some((_, new_num)) = old_to_new.iter().find(|(old, _)| (old - target).abs() < 0.005) {
                            d.target_audio_cue = Some(*new_num);
                        }
                    }
                }
            }
        }

        // Apply the new numbers.
        for (i, cue) in self.cues.iter_mut().enumerate() {
            if (from_idx..=to_idx).contains(&i) {
                cue.number = new_numbers[i - from_idx];
            }
        }

        // Re-sort by number, preserving the play head.
        let current_id = self.current.and_then(|i| self.cues.get(i)).map(|c| c.id);
        self.cues.sort_by(|a, b| a.number.partial_cmp(&b.number).unwrap_or(std::cmp::Ordering::Equal));
        self.current = current_id.and_then(|id| self.cues.iter().position(|c| c.id == id));

        Ok(count)
    }

    /// Re-number every cue whose number falls in `[from_num, to_num]`
    /// (inclusive). The list is sorted by number, so this selects a contiguous
    /// slice and delegates to [`Self::renumber_range`].
    pub fn renumber_range_for_numbers(&mut self, from_num: f32, to_num: f32, new_start: f32, step: f32) -> Result<usize> {
        if to_num < from_num {
            anyhow::bail!("Range start {:.1} is after range end {:.1}", from_num, to_num);
        }
        let from_idx = self
            .cues
            .iter()
            .position(|c| c.number >= from_num - 0.005)
            .ok_or_else(|| anyhow::anyhow!("No cue at or above {:.1}", from_num))?;
        let to_idx = self
            .cues
            .iter()
            .rposition(|c| c.number <= to_num + 0.005)
            .ok_or_else(|| anyhow::anyhow!("No cue at or below {:.1}", to_num))?;
        self.renumber_range(from_idx, to_idx, new_start, step)
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

    /// Replay effect actions of lighting cues 0..=idx to produce the set of
    /// effects that should be running at that point: (effect_id, fixture IDs),
    /// in first-start order. The effect-state analogue of `tracked_state_up_to`.
    pub fn effect_state_up_to(&self, idx: usize) -> Vec<(u32, Vec<usize>)> {
        use crate::effects::EffectAction;
        let mut state: Vec<(u32, Vec<usize>)> = Vec::new();
        for cue in self.cues.iter().take(idx + 1) {
            if let CueKind::Lighting(data) = &cue.kind {
                for action in &data.effect_actions {
                    match action {
                        EffectAction::Start { effect_id, fixtures } => {
                            if let Some(entry) = state.iter_mut().find(|(id, _)| id == effect_id) {
                                entry.1 = fixtures.clone();
                            } else {
                                state.push((*effect_id, fixtures.clone()));
                            }
                        }
                        EffectAction::Stop { effect_id } => {
                            state.retain(|(id, _)| id != effect_id);
                        }
                        EffectAction::StopAll => state.clear(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn lighting_list(numbers: &[f32]) -> CueList {
        let mut list = CueList::new();
        for n in numbers {
            list.add_cue(Cue::new_lighting(*n));
        }
        list
    }

    fn numbers_of(list: &CueList) -> Vec<f32> {
        list.cues().iter().map(|c| c.number).collect()
    }

    #[test]
    fn renumber_all_reassigns_sequentially() {
        let mut list = lighting_list(&[3.0, 1.5, 2.0]);
        let n = list.renumber_range(0, 2, 1.0, 1.0).unwrap();
        assert_eq!(n, 3);
        assert_eq!(numbers_of(&list), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn renumber_respects_custom_start_and_step() {
        let mut list = lighting_list(&[1.0, 2.0, 3.0, 4.0]);
        // Renumber cues 3..4 (indices 2..3) to start at 10 step 0.5.
        let n = list.renumber_range(2, 3, 10.0, 0.5).unwrap();
        assert_eq!(n, 2);
        assert_eq!(numbers_of(&list), vec![1.0, 2.0, 10.0, 10.5]);
    }

    #[test]
    fn renumber_range_for_numbers_selects_by_number() {
        let mut list = lighting_list(&[0.5, 0.8, 3.0, 5.0, 7.0]);
        // Cues with numbers in [3,7] → renumber to 1.0, 2.0, 3.0.
        let n = list.renumber_range_for_numbers(3.0, 7.0, 1.0, 1.0).unwrap();
        assert_eq!(n, 3);
        assert_eq!(numbers_of(&list), vec![0.5, 0.8, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn renumber_rejects_collision_outside_range() {
        let mut list = lighting_list(&[1.0, 2.0, 3.0, 4.0]);
        // Renumber cues 2..3 onto 0.5/1.0 — the 1.0 collides with the untouched
        // cue 1.0 that sits outside the slice.
        let err = list.renumber_range(1, 2, 0.5, 0.5).unwrap_err();
        assert!(err.to_string().contains("collides"));
    }

    #[test]
    fn renumber_rejects_invalid_parameters() {
        let mut list = lighting_list(&[1.0, 2.0]);
        assert!(list.renumber_range(1, 0, 1.0, 1.0).is_err());
        assert!(list.renumber_range(0, 2, 1.0, 1.0).is_err());
        assert!(list.renumber_range(0, 1, 1.0, 0.0).is_err());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn renumber_rewrites_adjust_cue_targets() {
        let mut list = CueList::new();
        list.add_cue(Cue::new_lighting(1.0));
        list.add_cue(Cue::new_audio(5.0, std::path::PathBuf::from("a.wav")));
        list.add_cue(Cue::new_audio(6.0, std::path::PathBuf::from("b.wav")));

        let mut adjust1 = Cue::new_adjust(7.0);
        if let crate::cue::CueKind::Adjust(d) = &mut adjust1.kind {
            d.target_audio_cue = Some(5.0);
        }
        let mut adjust2 = Cue::new_adjust(8.0);
        if let crate::cue::CueKind::Adjust(d) = &mut adjust2.kind {
            d.target_audio_cue = Some(6.0);
        }
        list.add_cue(adjust1);
        list.add_cue(adjust2);

        // Renumber everything 1.0 step 1.0: 1 lx, 2 audio, 3 audio, 4 adj, 5 adj.
        let n = list.renumber_range(0, list.len() - 1, 1.0, 1.0).unwrap();
        assert_eq!(n, 5);
        assert_eq!(numbers_of(&list), vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        let targets: Vec<Option<f32>> = list
            .cues()
            .iter()
            .filter(|c| c.is_adjust())
            .map(|c| c.adjust_data().and_then(|d| d.target_audio_cue))
            .collect();
        assert_eq!(targets, vec![Some(2.0), Some(3.0)]);
    }
}
