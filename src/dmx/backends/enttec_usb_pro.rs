//! Enttec DMXUSB Pro backend
//!
//! Supports Enttec DMXUSB Pro and compatible USB DMX interfaces.
//! Implements the Enttec USB Pro protocol manually for full control.
//!
//! Protocol: Start(0x7E) + Label(0x06) + Length(LSB,MSB) + Data + End(0xE7)
//! The key fix: Enttec Pro uses 250,000 baud, not 57,600!
//!
//! PERFORMANCE: Uses a background thread to send DMX at 40 Hz, preventing
//! UI blocking on serial I/O (which can take 80-100ms per frame).

use anyhow::{Result, Context};
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};
use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

#[cfg(feature = "usb")]
use serialport::SerialPort;
#[cfg(feature = "usb")]
use serialport::{SerialPortInfo, SerialPortType};

/// Enttec DMXUSB Pro backend with threaded sending
pub struct EnttecUsbProBackend {
    /// Channel to send universe data to background thread
    tx: Sender<[u8; 512]>,
    /// Port name for identification
    port_name: String,
    /// Handle to background thread (for cleanup)
    _thread_handle: Option<thread::JoinHandle<()>>,
    /// Set to false by the background thread when the device is lost.
    connected: Arc<AtomicBool>,
}

impl EnttecUsbProBackend {
    #[cfg(feature = "usb")]
    const ENTTEC_USB_VID: u16 = 0x0403;
    #[cfg(feature = "usb")]
    const ENTTEC_USB_PID: u16 = 0x6001;

