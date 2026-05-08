//! Virtual DMX backend for testing without hardware
//!
//! Logs DMX output to console for debugging and development.
//! Converts 0-100 intensities to 0-255 DMX values for output.

use anyhow::Result;
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// Virtual DMX backend that logs output
pub struct VirtualBackend {
    /// Whether to log all channels or only non-zero
    verbose: bool,
    /// Last emitted payload hash to avoid duplicate log spam.
    last_payload_hash: Option<u64>,
    /// Last time we emitted a debug line.
    last_log_time: Instant,
}

impl VirtualBackend {
    const LOG_INTERVAL: Duration = Duration::from_millis(250);

    /// Create a new virtual backend
    pub fn new(verbose: bool) -> Self {
        log::info!("Virtual DMX backend initialized (verbose: {})", verbose);
        Self {
            verbose,
            last_payload_hash: None,
            last_log_time: Instant::now() - Self::LOG_INTERVAL,
        }
    }
}

impl DmxBackend for VirtualBackend {
    fn send_universe(&mut self, universe: &Universe) -> Result<()> {
        if self.verbose {
            // Convert to DMX values for logging
            let dmx_data = universe_to_dmx(universe);
            let now = Instant::now();

            let mut hasher = DefaultHasher::new();
            universe.channels().hash(&mut hasher);
            let payload_hash = hasher.finish();

            let changed = self.last_payload_hash != Some(payload_hash);
            let interval_elapsed = now.duration_since(self.last_log_time) >= Self::LOG_INTERVAL;

            if !changed && !interval_elapsed {
                return Ok(());
            }
            
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

            self.last_payload_hash = Some(payload_hash);
            self.last_log_time = now;
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
