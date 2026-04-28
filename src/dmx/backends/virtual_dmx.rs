//! Virtual DMX backend for testing without hardware
//!
//! Logs DMX output to console for debugging and development.

use anyhow::Result;
use crate::dmx::{Universe, backends::DmxBackend};

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
            // Log all non-zero channels
            let non_zero: Vec<(usize, u8)> = universe.channels()
                .iter()
                .enumerate()
                .filter(|(_, &v)| v > 0)
                .map(|(i, &v)| (i + 1, v))
                .collect();
            
            if !non_zero.is_empty() {
                log::debug!("Universe {}: {:?}", universe.id(), non_zero);
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