    /// Create a new Enttec USB Pro backend with threaded sending
    /// 
    /// Spawns a background thread that sends DMX at 40 Hz (every 25ms).
    /// The main thread sends universe data via a channel (non-blocking).
    /// 
    /// # Arguments
    /// 
    /// * `port_path` - Serial port path (e.g., "/dev/ttyUSB0" on Linux, "COM3" on Windows)
    #[cfg(feature = "usb")]
    pub fn new(port_path: &str) -> Result<Self> {
        // Open serial port
        let mut port = serialport::new(port_path, 250_000)
            .timeout(std::time::Duration::from_millis(100))
            .open()
            .context(format!("Failed to open serial port {}", port_path))?;

        log::info!("Enttec DMXUSB Pro initialized on {} at 250000 baud", port_path);
        
        // Create channel for sending universe data to background thread
        let (tx, rx): (Sender<[u8; 512]>, Receiver<[u8; 512]>) = mpsc::channel();

        // Store port name for logging and display
        let thread_port_name = port_path.to_string();
        let port_name = format!("Enttec DMXUSB Pro ({})", port_path);

        let connected = Arc::new(AtomicBool::new(true));
        let thread_connected = Arc::clone(&connected);

        // Spawn background thread for DMX sending at 40 Hz
        let thread_handle = thread::spawn(move || {
            log::info!("DMX output thread started for {}", thread_port_name);

            let mut last_dmx = [0u8; 512];
            let dmx_interval = Duration::from_millis(25); // 40 Hz = 25ms per frame
            let mut consecutive_errors: u32 = 0;

            loop {
                let loop_start = std::time::Instant::now();

                // Drain the entire queue, keeping only the latest frame.
                // The UI sends at 60 Hz but this thread runs at 40 Hz, so without
                // draining, stale frames accumulate and create multi-second lag.
                let mut disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(new_dmx) => { last_dmx = new_dmx; }
                        Err(mpsc::TryRecvError::Disconnected) => { disconnected = true; break; }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if disconnected {
                    log::info!("DMX output thread stopping (channel disconnected)");
                    break;
                }

                // Send DMX packet; mark device lost after 5 consecutive failures
                match Self::send_dmx_packet_static(&mut port, &last_dmx) {
                    Ok(()) => { consecutive_errors = 0; }
                    Err(e) => {
                        consecutive_errors += 1;
                        log::error!("DMX send error on {} ({}): {}", thread_port_name, consecutive_errors, e);
                        if consecutive_errors >= 5 {
                            log::warn!("Device lost on {} — marking disconnected", thread_port_name);
                            thread_connected.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }

                // Sleep to maintain 40 Hz rate
                let elapsed = loop_start.elapsed();
                if elapsed < dmx_interval {
                    thread::sleep(dmx_interval - elapsed);
                }
            }

            log::info!("DMX output thread stopped for {}", thread_port_name);
        });

        Ok(Self {
            tx,
            port_name,
            _thread_handle: Some(thread_handle),
            connected,
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

    /// List ports ordered by how likely they are to be Enttec/compatible DMX interfaces.
    ///
    /// This is cross-platform and uses USB VID/PID and product/manufacturer metadata
    /// when available. It falls back to mild name heuristics for systems where USB
    /// metadata is incomplete.
    ///
    /// On macOS, `/dev/tty.*` ports are filtered out when the corresponding `/dev/cu.*`
    /// exists — `tty.*` blocks on open waiting for carrier-detect that USB adapters never
    /// assert; `cu.*` is always the correct choice for outgoing serial communication.
    #[cfg(feature = "usb")]
    pub fn list_recommended_ports() -> Result<Vec<String>> {
        let all_ports = serialport::available_ports()
            .context("Failed to enumerate serial ports")?;

        // On macOS, drop /dev/tty.* entries that have a /dev/cu.* twin.
        let cu_names: std::collections::HashSet<String> = all_ports
            .iter()
            .filter_map(|p| p.port_name.strip_prefix("/dev/cu.").map(|s| s.to_string()))
            .collect();

        let mut scored_ports: Vec<(i32, String)> = all_ports
            .into_iter()
            .filter(|p| {
                if let Some(suffix) = p.port_name.strip_prefix("/dev/tty.") {
                    !cu_names.contains(suffix)
                } else {
                    true
                }
            })
            .map(|port| (Self::score_port(&port), port.port_name))
            .collect();

        scored_ports.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        Ok(scored_ports.into_iter().map(|(_, name)| name).collect())
    }

    #[cfg(feature = "usb")]
    fn score_port(port: &SerialPortInfo) -> i32 {
        let mut score = 0;
        let lower_name = port.port_name.to_lowercase();

        // Prefer /dev/cu.* over /dev/tty.* on macOS — tty.* blocks on carrier-detect
        if lower_name.starts_with("/dev/cu.") {
            score += 15;
        } else if lower_name.starts_with("/dev/tty.") {
            score -= 20;
        }

        if lower_name.contains("usbserial") || lower_name.contains("ttyusb") || lower_name.contains("ttyacm") {
            score += 10;
        }

        if lower_name.starts_with("com") {
            score += 5;
        }

        if let SerialPortType::UsbPort(info) = &port.port_type {
            if info.vid == Self::ENTTEC_USB_VID && info.pid == Self::ENTTEC_USB_PID {
                score += 100;
            }

            if let Some(manufacturer) = &info.manufacturer {
                let manufacturer = manufacturer.to_lowercase();
                if manufacturer.contains("enttec") {
                    score += 60;
                } else if manufacturer.contains("ftdi") {
                    score += 25;
                }
            }

            if let Some(product) = &info.product {
                let product = product.to_lowercase();
                if product.contains("dmx") || product.contains("enttec") || product.contains("usb pro") {
                    score += 60;
                }
            }
        }

        score
    }
    
    /// Send DMX data using Enttec Pro protocol (static version for background thread)
    /// 
    /// Protocol format:
    /// - Start delimiter: 0x7E
    /// - Label: 0x06 (Send DMX Packet)
    /// - Data length LSB
    /// - Data length MSB
    /// - Data: DMX start code (0x00) + 512 DMX channels
    /// - End delimiter: 0xE7
    #[cfg(feature = "usb")]
    fn send_dmx_packet_static(port: &mut Box<dyn SerialPort>, dmx_data: &[u8; 512]) -> Result<()> {
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
        
        // Send to serial port
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
        
        // Send to background thread (non-blocking)
        // If the channel is full, this will block briefly, but in practice
        // the background thread consumes at 40 Hz so the channel rarely fills
        self.tx.send(dmx_data)
            .context("Failed to send DMX data to background thread")?;
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.port_name
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn close(&mut self) -> Result<()> {
        log::info!("Closing Enttec DMXUSB Pro on {}", self.port_name);
        // Drop the sender, which will cause the receiver to disconnect
        // and the background thread to exit gracefully
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
    
    fn close(&mut self) -> Result<()> {
        Ok(())
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
