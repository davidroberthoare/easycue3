//! Persisted lighting submasters.

use crate::dmx::Universe;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A sparse snapshot of live channel levels controlled by one submaster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submaster {
    pub name: String,
    /// Current submaster level, in the internal 0-100 range.
    #[serde(default)]
    pub level: u8,
    /// Captured channel levels, keyed like lighting cue channel values.
    #[serde(default, alias = "levels")]
    pub channel_values: HashMap<u16, u8>,
}

impl Submaster {
    pub fn new(number: usize) -> Self {
        Self {
            name: format!("Sub {}", number),
            level: 0,
            channel_values: HashMap::new(),
        }
    }

    /// Capture only non-zero live channels. Effects and submasters are applied to
    /// clones at output time, so the supplied universes are already the clean
    /// cue-stage state.
    pub fn capture(universes: &[Universe]) -> HashMap<u16, u8> {
        let mut values = HashMap::new();
        for (universe_idx, universe) in universes.iter().enumerate() {
            let universe_num = (universe_idx + 1) as u16;
            for channel in 1..=512u16 {
                if let Ok(value) = universe.get_channel(channel) {
                    if value > 0 {
                        values.insert(crate::cue::universe_key(universe_num, channel), value);
                    }
                }
            }
        }
        values
    }

    /// Apply this submaster's contribution using highest-takes-precedence.
    pub fn apply_to(&self, universes: &mut [Universe]) {
        for (&key, &captured) in &self.channel_values {
            let (universe_num, channel) = crate::cue::decode_universe_key(key);
            let Some(universe) = universes.get_mut((universe_num - 1) as usize) else {
                continue;
            };
            let value = (captured.min(100) as u16 * self.level.min(100) as u16 / 100) as u8;
            if universe.get_channel(channel).unwrap_or(0) < value {
                let _ = universe.set_channel(channel, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_omits_zero_channels() {
        let mut universe = Universe::new(1);
        universe.set_channel(1, 50).unwrap();
        universe.set_channel(2, 0).unwrap();
        let values = Submaster::capture(&[universe]);
        assert_eq!(values.get(&1), Some(&50));
        assert!(!values.contains_key(&2));
    }

    #[test]
    fn submasters_are_highest_takes_precedence_and_scaled() {
        let mut universe = Universe::new(1);
        universe.set_channel(1, 60).unwrap();
        let mut submaster = Submaster::new(1);
        submaster.level = 50;
        submaster.channel_values.insert(1, 100);
        submaster.apply_to(std::slice::from_mut(&mut universe));
        assert_eq!(universe.get_channel(1).unwrap(), 60);

        universe.set_channel(1, 40).unwrap();
        submaster.apply_to(std::slice::from_mut(&mut universe));
        assert_eq!(universe.get_channel(1).unwrap(), 50);
    }
}
