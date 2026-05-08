//! Virtual intensity system for RGB fixtures without dedicated intensity channels
//!
//! This module provides color-preserving intensity control for RGB/RGBAW/etc fixtures
//! by storing normalized color ratios and scaling them proportionally.
//!
//! **Note:** Universe uses 0-100 DMX range, not 0-255.

use anyhow::Result;
use std::collections::HashMap;

use crate::dmx::Universe;
use crate::fixtures::{Patch, FixtureProfile};
use crate::fixtures::profiles::FixtureParameter;

/// Virtual intensity manager for fixtures without dedicated intensity channels
#[derive(Debug, Clone)]
pub struct VirtualIntensity {
    /// Per-fixture color state storage (keyed by fixture ID)
    fixture_states: HashMap<usize, FixtureColorState>,
}

/// Color state for a single fixture (normalized ratios + current intensity)
#[derive(Debug, Clone)]
pub struct FixtureColorState {
    /// Normalized color ratios (0.0-1.0) for each color channel
    /// e.g., purple RGB = {Red: 1.0, Green: 0.0, Blue: 1.0}
    color_ratios: HashMap<FixtureParameter, f32>,
    
    /// Current virtual intensity (0.0-1.0)
    intensity: f32,
}

impl VirtualIntensity {
    /// Create a new virtual intensity manager
    pub fn new() -> Self {
        Self {
            fixture_states: HashMap::new(),
        }
    }
    
    /// Calculate current virtual intensity from DMX values
    /// Virtual intensity = max(all color channels) / 100.0
    pub fn calculate_intensity(
        &self,
        _fixture_id: usize,
        universe: &Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) -> f32 {
        let mut max_value = 0u8;
        
        // Find the maximum value among all color channels
        for param_mapping in profile.color_parameters() {
            let channel = patch.start_address + param_mapping.channel_offset;
            if let Ok(value) = universe.get_channel(channel) {
                max_value = max_value.max(value);
            }
        }
        
        max_value as f32 / 100.0
    }
    
    /// Update color ratios when color changes (e.g., via color picker)
    /// Stores normalized ratios and maintains current intensity
    pub fn set_color(
        &mut self,
        fixture_id: usize,
        color_values: HashMap<FixtureParameter, u8>,
    ) {
        // Find the maximum color value to calculate ratios
        let max_value = *color_values.values().max().unwrap_or(&0);
        
        if max_value == 0 {
            // All colors are zero - store equal ratios but zero intensity
            let state = self.fixture_states.entry(fixture_id).or_insert_with(|| {
                FixtureColorState {
                    color_ratios: HashMap::new(),
                    intensity: 0.0,
                }
            });
            
            // Equal ratios (white) when all zero
            for (param, _) in color_values.iter() {
                state.color_ratios.insert(param.clone(), 1.0);
            }
            state.intensity = 0.0;
        } else {
            // Calculate normalized ratios
            let mut ratios = HashMap::new();
            for (param, value) in color_values.iter() {
                ratios.insert(param.clone(), *value as f32 / max_value as f32);
            }
            
            let intensity = max_value as f32 / 100.0;
            
            self.fixture_states.insert(
                fixture_id,
                FixtureColorState {
                    color_ratios: ratios,
                    intensity,
                },
            );
        }
    }
    
    /// Set virtual intensity while preserving color ratios
    /// Returns error if no color state exists for this fixture
    pub fn set_intensity(
        &mut self,
        fixture_id: usize,
        intensity: f32,
        universe: &mut Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) -> Result<()> {
        let intensity = intensity.clamp(0.0, 1.0);
        
        // Initialize state if it doesn't exist
        if !self.fixture_states.contains_key(&fixture_id) {
            let state = Self::initialize_from_universe_static(universe, patch, profile);
            self.fixture_states.insert(fixture_id, state);
        }
        
        // Get mutable reference and update intensity
        if let Some(state) = self.fixture_states.get_mut(&fixture_id) {
            state.intensity = intensity;
            
            // Apply the new intensity to universe
            Self::apply_state_to_universe(state, universe, patch, profile)
        } else {
            Err(anyhow::anyhow!("Failed to get fixture state"))
        }
    }
    
