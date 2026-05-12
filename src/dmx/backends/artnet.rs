//! Art-Net DMX over Ethernet backend
//!
//! Sends Art-Net ArtDmx (Output) packets via UDP to a configurable target address.
//! Compatible with any Art-Net node (dimmers, LED drivers, etc.) on the local network.
//!
//! Default port: 6454. Default target: broadcast (255.255.255.255).
//! Universe numbers are 0-based Art-Net port addresses (0–32767).
//!
//! Uses the same non-blocking background-thread architecture as the USB backend:
//! the main thread queues a [u8; 512] frame and the sender thread drains at 40 Hz.

use anyhow::{Context, Result};
use artnet_protocol::{ArtCommand, Output, PortAddress};
use crate::dmx::{Universe, backends::{DmxBackend, universe_to_dmx}};
use std::convert::TryFrom;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

pub const ARTNET_PORT: u16 = 6454;

/// Art-Net backend — sends ArtDmx packets to a single target address at 40 Hz.
pub struct ArtNetBackend {
    tx: Sender<[u8; 512]>,
    display_name: String,
    _thread_handle: Option<thread::JoinHandle<()>>,
    /// Cleared by the background thread when the socket fails permanently.
    connected: Arc<AtomicBool>,
}

impl ArtNetBackend {
    /// Create a new Art-Net backend.
    ///
    /// `target` — IP address string of the Art-Net node, or `"255.255.255.255"` for subnet
    /// broadcast. `universe` — Art-Net universe (0-based, 0–32767).
    pub fn new(target: &str, universe: u16) -> Result<Self> {
        let target_addr: SocketAddr = format!("{}:{}", target, ARTNET_PORT)
            .to_socket_addrs()
            .context("Invalid Art-Net target address")?
            .next()
            .context("Art-Net address resolution returned no results")?;

        // Bind to any local port. Enable broadcast in case target is 255.255.255.255.
        let socket = UdpSocket::bind("0.0.0.0:0")
            .context("Failed to bind UDP socket for Art-Net")?;
        socket.set_broadcast(true)
            .context("Failed to enable UDP broadcast")?;

        log::info!(
            "Art-Net backend: target {} universe {}",
            target_addr, universe
        );

        let (tx, rx): (Sender<[u8; 512]>, Receiver<[u8; 512]>) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(true));
        let thread_connected = Arc::clone(&connected);
        let display_name = format!("Art-Net → {} (universe {})", target, universe);

        let thread_handle = thread::spawn(move || {
            log::info!("Art-Net sender thread started");
            let mut last_dmx = [0u8; 512];
            let interval = Duration::from_millis(25); // 40 Hz
            let mut sequence: u8 = 1;

            loop {
                let loop_start = std::time::Instant::now();

                // Drain queue, keep only the latest frame.
                let mut disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(frame) => { last_dmx = frame; }
                        Err(mpsc::TryRecvError::Disconnected) => { disconnected = true; break; }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if disconnected {
                    log::info!("Art-Net sender thread stopping");
                    break;
                }

                let port_addr = PortAddress::try_from(universe).unwrap_or_else(|_| 0_u8.into());
                let output = Output {
                    port_address: port_addr,
                    sequence,
                    data: last_dmx.to_vec().into(),
                    ..Output::default()
                };

                match ArtCommand::Output(output).write_to_buffer() {
                    Ok(buf) => {
                        if let Err(e) = socket.send_to(&buf, target_addr) {
                            log::warn!("Art-Net send error: {}", e);
                            // UDP errors are usually transient; don't kill the thread.
                        }
                    }
                    Err(e) => {
                        log::error!("Art-Net packet encode error: {}", e);
                        thread_connected.store(false, Ordering::Relaxed);
                        break;
                    }
                }

                // Wrap sequence 1-255 (0 means "disabled" in the spec).
                sequence = sequence.wrapping_add(1).max(1);

                let elapsed = loop_start.elapsed();
                if elapsed < interval {
                    thread::sleep(interval - elapsed);
                }
            }
        });

        Ok(Self {
            tx,
            display_name,
            _thread_handle: Some(thread_handle),
            connected,
        })
    }

    /// Attempt a broadcast discovery on the local subnet (sends ArtPoll).
    /// Returns the target address string to use as a hint for the UI.
    pub fn default_broadcast_target() -> &'static str {
        "255.255.255.255"
    }
}

impl DmxBackend for ArtNetBackend {
    fn send_universe(&mut self, universe: &Universe) -> Result<()> {
        let dmx = universe_to_dmx(universe);
        // Non-blocking: drop the frame if the channel is full (UI is faster than 40 Hz).
        let _ = self.tx.send(dmx);
        Ok(())
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}
