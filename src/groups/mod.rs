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
        Self { id, label: String::new(), fixture_ids: Vec::new() }
    }

    /// Format a fixture ID list as a comma-separated string: "1, 2, 3".
    pub fn fixtures_to_string(fixture_ids: &[usize]) -> String {
        fixture_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", ")
    }

    /// Parse a comma-separated string like "1, 2, 3" into fixture IDs.
    /// Invalid tokens and zeroes are silently skipped.
    pub fn parse_fixtures_string(s: &str) -> Vec<usize> {
        let mut ids: Vec<usize> = s
            .split(',')
            .filter_map(|part| part.trim().parse::<usize>().ok())
            .filter(|&id| id >= 1)
            .collect();
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

fn default_next_id() -> u32 { 1 }

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