    /// Get current virtual intensity for a fixture
    pub fn get_intensity(&self, fixture_id: usize) -> Option<f32> {
        self.fixture_states.get(&fixture_id).map(|s| s.intensity)
    }
    
    /// Initialize fixture state from current DMX values
    fn initialize_from_universe_static(
        universe: &Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) -> FixtureColorState {
        let mut color_values = HashMap::new();
        let mut max_value = 0u8;
        
        // Read current DMX values
        for param_mapping in profile.color_parameters() {
            let channel = patch.start_address + param_mapping.channel_offset;
            if let Ok(value) = universe.get_channel(channel) {
                color_values.insert(param_mapping.parameter.clone(), value);
                max_value = max_value.max(value);
            }
        }
        
        // Calculate ratios
        let mut ratios = HashMap::new();
        if max_value > 0 {
            for (param, value) in color_values.iter() {
                ratios.insert(param.clone(), *value as f32 / max_value as f32);
            }
        } else {
            // Default to equal ratios (white)
            for param_mapping in profile.color_parameters() {
                ratios.insert(param_mapping.parameter.clone(), 1.0);
            }
        }
        
        FixtureColorState {
            color_ratios: ratios,
            intensity: max_value as f32 / 100.0,
        }
    }
    
    /// Apply stored ratios + intensity to DMX universe
    #[allow(dead_code)]
    pub fn apply_to_universe(
        &self,
        fixture_id: usize,
        universe: &mut Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) -> Result<()> {
        let state = self.fixture_states.get(&fixture_id)
            .ok_or_else(|| anyhow::anyhow!("No color state for fixture {}", fixture_id))?;
        
        Self::apply_state_to_universe(state, universe, patch, profile)
    }
    
    /// Internal helper to apply state to universe (static to avoid borrow issues)
    fn apply_state_to_universe(
        state: &FixtureColorState,
        universe: &mut Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) -> Result<()> {
        // Apply color ratios scaled by intensity
        for param_mapping in profile.color_parameters() {
            let channel = patch.start_address + param_mapping.channel_offset;
            
            let ratio = state.color_ratios.get(&param_mapping.parameter)
                .copied()
                .unwrap_or(0.0);
            
            let dmx_value = (ratio * state.intensity * 100.0) as u8;
            
            universe.set_channel(channel, dmx_value)?;
        }
        
        Ok(())
    }
    
    /// Remove stored state for a fixture (e.g., when its ID is changed).
    pub fn remove_fixture(&mut self, fixture_id: usize) {
        self.fixture_states.remove(&fixture_id);
    }

    /// Update fixture state after DMX values change (e.g., during cue playback fade)
    /// Recalculates ratios from current DMX to allow subsequent intensity control
    pub fn update_from_universe(
        &mut self,
        fixture_id: usize,
        universe: &Universe,
        patch: &Patch,
        profile: &FixtureProfile,
    ) {
        let state = Self::initialize_from_universe_static(universe, patch, profile);
        self.fixture_states.insert(fixture_id, state);
    }
}

impl Default for VirtualIntensity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_profile() -> FixtureProfile {
        use crate::fixtures::profiles::ParameterMapping;
        
