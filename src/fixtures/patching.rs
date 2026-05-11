//! Fixture patching system
//!
//! Maps fixture instances to DMX addresses.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A patched fixture instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Unique fixture ID (auto-assigned)
    pub id: usize,
    /// User-assigned fixture number/label (e.g., "RGB #1", "Moving Light 3")
    pub label: String,
    /// Profile ID this fixture uses
    pub profile_id: String,
    /// Starting DMX address (1-512, 1-indexed)
    pub start_address: u16,
    /// Universe number (1-based, default 1)
    #[serde(default = "default_universe")]
    pub universe: u16,
    /// Optional user notes
    #[serde(default)]
    pub notes: String,
}

fn default_universe() -> u16 {
    1
}

impl Patch {
    /// Create a new patch
    pub fn new(id: usize, label: String, profile_id: String, start_address: u16) -> Self {
        Self {
            id,
            label,
            profile_id,
            start_address,
            universe: 1,
            notes: String::new(),
        }
    }

    /// Get the ending DMX address for this fixture (inclusive)
    pub fn end_address(&self, channel_count: u16) -> u16 {
        self.start_address + channel_count - 1
    }

    /// Check if this patch uses a specific DMX channel
    pub fn uses_channel(&self, channel: u16, channel_count: u16) -> bool {
        channel >= self.start_address && channel <= self.end_address(channel_count)
    }

    /// Get the channel offset for a fixture parameter
    /// Returns the absolute DMX channel number (1-indexed)
    #[allow(dead_code)]
    pub fn get_channel_for_offset(&self, offset: u16) -> u16 {
        self.start_address + offset
    }
}

/// Collection of patched fixtures
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchList {
    patches: Vec<Patch>,
    next_id: usize,
}

impl PatchList {
    /// Create a new empty patch list
    pub fn new() -> Self {
        Self {
            patches: Vec::new(),
            next_id: 1,
        }
    }

    /// Remove all patches and reset the ID counter
    pub fn clear(&mut self) {
        self.patches.clear();
        self.next_id = 1;
    }

    /// Add a new fixture patch on the given universe (1-based, default 1).
    pub fn add_patch(
        &mut self,
        label: String,
        profile_id: String,
        start_address: u16,
        universe: u16,
        channel_count: u16,
        channel_counts: &HashMap<String, u16>,
    ) -> Result<usize> {
        // Validate address range
        if start_address == 0 || start_address > 512 {
            return Err(anyhow!(
                "Invalid start address {}: must be between 1 and 512",
                start_address
            ));
        }

        let end_address = start_address + channel_count - 1;
        if end_address > 512 {
            return Err(anyhow!(
                "Fixture extends beyond channel 512 (start: {}, count: {}, end: {})",
                start_address,
                channel_count,
                end_address
            ));
        }

        // Check for overlaps with existing patches in the same universe
        if let Some(conflict) = self.find_overlap(start_address, channel_count, universe, None, channel_counts) {
            let conflict_channel_count = channel_counts.get(&conflict.profile_id).copied().unwrap_or(1);
            return Err(anyhow!(
                "Address range {}-{} overlaps with fixture '{}' ({}-{}) in universe {}",
                start_address,
                end_address,
                conflict.label,
                conflict.start_address,
                conflict.end_address(conflict_channel_count),
                universe,
            ));
        }

        let id = self.next_id;
        self.next_id += 1;

        self.add_patch_with_explicit_id(id, label, profile_id, start_address, end_address, universe)
    }

    /// Add a patch with a caller-supplied fixture ID. Checks that the ID is not already used.
    pub fn add_patch_with_id(
        &mut self,
        fixture_id: usize,
        label: String,
        profile_id: String,
        start_address: u16,
        universe: u16,
        channel_count: u16,
        channel_counts: &HashMap<String, u16>,
    ) -> Result<usize> {
        if start_address == 0 || start_address > 512 {
            return Err(anyhow!("Invalid start address {}: must be between 1 and 512", start_address));
        }
        let end_address = start_address + channel_count - 1;
        if end_address > 512 {
            return Err(anyhow!("Fixture extends beyond channel 512 (end: {})", end_address));
        }
        if self.patches.iter().any(|p| p.id == fixture_id) {
            return Err(anyhow!("Fixture number {} is already in use", fixture_id));
        }
        if let Some(conflict) = self.find_overlap(start_address, channel_count, universe, None, channel_counts) {
            let cc = channel_counts.get(&conflict.profile_id).copied().unwrap_or(1);
            return Err(anyhow!(
                "Address range {}-{} overlaps with fixture '{}' ({}-{}) in universe {}",
                start_address, end_address, conflict.label,
                conflict.start_address, conflict.end_address(cc), universe,
            ));
        }
        self.next_id = self.next_id.max(fixture_id + 1);
        self.add_patch_with_explicit_id(fixture_id, label, profile_id, start_address, end_address, universe)
    }

