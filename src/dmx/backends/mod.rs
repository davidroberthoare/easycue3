//! DMX output backends
//!
//! Supports multiple output methods: Virtual (logging), USB, and Art-Net.
//! All backends receive 0-100 values and convert to DMX 0-255.

use anyhow::Result;
use crate::dmx::Universe;

pub mod virtual_dmx;
pub mod enttec_usb_pro;

pub use virtual_dmx::VirtualBackend;
pub use enttec_usb_pro::EnttecUsbProBackend;

/// Convert 0-100 intensity to 0-255 DMX value
#[inline]
pub fn intensity_to_dmx(intensity: u8) -> u8 {
    ((intensity as f32 * 2.55).round() as u8).min(255)
}

/// Convert entire universe to DMX format (0-255)
pub fn universe_to_dmx(universe: &Universe) -> [u8; 512] {
    let mut dmx = [0u8; 512];
    let channels = universe.channels();
    for i in 0..512 {
        dmx[i] = intensity_to_dmx(channels[i]);
    }
    dmx
}

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
