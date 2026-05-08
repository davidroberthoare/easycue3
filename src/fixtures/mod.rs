//! Fixture definitions and patching
//!
//! Manages fixture profiles and DMX addressing.

pub mod patching;
pub mod profiles;
pub mod intensity;

pub use patching::{Patch, PatchList};
pub use profiles::FixtureProfile;
pub use intensity::VirtualIntensity;

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

/// Fixture library managing profiles and patch
pub struct FixtureLibrary {
    /// All loaded fixture profiles (keyed by profile ID)
    profiles: HashMap<String, FixtureProfile>,
    /// IDs of profiles loaded from the user config directory
    user_profile_ids: std::collections::HashSet<String>,
    /// Patched fixtures
    patch_list: PatchList,
}

impl FixtureLibrary {
    /// Create a new fixture library and load profiles
    pub fn new() -> Self {
        let mut library = Self {
            profiles: HashMap::new(),
            user_profile_ids: std::collections::HashSet::new(),
            patch_list: PatchList::new(),
        };

        // Load bundled default profiles
        if let Err(e) = library.load_bundled_profiles() {
            log::warn!("Failed to load bundled profiles: {}", e);
        }

        // Load user profiles (override bundled if same ID)
        if let Err(e) = library.load_user_profiles() {
            log::warn!("Failed to load user profiles: {}", e);
        }

        log::info!(
            "Fixture library initialized with {} profiles",
            library.profiles.len()
        );

        library
    }

    /// Load bundled fixture profiles from the app directory
    fn load_bundled_profiles(&mut self) -> Result<()> {
        let Some(bundled_dir) = crate::paths::find_resource_dir("fixture_profiles") else {
            log::warn!("Bundled fixture profiles directory not found: \"fixture_profiles\"");
            return Ok(());
        };

        self.load_profiles_from_dir(&bundled_dir, "bundled", false)
    }

    /// Load user fixture profiles from platform-specific config directory
    fn load_user_profiles(&mut self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not determine config directory"))?;

        let user_dir = config_dir.join("easycue3").join("fixture_profiles");

        if !user_dir.exists() {
            log::debug!("User fixture profiles directory not found: {:?}", user_dir);
            if let Err(e) = std::fs::create_dir_all(&user_dir) {
                log::warn!("Failed to create user profiles directory: {}", e);
            } else {
                log::info!("Created user profiles directory: {:?}", user_dir);
            }
            return Ok(());
        }

        self.load_profiles_from_dir(&user_dir, "user", true)
    }