    fn add_patch_with_explicit_id(
        &mut self,
        id: usize,
        label: String,
        profile_id: String,
        start_address: u16,
        end_address: u16,
        universe: u16,
    ) -> Result<usize> {
        let mut patch = Patch::new(id, label, profile_id, start_address);
        patch.universe = universe;
        self.patches.push(patch);
        log::info!("Patched fixture #{} at U{}:{}-{}", id, universe, start_address, end_address);
        Ok(id)
    }

    /// Returns the lowest positive integer not yet used as a fixture ID.
    pub fn next_available_id(&self) -> usize {
        let mut id = 1usize;
        let mut used: Vec<usize> = self.patches.iter().map(|p| p.id).collect();
        used.sort_unstable();
        for used_id in &used {
            if *used_id == id { id += 1; } else { break; }
        }
        id
    }

    /// Remove a patch by ID
    pub fn remove_patch(&mut self, id: usize) -> Result<()> {
        let index = self
            .patches
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| anyhow!("Fixture #{} not found", id))?;

        let patch = self.patches.remove(index);
        log::info!("Removed fixture #{} ({})", id, patch.label);

        Ok(())
    }

    /// Change a fixture's ID, checking that the new ID is not already in use.
    pub fn rename_id(&mut self, old_id: usize, new_id: usize) -> Result<()> {
        if new_id == 0 {
            anyhow::bail!("Fixture number must be ≥ 1");
        }
        if self.patches.iter().any(|p| p.id == new_id) {
            anyhow::bail!("Fixture number {} is already in use", new_id);
        }
        let patch = self.patches.iter_mut()
            .find(|p| p.id == old_id)
            .ok_or_else(|| anyhow::anyhow!("Fixture {} not found", old_id))?;
        patch.id = new_id;
        self.next_id = self.next_id.max(new_id + 1);
        self.patches.sort_by_key(|p| p.id);
        Ok(())
    }

    /// Get a patch by ID
    pub fn get_patch(&self, id: usize) -> Option<&Patch> {
        self.patches.iter().find(|p| p.id == id)
    }

    /// Get a mutable patch by ID
    pub fn get_patch_mut(&mut self, id: usize) -> Option<&mut Patch> {
        self.patches.iter_mut().find(|p| p.id == id)
    }

    /// Find which fixture (if any) is patched to a specific DMX channel
    pub fn find_patch_at_channel(
        &self,
        channel: u16,
        channel_counts: &HashMap<String, u16>,
    ) -> Option<&Patch> {
        self.patches.iter().find(|p| {
            let channel_count = channel_counts.get(&p.profile_id).copied().unwrap_or(1);
            p.uses_channel(channel, channel_count)
        })
    }

    /// Find overlapping patch within the same universe (excluding patch with given ID).
    fn find_overlap(
        &self,
        start_address: u16,
        channel_count: u16,
        universe: u16,
        exclude_id: Option<usize>,
        channel_counts: &HashMap<String, u16>,
    ) -> Option<&Patch> {
        let end_address = start_address + channel_count - 1;

        self.patches.iter().find(|p| {
            if let Some(id) = exclude_id {
                if p.id == id { return false; }
            }
            // Only overlaps matter within the same universe
            if p.universe != universe { return false; }
            let p_channel_count = channel_counts.get(&p.profile_id).copied().unwrap_or(1);
            let p_end = p.start_address + p_channel_count - 1;
            !(end_address < p.start_address || start_address > p_end)
        })
    }

    /// Get all patches
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Get number of patched fixtures
    pub fn len(&self) -> usize {
        self.patches.len()
    }

    /// Check if patch list is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// Update a patch's address (with universe-aware overlap validation).
    pub fn update_patch_address(
        &mut self,
        id: usize,
        new_start_address: u16,
        channel_count: u16,
        channel_counts: &HashMap<String, u16>,
    ) -> Result<()> {
        // Validate new address
        if new_start_address == 0 || new_start_address > 512 {
            return Err(anyhow!(
                "Invalid address {}: must be between 1 and 512",
                new_start_address
            ));
        }

        let end_address = new_start_address + channel_count - 1;
        if end_address > 512 {
            return Err(anyhow!(
                "Fixture would extend beyond channel 512 (start: {}, count: {}, end: {})",
                new_start_address,
                channel_count,
                end_address
            ));
        }

        // Fetch the universe of this patch before the mutable borrow.
        let universe = self.patches.iter().find(|p| p.id == id).map(|p| p.universe).unwrap_or(1);

        // Check for overlaps within the same universe (excluding this patch)
        if let Some(conflict) = self.find_overlap(new_start_address, channel_count, universe, Some(id), channel_counts) {
            let conflict_channel_count = channel_counts.get(&conflict.profile_id).copied().unwrap_or(1);
            return Err(anyhow!(
                "New address range {}-{} overlaps with fixture '{}' ({}-{})",
                new_start_address,
                end_address,
                conflict.label,
                conflict.start_address,
                conflict.end_address(conflict_channel_count)
            ));
        }

        // Update the address
        let patch = self
            .get_patch_mut(id)
            .ok_or_else(|| anyhow!("Fixture #{} not found", id))?;

        patch.start_address = new_start_address;
        log::info!(
            "Updated fixture #{} address to U{}:{}-{}",
            id, universe, new_start_address, end_address
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_address_range() {
        let patch = Patch::new(1, "RGB #1".to_string(), "rgb".to_string(), 10);

        // RGB has 3 channels
        assert_eq!(patch.end_address(3), 12);
        assert!(patch.uses_channel(10, 3));
        assert!(patch.uses_channel(11, 3));
        assert!(patch.uses_channel(12, 3));
        assert!(!patch.uses_channel(9, 3));
        assert!(!patch.uses_channel(13, 3));
    }

    #[test]
    fn test_add_patch() {
        let mut patch_list = PatchList::new();
        let mut channel_counts = HashMap::new();
        channel_counts.insert("rgb".to_string(), 3);

        // Add valid patch on universe 1
        let id = patch_list
            .add_patch("RGB #1".to_string(), "rgb".to_string(), 10, 1, 3, &channel_counts)
            .unwrap();
        assert_eq!(id, 1);
        assert_eq!(patch_list.len(), 1);

        // Try to add overlapping patch in the same universe (should fail)
        let result = patch_list.add_patch(
            "RGB #2".to_string(),
            "rgb".to_string(),
            11,
            1,
            3,
            &channel_counts,
        );
        assert!(result.is_err());

        // Same address on a different universe (should succeed)
        let id_u2 = patch_list
            .add_patch("RGB #U2".to_string(), "rgb".to_string(), 11, 2, 3, &channel_counts)
            .unwrap();
        assert_eq!(id_u2, 2);

        // Add non-overlapping patch on universe 1 (should succeed)
        let id2 = patch_list
            .add_patch("RGB #2".to_string(), "rgb".to_string(), 20, 1, 3, &channel_counts)
            .unwrap();
        assert_eq!(id2, 3);
        assert_eq!(patch_list.len(), 3);
    }

    #[test]
    fn test_find_patch_at_channel() {
        let mut patch_list = PatchList::new();
        let mut channel_counts = HashMap::new();
        channel_counts.insert("rgb".to_string(), 3);
        patch_list
            .add_patch("RGB #1".to_string(), "rgb".to_string(), 10, 1, 3, &channel_counts)
            .unwrap();

        // Find patch at channels 10-12
        assert!(patch_list.find_patch_at_channel(10, &channel_counts).is_some());
        assert!(patch_list.find_patch_at_channel(11, &channel_counts).is_some());
        assert!(patch_list.find_patch_at_channel(12, &channel_counts).is_some());

        // No patch at these channels
        assert!(patch_list.find_patch_at_channel(9, &channel_counts).is_none());
        assert!(patch_list.find_patch_at_channel(13, &channel_counts).is_none());
    }

    #[test]
    fn test_remove_patch() {
        let mut patch_list = PatchList::new();
        let mut channel_counts = HashMap::new();
        channel_counts.insert("rgb".to_string(), 3);
        let id = patch_list
            .add_patch("RGB #1".to_string(), "rgb".to_string(), 10, 1, 3, &channel_counts)
            .unwrap();

        assert_eq!(patch_list.len(), 1);

        patch_list.remove_patch(id).unwrap();
        assert_eq!(patch_list.len(), 0);

        // Try to remove non-existent patch
        assert!(patch_list.remove_patch(999).is_err());
    }

    #[test]
    fn test_overlap_uses_existing_fixture_width() {
        let mut patch_list = PatchList::new();

        let mut channel_counts = HashMap::new();
        channel_counts.insert("irgb".to_string(), 4);
        channel_counts.insert("dimmer".to_string(), 1);

        patch_list
            .add_patch("iRGB #1".to_string(), "irgb".to_string(), 1, 1, 4, &channel_counts)
            .unwrap();

        // 4 is inside iRGB #1 range (1-4) in universe 1, so this dimmer patch must fail.
        let result = patch_list.add_patch(
            "Dimmer #1".to_string(),
            "dimmer".to_string(),
            4,
            1,
            1,
            &channel_counts,
        );
        assert!(result.is_err());

        // Same address on universe 2 must succeed.
        let result2 = patch_list.add_patch(
            "Dimmer #U2".to_string(),
            "dimmer".to_string(),
            4,
            2,
            1,
            &channel_counts,
        );
        assert!(result2.is_ok());
    }
}