        FixtureProfile {
            id: "test_rgb".to_string(),
            name: "Test RGB".to_string(),
            manufacturer: None,
            channel_count: 3,
            parameters: vec![
                ParameterMapping {
                    parameter: FixtureParameter::Red,
                    channel_offset: 0,
                    default_value: None,
                },
                ParameterMapping {
                    parameter: FixtureParameter::Green,
                    channel_offset: 1,
                    default_value: None,
                },
                ParameterMapping {
                    parameter: FixtureParameter::Blue,
                    channel_offset: 2,
                    default_value: None,
                },
            ],
            notes: None,
        }
    }
    
    fn create_test_patch() -> Patch {
        Patch {
            id: 1,
            label: "Test Fixture".to_string(),
            profile_id: "test_rgb".to_string(),
            start_address: 10,
            universe: 1,
            notes: String::new(),
        }
    }
    
    #[test]
    fn test_purple_at_half_intensity() {
        let mut vi = VirtualIntensity::new();
        let mut universe = Universe::new(0);
        let patch = create_test_patch();
        let profile = create_test_profile();
        
        // Set purple (100, 0, 100) at full intensity
        let mut colors = HashMap::new();
        colors.insert(FixtureParameter::Red, 100);
        colors.insert(FixtureParameter::Green, 0);
        colors.insert(FixtureParameter::Blue, 100);
        
        vi.set_color(1, colors);
        
        // Set intensity to 50%
        vi.set_intensity(1, 0.5, &mut universe, &patch, &profile).unwrap();
        
        // Should be (50, 0, 50)
        assert_eq!(universe.get_channel(10).unwrap(), 50);
        assert_eq!(universe.get_channel(11).unwrap(), 0);
        assert_eq!(universe.get_channel(12).unwrap(), 50);
    }
    
    #[test]
    fn test_intensity_to_zero_and_back() {
        let mut vi = VirtualIntensity::new();
        let mut universe = Universe::new(0);
        let patch = create_test_patch();
        let profile = create_test_profile();
        
        // Set RGB (80, 40, 20)
        let mut colors = HashMap::new();
        colors.insert(FixtureParameter::Red, 80);
        colors.insert(FixtureParameter::Green, 40);
        colors.insert(FixtureParameter::Blue, 20);
        
        vi.set_color(1, colors);
        
        // Get the initial intensity (should be 0.8)
        let initial_intensity = vi.get_intensity(1).unwrap();
        assert_eq!(initial_intensity, 0.8);
        
        // Dim to 0%
        vi.set_intensity(1, 0.0, &mut universe, &patch, &profile).unwrap();
        
        assert_eq!(universe.get_channel(10).unwrap(), 0);
        assert_eq!(universe.get_channel(11).unwrap(), 0);
        assert_eq!(universe.get_channel(12).unwrap(), 0);
        
        // Restore to original intensity (80%)
        vi.set_intensity(1, initial_intensity, &mut universe, &patch, &profile).unwrap();
        
        // Should restore original values
        assert_eq!(universe.get_channel(10).unwrap(), 80);
        assert_eq!(universe.get_channel(11).unwrap(), 40);
        assert_eq!(universe.get_channel(12).unwrap(), 20);
    }
    
    #[test]
    fn test_intensity_stops_at_max() {
        let mut vi = VirtualIntensity::new();
        let mut universe = Universe::new(0);
        let patch = create_test_patch();
        let profile = create_test_profile();
        
        // Set red at max (100, 0, 0)
        let mut colors = HashMap::new();
        colors.insert(FixtureParameter::Red, 100);
        colors.insert(FixtureParameter::Green, 0);
        colors.insert(FixtureParameter::Blue, 0);
        
        vi.set_color(1, colors);
        
        // Try to increase intensity beyond 100%
        vi.set_intensity(1, 1.5, &mut universe, &patch, &profile).unwrap();
        
        // Should clamp to 100
        assert_eq!(universe.get_channel(10).unwrap(), 100);
        assert_eq!(universe.get_channel(11).unwrap(), 0);
        assert_eq!(universe.get_channel(12).unwrap(), 0);
    }
    
    #[test]
    fn test_multi_color_fixture_intensity() {
        // Test RGBAWUV fixture preserves all colors when adjusting intensity
        use crate::fixtures::profiles::ParameterMapping;
        
        let mut universe = Universe::new(1);
        let mut vi = VirtualIntensity::new();
        
        // RGBAWUV fixture at channel 1-6
        let profile = FixtureProfile {
            id: "test_rgbawuv".to_string(),
            name: "Test RGBAWUV".to_string(),
            manufacturer: None,
            channel_count: 6,
            parameters: vec![
                ParameterMapping { 
                    parameter: FixtureParameter::Red, 
                    channel_offset: 0, 
                    default_value: None,
                },
                ParameterMapping { 
                    parameter: FixtureParameter::Green, 
                    channel_offset: 1, 
                    default_value: None,
                },
                ParameterMapping { 
                    parameter: FixtureParameter::Blue, 
                    channel_offset: 2, 
                    default_value: None,
                },
                ParameterMapping { 
                    parameter: FixtureParameter::Amber, 
                    channel_offset: 3, 
                    default_value: None,
                },
                ParameterMapping { 
                    parameter: FixtureParameter::White, 
                    channel_offset: 4, 
                    default_value: None,
                },
                ParameterMapping { 
                    parameter: FixtureParameter::Uv, 
                    channel_offset: 5, 
                    default_value: None,
                },
            ],
            notes: None,
        };
        
        let patch = Patch {
            id: 1,
            start_address: 1,
            profile_id: "test_rgbawuv".to_string(),
            label: "Test".to_string(),
            universe: 1,
            notes: String::new(),
        };
        
        // Set initial color values: r=55, g=20, b=30, a=66, w=44, uv=10
        let mut colors = HashMap::new();
        colors.insert(FixtureParameter::Red, 55);
        colors.insert(FixtureParameter::Green, 20);
        colors.insert(FixtureParameter::Blue, 30);
        colors.insert(FixtureParameter::Amber, 66);
        colors.insert(FixtureParameter::White, 44);
        colors.insert(FixtureParameter::Uv, 10);
        
        vi.set_color(1, colors);
        vi.apply_to_universe(1, &mut universe, &patch, &profile).unwrap();
        
        // Verify initial values
        assert_eq!(universe.get_channel(1).unwrap(), 55);
        assert_eq!(universe.get_channel(2).unwrap(), 20);
        assert_eq!(universe.get_channel(3).unwrap(), 30);
        assert_eq!(universe.get_channel(4).unwrap(), 66);
        assert_eq!(universe.get_channel(5).unwrap(), 44);
        assert_eq!(universe.get_channel(6).unwrap(), 10);
        
        // Reduce intensity to 50%
        vi.set_intensity(1, 0.5, &mut universe, &patch, &profile).unwrap();
        
        // All colors should scale proportionally (not just red!)
        // Ratios: r=55/66=0.833, g=20/66=0.303, b=30/66=0.455, a=66/66=1.0, w=44/66=0.667, uv=10/66=0.152
        // At 50%: r=41, g=15, b=22, a=50, w=33, uv=7
        assert_eq!(universe.get_channel(1).unwrap(), 41);  // Red
        assert_eq!(universe.get_channel(2).unwrap(), 15);  // Green
        assert_eq!(universe.get_channel(3).unwrap(), 22);  // Blue
        assert_eq!(universe.get_channel(4).unwrap(), 50);  // Amber
        assert_eq!(universe.get_channel(5).unwrap(), 33);  // White
        assert_eq!(universe.get_channel(6).unwrap(), 7);   // UV
        
        // Increase intensity to 75%
        vi.set_intensity(1, 0.75, &mut universe, &patch, &profile).unwrap();
        
        // At 75%: r=62, g=22, b=34, a=75, w=50, uv=11
        assert_eq!(universe.get_channel(1).unwrap(), 62);  // Red
        assert_eq!(universe.get_channel(2).unwrap(), 22);  // Green  
        assert_eq!(universe.get_channel(3).unwrap(), 34);  // Blue
        assert_eq!(universe.get_channel(4).unwrap(), 75);  // Amber
        assert_eq!(universe.get_channel(5).unwrap(), 50);  // White
        assert_eq!(universe.get_channel(6).unwrap(), 11);  // UV
    }
}