    /// Load all JSON profiles from a directory
    fn load_profiles_from_dir(&mut self, dir: &Path, source: &str, mark_as_user: bool) -> Result<()> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| anyhow!("Failed to read directory {:?}: {}", dir, e))?;

        let mut loaded_count = 0;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            match FixtureProfile::load_from_file(&path) {
                Ok(profile) => {
                    let id = profile.id.clone();
                    if self.profiles.contains_key(&id) {
                        log::debug!(
                            "Overriding profile '{}' with {} version from {:?}",
                            id,
                            source,
                            path.file_name().unwrap_or_default()
                        );
                    } else {
                        log::info!(
                            "Loaded {} profile '{}' from {:?}",
                            source,
                            id,
                            path.file_name().unwrap_or_default()
                        );
                    }
                    if mark_as_user {
                        self.user_profile_ids.insert(id.clone());
                    }
                    self.profiles.insert(id, profile);
                    loaded_count += 1;
                }
                Err(e) => {
                    log::error!("Failed to load profile from {:?}: {}", path, e);
                }
            }
        }

        if loaded_count > 0 {
            log::info!("Loaded {} {} profiles from {:?}", loaded_count, source, dir);
        }

        Ok(())
    }

    /// Returns the path to the user fixture profiles directory, creating it if needed.
    pub fn user_profiles_dir() -> Result<std::path::PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not determine config directory"))?;
        let dir = config_dir.join("easycue3").join("fixture_profiles");
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow!("Failed to create user profiles directory: {}", e))?;
        Ok(dir)
    }

    /// IDs of profiles that came from the user config directory.
    pub fn user_profile_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.user_profile_ids.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Whether a given profile ID is user-created (not bundled).
    #[allow(dead_code)]
    pub fn is_user_profile(&self, id: &str) -> bool {
        self.user_profile_ids.contains(id)
    }

    /// Save a profile to the user directory and register it in the library.
    /// If `old_id` differs from `profile.id`, the old file is removed.
    pub fn save_user_profile(&mut self, profile: FixtureProfile, old_id: Option<&str>) -> Result<()> {
        let dir = Self::user_profiles_dir()?;

        // Remove old file when the ID was renamed
        if let Some(oid) = old_id {
            if oid != profile.id {
                let old_path = dir.join(format!("{}.json", oid));
                if old_path.exists() {
                    std::fs::remove_file(&old_path)
                        .map_err(|e| anyhow!("Failed to remove old profile file: {}", e))?;
                }
                self.profiles.remove(oid);
                self.user_profile_ids.remove(oid);
            }
        }

        let path = dir.join(format!("{}.json", profile.id));
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|e| anyhow!("Failed to serialize profile: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| anyhow!("Failed to write profile to {:?}: {}", path, e))?;

        self.user_profile_ids.insert(profile.id.clone());
        self.profiles.insert(profile.id.clone(), profile);
        log::info!("Saved user profile to {:?}", path);
        Ok(())
    }

    /// Delete a user profile from disk and unregister it.
    pub fn delete_user_profile(&mut self, id: &str) -> Result<()> {
        if !self.user_profile_ids.contains(id) {
            return Err(anyhow!("Profile '{}' is not a user profile", id));
        }
        let dir = Self::user_profiles_dir()?;
        let path = dir.join(format!("{}.json", id));
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| anyhow!("Failed to delete profile file: {}", e))?;
        }
        self.user_profile_ids.remove(id);
        self.profiles.remove(id);
        log::info!("Deleted user profile '{}'", id);
        Ok(())
    }

    /// Get a fixture profile by ID
    pub fn get_profile(&self, id: &str) -> Option<&FixtureProfile> {
        self.profiles.get(id)
    }

    /// Get all loaded profiles
    #[allow(dead_code)]
    pub fn profiles(&self) -> &HashMap<String, FixtureProfile> {
        &self.profiles
    }

    /// Get profile IDs sorted alphabetically
    pub fn profile_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.profiles.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Get the patch list
    pub fn patch_list(&self) -> &PatchList {
        &self.patch_list
    }

    /// Get mutable patch list
    pub fn patch_list_mut(&mut self) -> &mut PatchList {
        &mut self.patch_list
    }

    /// Add a new patched fixture
    pub fn add_patch(
        &mut self,
        label: String,
        profile_id: String,
        start_address: u16,
    ) -> Result<usize> {
        // Verify profile exists
        let profile = self
            .get_profile(&profile_id)
            .ok_or_else(|| anyhow!("Profile '{}' not found", profile_id))?;

        let channel_counts = self.get_channel_counts();
        self.patch_list
            .add_patch(
                label,
                profile_id,
                start_address,
                profile.channel_count,
                &channel_counts,
            )
    }

    /// Add a fixture with a caller-supplied fixture number (ID). Rejects duplicate IDs.
    pub fn add_patch_with_id(
        &mut self,
        fixture_id: usize,
        label: String,
        profile_id: String,
        start_address: u16,
    ) -> Result<usize> {
        let profile = self
            .get_profile(&profile_id)
            .ok_or_else(|| anyhow!("Profile '{}' not found", profile_id))?;
        let channel_count = profile.channel_count;
        let channel_counts = self.get_channel_counts();
        self.patch_list.add_patch_with_id(
            fixture_id, label, profile_id, start_address, channel_count, &channel_counts,
        )
    }

    /// Lowest positive integer not yet used as a fixture ID.
    pub fn next_available_fixture_id(&self) -> usize {
        self.patch_list.next_available_id()
    }

    /// Rename a fixture's ID (overlap-checked).
    pub fn rename_fixture_id(&mut self, old_id: usize, new_id: usize) -> anyhow::Result<()> {
        self.patch_list.rename_id(old_id, new_id)
    }

    /// Update a patched fixture address
    pub fn update_patch_address(
        &mut self,
        id: usize,
        new_start_address: u16,
        channel_count: u16,
    ) -> Result<()> {
        let channel_counts = self.get_channel_counts();
        self.patch_list
            .update_patch_address(id, new_start_address, channel_count, &channel_counts)
    }

    /// Remove a patched fixture
    pub fn remove_patch(&mut self, id: usize) -> Result<()> {
        self.patch_list.remove_patch(id)
    }

    /// Get channel counts for all profiles (used by patch validation)
    pub fn get_channel_counts(&self) -> HashMap<String, u16> {
        self.profiles
            .iter()
            .map(|(id, profile)| (id.clone(), profile.channel_count))
            .collect()
    }

    /// Find which fixture (if any) is patched to a specific DMX channel
    #[allow(dead_code)]
    pub fn find_patch_at_channel(&self, channel: u16) -> Option<&Patch> {
        let channel_counts = self.get_channel_counts();
        self.patch_list.find_patch_at_channel(channel, &channel_counts)
    }
}

impl Default for FixtureLibrary {
    fn default() -> Self {
        Self::new()
    }
}

