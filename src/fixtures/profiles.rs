//! Fixture profile definitions
//!
//! Defines fixture types, parameters, and channel mappings for DMX control.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Parameter types supported by fixture profiles
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureParameter {
    /// Master intensity/dimmer channel
    Intensity,
    /// Red channel
    Red,
    /// Green channel
    Green,
    /// Blue channel
    Blue,
    /// Amber channel
    Amber,
    /// White channel
    White,
    /// UV channel
    Uv,
    /// Strobe/shutter control
    Strobe,
    /// Iris control (beam size)
    Iris,
    /// Gobo wheel selection
    Gobo,
    /// Pan (horizontal movement)
    Pan,
    /// Pan fine (16-bit pan LSB)
    PanFine,
    /// Tilt (vertical movement)
    Tilt,
    /// Tilt fine (16-bit tilt LSB)
    TiltFine,
    /// Focus control
    Focus,
    /// Zoom control
    Zoom,
    /// Prism control
    Prism,
    /// Frost/diffusion control
    Frost,
    /// Custom parameter (user-defined)
    Custom(String),
}

impl FixtureParameter {
    /// Returns true if this is a color channel (RGB/AWUV)
    pub fn is_color(&self) -> bool {
        matches!(
            self,
            FixtureParameter::Red
                | FixtureParameter::Green
                | FixtureParameter::Blue
                | FixtureParameter::Amber
                | FixtureParameter::White
                | FixtureParameter::Uv
        )
    }

    /// Returns true if this is a beam control parameter
    #[allow(dead_code)]
    pub fn is_beam(&self) -> bool {
        matches!(
            self,
            FixtureParameter::Iris
                | FixtureParameter::Focus
                | FixtureParameter::Zoom
                | FixtureParameter::Frost
        )
    }

    /// Returns true if this is a position parameter
    #[allow(dead_code)]
    pub fn is_position(&self) -> bool {
        matches!(
            self,
            FixtureParameter::Pan
                | FixtureParameter::PanFine
                | FixtureParameter::Tilt
                | FixtureParameter::TiltFine
        )
    }

    /// Short display label for use in UI sliders (≤5 chars).
    pub fn short_label(&self) -> &str {
        match self {
            FixtureParameter::Intensity => "Int",
            FixtureParameter::Red       => "R",
            FixtureParameter::Green     => "G",
            FixtureParameter::Blue      => "B",
            FixtureParameter::Amber     => "A",
            FixtureParameter::White     => "W",
            FixtureParameter::Uv        => "UV",
            FixtureParameter::Strobe    => "Strb",
            FixtureParameter::Iris      => "Iris",
            FixtureParameter::Gobo      => "Gobo",
            FixtureParameter::Pan       => "Pan",
            FixtureParameter::PanFine   => "PanF",
            FixtureParameter::Tilt      => "Tilt",
            FixtureParameter::TiltFine  => "TltF",
            FixtureParameter::Focus     => "Foc",
            FixtureParameter::Zoom      => "Zoom",
            FixtureParameter::Prism     => "Prsm",
            FixtureParameter::Frost     => "Frst",
            FixtureParameter::Custom(s) => s.as_str(),
        }
    }
}

/// Maps a fixture parameter to a DMX channel offset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterMapping {
    /// The parameter being controlled
    pub parameter: FixtureParameter,
    /// Channel offset from fixture's base address (0-indexed)
    pub channel_offset: u16,
    /// Optional default value (0-255)
    #[serde(default)]
    pub default_value: Option<u8>,
}

/// Fixture profile definition loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureProfile {
    /// Unique identifier for this profile (e.g., "generic_dimmer")
    pub id: String,
    /// Human-readable name (e.g., "Generic Dimmer")
    pub name: String,
    /// Manufacturer name (optional)
    #[serde(default)]
    pub manufacturer: Option<String>,
    /// Total number of DMX channels used
    pub channel_count: u16,
    /// Parameter to channel mappings
    pub parameters: Vec<ParameterMapping>,
    /// Optional notes or documentation
    #[serde(default)]
    pub notes: Option<String>,
}

