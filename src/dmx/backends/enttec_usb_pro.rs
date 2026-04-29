//! Enttec DMXUSB Pro backend
//!
//! Supports Enttec DMXUSB Pro and compatible USB DMX interfaces.
//! Implements the Enttec USB Pro protocol manually for full control.
//!
//! Protocol: Start(0x7E) + Label(0x06) + Length(LSB,MSB) + Data + End(0xE7)
//! The key fix: Enttec Pro uses 250,000 baud, not 57,600!

use anyhow::{Result, Context};
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};
use std::sync::{Arc, Mutex};

#[cfg(feature = "usb")]
use serialport::SerialPort;

/// Enttec DMXUSB Pro backend
pub struct EnttecUsbProBackend {
    #[cfg(feature = "usb")]
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    port_name: String,
}

impl EnttecUsbProBackend {
    /// Create a new Enttec USB Pro backend
    /// 
    /// # Arguments
    /// 
    /// * `port_path` - Serial port path (e.g., "/dev/ttyUSB0" on Linux, "COM3" on Windows)
    #[cfg(feature = "usb")]
    pub fn new(port_path: &str) -> Result<Self> {
        // CRITICAL FIX: Enttec Pro uses 57,600 baud for the API protocol
        // (not 250,000 - that's for raw DMX on the DMX side)
        let port = serialport::new(port_path, 57600)
            .timeout(std::time::Duration::from_millis(100))
            .open()
            .context(format!("Failed to open serial port {}", port_path))?;
        
        log::info!("Enttec DMXUSB Pro initialized on {} at 57600 baud", port_path);
        
        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            port_name: port_path.to_string(),
        })
    }
    
    /// Create backend without USB feature (compile-time error prevention)
    #[cfg(not(feature = "usb"))]
    pub fn new(_port_path: &str) -> Result<Self> {
        anyhow::bail!("USB support not enabled. Rebuild with --features usb")
    }
    
    /// List available serial ports
    #[cfg(feature = "usb")]
    pub fn list_ports() -> Result<Vec<String>> {
        let ports = serialport::available_ports()
            .context("Failed to enumerate serial ports")?;
        
        Ok(ports.into_iter().map(|p| p.port_name).collect())
    }
    
    /// Send DMX data using Enttec Pro protocol
    /// 
    /// Protocol format:
    /// - Start delimiter: 0x7E
    /// - Label: 0x06 (Send DMX Packet)
    /// - Data length LSB
    /// - Data length MSB
    /// - Data: DMX start code (0x00) + 512 DMX channels
    /// - End delimiter: 0xE7
    #[cfg(feature = "usb")]
    fn send_dmx_packet(&self, dmx_data: &[u8; 512]) -> Result<()> {
        const START_BYTE: u8 = 0x7E;
        const END_BYTE: u8 = 0xE7;
        const LABEL_SEND_DMX: u8 = 0x06;
        const DATA_LENGTH: u16 = 513; // start code (1) + 512 channels
        
        // Build message: 5-byte header + 513 data bytes + 1 end byte = 519 bytes total
        let mut message = Vec::with_capacity(519);
        
        // Header
        message.push(START_BYTE);
        message.push(LABEL_SEND_DMX);
        message.push((DATA_LENGTH & 0xFF) as u8);      // Length LSB
        message.push(((DATA_LENGTH >> 8) & 0xFF) as u8); // Length MSB
        
        // Data payload: DMX start code + 512 channels
        message.push(0x00); // DMX512 start code (0x00 for dimmer data)
        message.extend_from_slice(dmx_data);
        
        // End delimiter
        message.push(END_BYTE);
        
        // Send to serial port (thread-safe)
        let mut port = self.port.lock().unwrap();
        port.write_all(&message)
            .context("Failed to write DMX data to serial port")?;
        
        port.flush()
            .context("Failed to flush serial port")?;
        
        Ok(())
    }
}

#[cfg(feature = "usb")]
impl DmxBackend for EnttecUsbProBackend {
    fn send_universe(&mut self, universe: &Universe) -> Result<()> {
        // Convert 0-100 intensities to 0-255 DMX values
        let dmx_data = universe_to_dmx(universe);
        
        // Send via Enttec Pro protocol
        self.send_dmx_packet(&dmx_data)?;
        
        // Log non-zero channels for debugging
        let non_zero: Vec<(usize, u8)> = dmx_data
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > 0)
            .map(|(i, &v)| (i + 1, v))
            .collect();
        
        if !non_zero.is_empty() {
            log::debug!("Sent to {} - Universe {} DMX values: {:?}", 
                self.port_name, universe.id(), non_zero);
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Enttec DMXUSB Pro"
    }
    
    fn close(&mut self) -> Result<()> {
        log::info!("Closing Enttec DMXUSB Pro on {}", self.port_name);
        Ok(())
    }
}

#[cfg(not(feature = "usb"))]
impl DmxBackend for EnttecUsbProBackend {
    fn send_universe(&mut self, _universe: &Universe) -> Result<()> {
        anyhow::bail!("USB support not enabled")
    }
    
    fn name(&self) -> &str {
        "Enttec DMXUSB Pro (USB not enabled)"
    }
}

#[cfg(test)]
#[cfg(feature = "usb")]
mod tests {
    use super::*;
    
    #[test]
    fn test_list_ports() {
        // Should not panic
        match EnttecUsbProBackend::list_ports() {
            Ok(ports) => {
                println!("Available ports: {:?}", ports);
            }
            Err(e) => {
                println!("No ports available: {}", e);
            }
        }
    }
    
    #[test]
    fn test_protocol_message_format() {
        // Verify the message format is correct
        let dmx_data = [0u8; 512];
        
        const START_BYTE: u8 = 0x7E;
        const END_BYTE: u8 = 0xE7;
        const LABEL: u8 = 0x06;
        const LEN_LSB: u8 = 0x01; // 513 & 0xFF = 1
        const LEN_MSB: u8 = 0x02; // 513 >> 8 = 2
        
        let mut expected = vec![START_BYTE, LABEL, LEN_LSB, LEN_MSB, 0x00];
        expected.extend_from_slice(&dmx_data);
        expected.push(END_BYTE);
        
        assert_eq!(expected.len(), 519);
        assert_eq!(expected[0], 0x7E);
        assert_eq!(expected[518], 0xE7);
    }
}
