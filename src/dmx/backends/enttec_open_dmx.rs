//! Enttec Open DMX USB backend
//!
//! Unlike the DMX USB Pro, the Open DMX USB has no onboard microcontroller.
//! The host CPU must produce the DMX break by toggling the serial break line,
//! then write the raw DMX frame at 250,000 baud — no 0x7E/0xE7 framing.
//!
//! Protocol per frame:
//!   1. Assert BREAK via set_break()
//!   2. Clear BREAK (Mark After Break) via clear_break()
//!   3. Write: 0x00 (start code) + 512 channel bytes at 250 kbaud
//!
//! Max refresh rate is ~30 Hz due to USB latency overhead.

use anyhow::{Result, Context};
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};
use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

#[cfg(feature = "usb")]
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};
#[cfg(feature = "usb")]
use serialport::{SerialPortInfo, SerialPortType};

/// Enttec Open DMX USB backend with threaded sending
pub struct EnttecOpenDmxBackend {
    tx: Option<Sender<[u8; 512]>>,
    port_name: String,
    thread_handle: Option<thread::JoinHandle<()>>,
    connected: Arc<AtomicBool>,
}

impl Drop for EnttecOpenDmxBackend {
    fn drop(&mut self) {
        // Drop tx first: disconnects the channel so the output thread exits its loop.
        // Then join the thread so the serial port FD is released before we return.
        drop(self.tx.take());
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl EnttecOpenDmxBackend {
    #[cfg(feature = "usb")]
    const ENTTEC_USB_VID: u16 = 0x0403;
    // Same FTDI chip as the Pro — distinguished by product string at detection time
    #[cfg(feature = "usb")]
    const ENTTEC_OPEN_DMX_PID: u16 = 0x6001;

    #[cfg(feature = "usb")]
    pub fn new(port_path: &str) -> Result<Self> {
        let mut port = serialport::new(port_path, 250_000)
            .data_bits(DataBits::Eight)
            .flow_control(FlowControl::None)
            .parity(Parity::None)
            .stop_bits(StopBits::Two)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| {
                let hint = if e.to_string().contains("busy") || e.to_string().contains("Permission") {
                    " (ModemManager may be probing it — wait a few seconds and try again)"
                } else {
                    ""
                };
                anyhow::anyhow!("Failed to open {}: {}{}", port_path, e, hint)
            })?;

        // Open DMX on Linux can inherit stale FTDI control-line state from the
        // previous client. Clearing RTS/DTR and purging buffers before the
        // send loop makes the adapter start driving the DMX line reliably.
        port.write_request_to_send(false)
            .context("Failed to clear RTS on Open DMX port")?;
        port.write_data_terminal_ready(false)
            .context("Failed to clear DTR on Open DMX port")?;
        port.clear(ClearBuffer::All)
            .context("Failed to clear Open DMX serial buffers")?;
        thread::sleep(Duration::from_millis(1));

        log::info!("Enttec Open DMX USB initialized on {} at 250000 baud", port_path);

        let (tx, rx): (Sender<[u8; 512]>, Receiver<[u8; 512]>) = mpsc::channel();
        let thread_port_name = port_path.to_string();
        let port_name = format!("Enttec Open DMX USB ({})", port_path);

        let connected = Arc::new(AtomicBool::new(true));
        let thread_connected = Arc::clone(&connected);

        let thread_handle = thread::spawn(move || {
            log::info!("Open DMX output thread started for {}", thread_port_name);

            let mut last_dmx = [0u8; 512];
            // 30 Hz — Open DMX USB tops out around here due to USB round-trip overhead
            let dmx_interval = Duration::from_millis(33);
            let mut consecutive_errors: u32 = 0;
            let mut port = port;

            loop {
                let loop_start = std::time::Instant::now();

                // Drain queue, keep only the latest frame
                let mut disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(new_dmx) => { last_dmx = new_dmx; }
                        Err(mpsc::TryRecvError::Disconnected) => { disconnected = true; break; }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if disconnected {
                    log::info!("Open DMX output thread stopping (channel disconnected)");
                    break;
                }

                match Self::send_dmx_frame(&mut port, &last_dmx) {
                    Ok(()) => {
                        consecutive_errors = 0;
                    }
                    Err(e) => {
                        consecutive_errors += 1;
                        log::error!("Open DMX send error on {} ({}): {:#}", thread_port_name, consecutive_errors, e);
                        if consecutive_errors >= 5 {
                            log::warn!("Device lost on {} — marking disconnected", thread_port_name);
                            thread_connected.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }

                let elapsed = loop_start.elapsed();
                if elapsed < dmx_interval {
                    thread::sleep(dmx_interval - elapsed);
                }
            }

            log::info!("Open DMX output thread stopped for {}", thread_port_name);
        });

        Ok(Self {
            tx: Some(tx),
            port_name,
            thread_handle: Some(thread_handle),
            connected,
        })
    }

    #[cfg(not(feature = "usb"))]
    pub fn new(_port_path: &str) -> Result<Self> {
        anyhow::bail!("USB support not enabled. Rebuild with --features usb")
    }

    /// Send one raw DMX frame: BREAK → MAB → start code + 512 channel bytes.
    ///
    /// Uses TIOCSBRK/TIOCCBRK with explicit timing. QLC+ uses the same approach
    /// on Linux and it works reliably with the FTDI ftdi_sio driver.
    #[cfg(feature = "usb")]
    fn send_dmx_frame(port: &mut Box<dyn SerialPort>, dmx_data: &[u8; 512]) -> Result<()> {
        port.set_break()
            .context("Failed to assert serial BREAK")?;
        std::thread::sleep(Duration::from_micros(110));

        port.clear_break()
            .context("Failed to clear serial BREAK")?;
        std::thread::sleep(Duration::from_micros(40));

        let mut frame = [0u8; 513];
        frame[0] = 0x00;
        frame[1..].copy_from_slice(dmx_data);

        port.write_all(&frame)
            .context("Failed to write DMX frame")?;
        port.flush()
            .context("Failed to flush DMX frame")?;

        Ok(())
    }

    /// List ports likely to be an Open DMX USB.
    ///
    /// The Open DMX USB uses the same FTDI VID/PID (0x0403/0x6001) as the Pro.
    /// Ports whose product string contains "Open" are ranked higher; otherwise
    /// the same heuristics as the Pro apply (ttyUSB*, FTDI manufacturer, etc.).
    #[cfg(feature = "usb")]
    pub fn list_recommended_ports() -> Result<Vec<String>> {
        let all_ports = serialport::available_ports()
            .context("Failed to enumerate serial ports")?;

        let cu_names: std::collections::HashSet<String> = all_ports
            .iter()
            .filter_map(|p| p.port_name.strip_prefix("/dev/cu.").map(|s| s.to_string()))
            .collect();

        let mut scored: Vec<(i32, String)> = all_ports
            .into_iter()
            .filter(|p| {
                if let Some(suffix) = p.port_name.strip_prefix("/dev/tty.") {
                    !cu_names.contains(suffix)
                } else {
                    true
                }
            })
            .filter(|p| matches!(p.port_type, SerialPortType::UsbPort(_)))
            .filter(|p| {
                let n = p.port_name.to_lowercase();
                !n.contains("bluetooth") && !n.contains("debug-console")
            })
            .map(|p| (Self::score_port(&p), p.port_name))
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        Ok(scored.into_iter().map(|(_, name)| name).collect())
    }

    #[cfg(feature = "usb")]
    fn score_port(port: &SerialPortInfo) -> i32 {
        let mut score = 0;
        let lower_name = port.port_name.to_lowercase();

        if lower_name.starts_with("/dev/cu.") {
            score += 15;
        } else if lower_name.starts_with("/dev/tty.") {
            score -= 20;
        }

        if lower_name.contains("bluetooth") {
            score -= 200;
        }

        if lower_name.contains("usbserial") || lower_name.contains("ttyusb") || lower_name.contains("ttyacm") {
            score += 10;
        }

        if lower_name.starts_with("com") {
            score += 5;
        }

        if let SerialPortType::UsbPort(info) = &port.port_type {
            if info.vid == Self::ENTTEC_USB_VID && info.pid == Self::ENTTEC_OPEN_DMX_PID {
                score += 60;
            }

            if let Some(manufacturer) = &info.manufacturer {
                let m = manufacturer.to_lowercase();
                if m.contains("enttec") {
                    score += 60;
                } else if m.contains("ftdi") {
                    score += 25;
                }
            }

            if let Some(product) = &info.product {
                let p = product.to_lowercase();
                // Strongly prefer ports whose product string names this device
                if p.contains("open dmx") {
                    score += 100;
                } else if p.contains("dmx") || p.contains("enttec") {
                    score += 40;
                } else if p.contains("ft232") || p.contains("ft231") || p.contains("usb serial") || p.contains("usb-serial") {
                    // Bare FTDI cables (e.g. DSD TECH's USB-to-DMX512) have no onboard
                    // microcontroller and report a generic chip product string instead
                    // of naming the device — same wiring/protocol as Open DMX USB.
                    score += 20;
                }
            }
        }

        score
    }
}

#[cfg(feature = "usb")]
impl DmxBackend for EnttecOpenDmxBackend {
    fn send_universe(&mut self, universe: &Universe) -> Result<()> {
        let dmx_data = universe_to_dmx(universe);
        if let Some(tx) = &self.tx {
            tx.send(dmx_data)
                .context("Failed to send DMX data to background thread")?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.port_name
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn close(&mut self) -> Result<()> {
        log::info!(
            "Closing Enttec Open DMX USB on {} (connected={})",
            self.port_name,
            self.connected.load(Ordering::Relaxed)
        );
        Ok(())
    }
}

#[cfg(not(feature = "usb"))]
impl DmxBackend for EnttecOpenDmxBackend {
    fn send_universe(&mut self, _universe: &Universe) -> Result<()> {
        anyhow::bail!("USB support not enabled")
    }

    fn name(&self) -> &str {
        "Enttec Open DMX USB (USB not enabled)"
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
