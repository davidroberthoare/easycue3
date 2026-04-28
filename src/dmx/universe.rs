//! DMX Universe representation
//!
//! A DMX universe consists of 512 channels (1-512), each with a value 0-255.

use anyhow::Result;

/// A single DMX universe with 512 channels
#[derive(Debug, Clone)]
pub struct Universe {
    /// DMX channel values (0-255), indexed 0-511 for channels 1-512
    channels: [u8; 512],
    /// Universe number (typically 0 or 1 for our use case)
    universe_id: u16,
}

impl Universe {
    /// Create a new universe with all channels at 0
    pub fn new(universe_id: u16) -> Self {
        Self {
            channels: [0; 512],
            universe_id,
        }
    }

    /// Get the universe ID
    pub fn id(&self) -> u16 {
        self.universe_id
    }

    /// Set a channel value (1-indexed: channel 1-512)
    pub fn set_channel(&mut self, channel: u16, value: u8) -> Result<()> {
        if channel < 1 || channel > 512 {
            anyhow::bail!("Channel {} out of range (1-512)", channel);
        }
        self.channels[(channel - 1) as usize] = value;
        Ok(())
    }

    /// Get a channel value (1-indexed: channel 1-512)
    pub fn get_channel(&self, channel: u16) -> Result<u8> {
        if channel < 1 || channel > 512 {
            anyhow::bail!("Channel {} out of range (1-512)", channel);
        }
        Ok(self.channels[(channel - 1) as usize])
    }

    /// Get all channel values as a slice
    pub fn channels(&self) -> &[u8; 512] {
        &self.channels
    }

    /// Set multiple channels at once
    pub fn set_channels(&mut self, start_channel: u16, values: &[u8]) -> Result<()> {
        if start_channel < 1 || start_channel > 512 {
            anyhow::bail!("Start channel {} out of range (1-512)", start_channel);
        }
        
        let start_idx = (start_channel - 1) as usize;
        let end_idx = start_idx + values.len();
        
        if end_idx > 512 {
            anyhow::bail!("Channel range exceeds universe size (512 channels)");
        }
        
        self.channels[start_idx..end_idx].copy_from_slice(values);
        Ok(())
    }

    /// Clear all channels to 0
    pub fn clear(&mut self) {
        self.channels.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universe_creation() {
        let universe = Universe::new(0);
        assert_eq!(universe.id(), 0);
        assert_eq!(universe.get_channel(1).unwrap(), 0);
        assert_eq!(universe.get_channel(512).unwrap(), 0);
    }

    #[test]
    fn test_set_get_channel() {
        let mut universe = Universe::new(0);
        universe.set_channel(1, 255).unwrap();
        assert_eq!(universe.get_channel(1).unwrap(), 255);
        
        universe.set_channel(512, 128).unwrap();
        assert_eq!(universe.get_channel(512).unwrap(), 128);
    }

    #[test]
    fn test_channel_bounds() {
        let mut universe = Universe::new(0);
        assert!(universe.set_channel(0, 255).is_err());
        assert!(universe.set_channel(513, 255).is_err());
        assert!(universe.get_channel(0).is_err());
        assert!(universe.get_channel(513).is_err());
    }

    #[test]
    fn test_set_multiple_channels() {
        let mut universe = Universe::new(0);
        let values = [100, 150, 200, 250];
        universe.set_channels(1, &values).unwrap();
        
        assert_eq!(universe.get_channel(1).unwrap(), 100);
        assert_eq!(universe.get_channel(2).unwrap(), 150);
        assert_eq!(universe.get_channel(3).unwrap(), 200);
        assert_eq!(universe.get_channel(4).unwrap(), 250);
    }
}
