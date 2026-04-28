//! DMX output backends
//!
//! Supports multiple output methods: Virtual (logging), USB, and Art-Net.

use anyhow::Result;
use crate::dmx::Universe;

pub mod virtual_dmx;

pub use virtual_dmx::VirtualBackend;

/// Trait for DMX output backends
pub trait DmxBackend: Send + Sync {
    /// Send a universe to the output
    fn send_universe(&mut self, universe: &Universe) -> Result<()>;
    
    /// Get backend name/description
    fn name(&self) -> &str;
    
    /// Close/cleanup the backend
    fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