impl FixtureProfile {
    /// Load a fixture profile from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(&path).map_err(|e| {
            anyhow!(
                "Failed to read fixture profile '{}': {}",
                path.as_ref().display(),
                e
            )
        })?;

        let profile: FixtureProfile = serde_json::from_str(&contents).map_err(|e| {
            anyhow!(
                "Failed to parse fixture profile '{}': {}",
                path.as_ref().display(),
                e
            )
        })?;

        profile.validate()?;
        Ok(profile)
    }

    /// Validate that the profile is well-formed
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("Fixture profile ID cannot be empty"));
        }

        if self.name.is_empty() {
            return Err(anyhow!("Fixture profile name cannot be empty"));
        }

        if self.channel_count == 0 {
            return Err(anyhow!(
                "Fixture profile '{}' must have at least one channel",
                self.id
            ));
        }

        // Verify all channel offsets are within bounds
        for param_map in &self.parameters {
            if param_map.channel_offset >= self.channel_count {
                return Err(anyhow!(
                    "Parameter {:?} offset {} exceeds channel count {} in profile '{}'",
                    param_map.parameter,
                    param_map.channel_offset,
                    self.channel_count,
                    self.id
                ));
            }
        }

        Ok(())
    }

    /// Get the channel offset for a specific parameter, if it exists
    pub fn get_parameter_offset(&self, param: &FixtureParameter) -> Option<u16> {
        self.parameters
            .iter()
            .find(|p| &p.parameter == param)
            .map(|p| p.channel_offset)
    }

    /// Check if this profile has a specific parameter
    pub fn has_parameter(&self, param: &FixtureParameter) -> bool {
        self.parameters.iter().any(|p| &p.parameter == param)
    }

    /// Get all color parameters defined in this profile
    pub fn color_parameters(&self) -> Vec<&ParameterMapping> {
        self.parameters
            .iter()
            .filter(|p| p.parameter.is_color())
            .collect()
    }

    /// Check if this is an RGB-capable fixture
    pub fn is_rgb(&self) -> bool {
        self.has_parameter(&FixtureParameter::Red)
            && self.has_parameter(&FixtureParameter::Green)
            && self.has_parameter(&FixtureParameter::Blue)
    }

    /// Check if this has a separate intensity channel
    pub fn has_intensity(&self) -> bool {
        self.has_parameter(&FixtureParameter::Intensity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_is_color() {
        assert!(FixtureParameter::Red.is_color());
        assert!(FixtureParameter::Green.is_color());
        assert!(FixtureParameter::Blue.is_color());
        assert!(FixtureParameter::White.is_color());
        assert!(!FixtureParameter::Intensity.is_color());
        assert!(!FixtureParameter::Pan.is_color());
    }

    #[test]
    fn test_profile_validation() {
        let mut profile = FixtureProfile {
            id: "test".to_string(),
            name: "Test Fixture".to_string(),
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
        };

        // Valid profile
        assert!(profile.validate().is_ok());
        assert!(profile.is_rgb());
        assert!(!profile.has_intensity());

        // Invalid: offset out of bounds
        profile.parameters[2].channel_offset = 10;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_get_parameter_offset() {
        let profile = FixtureProfile {
            id: "rgb".to_string(),
            name: "RGB".to_string(),
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
        };

        assert_eq!(profile.get_parameter_offset(&FixtureParameter::Red), Some(0));
        assert_eq!(
            profile.get_parameter_offset(&FixtureParameter::Green),
            Some(1)
        );
        assert_eq!(profile.get_parameter_offset(&FixtureParameter::Blue), Some(2));
        assert_eq!(profile.get_parameter_offset(&FixtureParameter::White), None);
    }
}
