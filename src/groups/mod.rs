//! Lighting groups — named collections of fixture IDs used for quick selection.
//!
//! Groups only store fixture references; levels are never saved in a group.
//! They exist purely to let an operator select multiple fixtures in one command
//! (e.g. `g1@50`) or by clicking a group shape on the magic sheet.

use serde::{Deserialize, Serialize};

/// A named set of fixture IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u32,
    pub label: String,
    /// Ordered list of fixture IDs belonging to this group.
    pub fixture_ids: Vec<usize>,
}

impl Group {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            label: String::new(),
            fixture_ids: Vec::new(),
        }
    }

    /// Format a fixture ID list as a comma-separated string, collapsing runs of
    /// 4+ consecutive IDs into ranges: "1, 2, 3, 6-9, 12, 13".
    pub fn fixtures_to_string(fixture_ids: &[usize]) -> String {
        let mut ids: Vec<usize> = fixture_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < ids.len() {
            let start = ids[i];
            let mut end = start;
            while i + 1 < ids.len() && ids[i + 1] == ids[i] + 1 {
                i += 1;
                end = ids[i];
            }
            if end - start >= 3 {
                parts.push(format!("{}-{}", start, end));
            } else {
                for id in start..=end {
                    parts.push(id.to_string());
                }
            }
            i += 1;
        }
        parts.join(", ")
    }

    /// Parse a comma-separated string like "1, 2, 3" into fixture IDs.
    /// Ranges are supported: "6-9" expands to 6, 7, 8, 9.
    /// Invalid tokens and zeroes are silently skipped.
    pub fn parse_fixtures_string(s: &str) -> Vec<usize> {
        let mut ids: Vec<usize> = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((start, end)) = part.split_once('-') {
                let start: usize = match start.trim().parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let end: usize = match end.trim().parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if start >= 1 && end >= start {
                    for id in start..=end {
                        ids.push(id);
                    }
                }
                continue;
            }
            if let Ok(id) = part.parse::<usize>() {
                if id >= 1 {
                    ids.push(id);
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// Collection of all groups in a show.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupList {
    #[serde(default)]
    pub groups: Vec<Group>,
    /// Monotonically increasing; never reused after deletion.
    #[serde(default = "default_next_id")]
    pub next_id: u32,
}

fn default_next_id() -> u32 {
    1
}

impl GroupList {
    /// Add a new empty group and return its ID.
    pub fn add_group(&mut self) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.groups.push(Group::new(id));
        id
    }

    pub fn remove_group(&mut self, id: u32) {
        self.groups.retain(|g| g.id != id);
    }

    /// Renumber a group, returning every `(old_id, new_id)` pair that changed so
    /// callers can update references (e.g. magic sheet shapes).
    ///
    /// If the target number is already taken by another group, the two groups
    /// swap ids. Does nothing (returns empty) if `old_id` doesn't exist.
    pub fn renumber(&mut self, old_id: u32, new_id: u32) -> Vec<(u32, u32)> {
        if old_id == new_id {
            return Vec::new();
        }
        let idx = match self.groups.iter().position(|g| g.id == old_id) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let mut changes = Vec::new();
        if let Some(other_idx) = self.groups.iter().position(|g| g.id == new_id) {
            self.groups[idx].id = new_id;
            self.groups[other_idx].id = old_id;
            changes.push((old_id, new_id));
            changes.push((new_id, old_id));
        } else {
            self.groups[idx].id = new_id;
            changes.push((old_id, new_id));
        }
        // Keep next_id above all current ids so future adds never collide.
        let max_id = self.groups.iter().map(|g| g.id).max().unwrap_or(0);
        if self.next_id <= max_id {
            self.next_id = max_id + 1;
        }
        changes
    }

    pub fn get_group(&self, id: u32) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn get_group_mut(&mut self, id: u32) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// Resolve a group to its fixture IDs. Returns an empty vec if the group is unknown.
    pub fn resolve_fixtures(&self, group_id: u32) -> Vec<usize> {
        self.get_group(group_id)
            .map(|g| g.fixture_ids.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ranges_and_mixed_lists() {
        assert_eq!(
            Group::parse_fixtures_string("1,2,3,6-9,12,13"),
            vec![1, 2, 3, 6, 7, 8, 9, 12, 13]
        );
        // Reversed input is normalised to ascending order.
        assert_eq!(
            Group::parse_fixtures_string("13, 6-9, 1,2,3"),
            vec![1, 2, 3, 6, 7, 8, 9, 13]
        );
        // Single IDs still parse.
        assert_eq!(Group::parse_fixtures_string("1, 2, 3"), vec![1, 2, 3]);
        // Invalid tokens and zeroes are skipped.
        assert_eq!(Group::parse_fixtures_string("1, x, 0, 5-2, 4"), vec![1, 4]);
    }

    #[test]
    fn format_compresses_runs_into_ranges() {
        assert_eq!(
            Group::fixtures_to_string(&[1, 2, 3, 6, 7, 8, 9, 12, 13]),
            "1, 2, 3, 6-9, 12, 13"
        );
        // Runs of exactly two stay flat.
        assert_eq!(Group::fixtures_to_string(&[12, 13]), "12, 13");
        // Input order is normalised.
        assert_eq!(Group::fixtures_to_string(&[5, 1, 3, 2, 4]), "1-5");
    }

    #[test]
    fn parse_and_format_round_trip() {
        let list = "1, 2, 3, 6-9, 12, 13";
        let ids = Group::parse_fixtures_string(list);
        assert_eq!(Group::fixtures_to_string(&ids), list);
    }

    #[test]
    fn renumber_to_free_id() {
        let mut list = GroupList::default();
        let a = list.add_group(); // 1
        list.add_group(); // 2
        list.add_group(); // 3
        assert_eq!(a, 1);

        // Renumber group 3 to 7 (free).
        let changes = list.renumber(3, 7);
        assert_eq!(changes, vec![(3, 7)]);
        assert!(list.get_group(3).is_none());
        assert!(list.get_group(7).is_some());
        // next_id must be above all ids so future adds don't collide.
        assert!(list.next_id > 7);
    }

    #[test]
    fn renumber_swaps_when_target_taken() {
        let mut list = GroupList::default();
        list.add_group(); // 1
        list.add_group(); // 2
        list.add_group(); // 3

        // Renumber group 3 to 2 (taken) → they swap.
        let mut changes = list.renumber(3, 2);
        changes.sort_unstable();
        assert_eq!(changes, vec![(2, 3), (3, 2)]);
        assert!(list.get_group(3).is_some());
        assert!(list.get_group(2).is_some());
    }

    #[test]
    fn renumber_unknown_and_same_id_are_noops() {
        let mut list = GroupList::default();
        list.add_group(); // 1
        assert!(list.renumber(42, 1).is_empty());
        assert!(list.renumber(1, 1).is_empty());
    }
}
