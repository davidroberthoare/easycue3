//! Virtual DMX backend for testing without hardware
//!
//! Logs DMX output to console for debugging and development.
//! Converts 0-100 intensities to 0-255 DMX values for output.

use anyhow::Result;
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};

/// Virtual DMX backend that logs output
pub struct VirtualBackend {
    /// Whether to log all channels or only non-zero
    verbose: bool,
}

impl VirtualBackend {
    /// Create a new virtual backend
    pub fn new(verbose: bool) -> Self {
        log::info!("Virtual DMX backend initialized (verbose: {})", verbose);
        Self { verbose }
    }
}

impl DmxBackend for VirtualBackend {
    fn send_universe(&mut self, universe: &Universe) -> Result<()> {
        if self.verbose {
            // Convert to DMX values for logging
            let dmx_data = universe_to_dmx(universe);
            
            // Log all non-zero channels (showing both intensity and DMX value)
            let non_zero: Vec<(usize, u8, u8)> = universe.channels()
                .iter()
                .enumerate()
                .filter(|(_, &v)| v > 0)
                .map(|(i, &intensity)| (i + 1, intensity, dmx_data[i]))
                .collect();
            
            if !non_zero.is_empty() {
                log::debug!("Universe {} (intensity@DMX): {:?}", universe.id(), 
                    non_zero.iter().map(|(ch, int, dmx)| format!("{}:{}@{}", ch, int, dmx)).collect::<Vec<_>>());
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Virtual DMX (logging)"
    }
}

impl Default for VirtualBackend {
    fn default() -> Self {
        Self::new(false)
    }
}
