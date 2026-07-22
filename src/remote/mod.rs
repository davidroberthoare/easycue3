//! Remote control — embedded web server + phone client bridge.
//!
//! Architecture: the desktop app stays the sole owner of engine state. The
//! server (own tokio runtime thread, `server.rs`) only enqueues protocol
//! commands and fans out state JSON; `glue.rs` runs once per egui frame to
//! drain commands into the engine and diff/publish state back out.

pub mod glue;
pub mod protocol;
mod server;

use anyhow::Result;
use protocol::ClientMessage;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

pub const DEFAULT_PORT: u16 = 7373;

/// Persisted remote-control settings (eframe storage, not the show file).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteSettings {
    pub enabled: bool,
    pub port: u16,
    /// Shared PIN; empty disables auth.
    pub pin: String,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            pin: String::new(),
        }
    }
}

/// Handle to a running remote server. Dropping it shuts the server down.
pub struct RemoteServer {
    pub port: u16,
    cmd_rx: tokio::sync::mpsc::UnboundedReceiver<ClientMessage>,
    broadcast: tokio::sync::broadcast::Sender<String>,
    snapshot: Arc<RwLock<String>>,
    clients: Arc<AtomicUsize>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    mdns: Option<mdns_sd::ServiceDaemon>,
    /// Per-frame change-detection state, owned by the glue layer.
    pub(crate) shadow: glue::Shadow,
}

impl RemoteServer {
    pub fn start(port: u16, pin: &str, egui_ctx: egui::Context) -> Result<Self> {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        // 256 pending messages per client before a lagged client is resynced.
        let (broadcast, _) = tokio::sync::broadcast::channel(256);
        let snapshot = Arc::new(RwLock::new(String::new()));
        let clients = Arc::new(AtomicUsize::new(0));

        let shared = server::ServerShared {
            cmd_tx,
            snapshot: Arc::clone(&snapshot),
            broadcast: broadcast.clone(),
            pin: pin.to_string(),
            egui_ctx,
            clients: Arc::clone(&clients),
        };

        let (thread, shutdown, bound_port) = server::spawn(port, shared)?;
        let mdns = register_mdns(bound_port);

        Ok(Self {
            port: bound_port,
            cmd_rx,
            broadcast,
            snapshot,
            clients,
            shutdown: Some(shutdown),
            thread: Some(thread),
            mdns,
            shadow: glue::Shadow::default(),
        })
    }

    pub fn client_count(&self) -> usize {
        self.clients.load(Ordering::Relaxed)
    }

    /// Take all pending client commands (called once per frame).
    pub fn drain_commands(&mut self) -> Vec<ClientMessage> {
        let mut out = Vec::new();
        while let Ok(msg) = self.cmd_rx.try_recv() {
            out.push(msg);
        }
        out
    }

    /// Fan an incremental state message out to all connected sockets.
    pub fn publish(&self, json: String) {
        // Err just means no connected clients — fine.
        let _ = self.broadcast.send(json);
    }

    /// Replace the cached full snapshot served to new connections and REST.
    pub fn set_snapshot(&self, json: String) {
        if let Ok(mut guard) = self.snapshot.write() {
            *guard = json;
        }
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        if let Some(mdns) = self.mdns.take() {
            let _ = mdns.shutdown();
        }
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        log::info!("[remote] server shut down");
    }
}

/// Best-effort LAN IP discovery (UDP connect trick — no packets are sent).
pub fn local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// Advertise the server over mDNS so phones can find `easycue3.local`.
/// Failure is non-fatal — the QR code / IP URL still works.
fn register_mdns(port: u16) -> Option<mdns_sd::ServiceDaemon> {
    let daemon = match mdns_sd::ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("[remote] mDNS daemon failed to start: {}", e);
            return None;
        }
    };
    let ip = local_ip();
    let addr_str = ip.map(|i| i.to_string()).unwrap_or_default();
    let service = mdns_sd::ServiceInfo::new(
        "_easycue3._tcp.local.",
        "EasyCue3",
        "easycue3.local.",
        addr_str.as_str(),
        port,
        None,
    );
    match service {
        Ok(info) => {
            let info = info.enable_addr_auto();
            match daemon.register(info) {
                Ok(()) => {
                    log::info!("[remote] mDNS registered as easycue3.local:{}", port);
                    Some(daemon)
                }
                Err(e) => {
                    log::warn!("[remote] mDNS registration failed: {}", e);
                    let _ = daemon.shutdown();
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("[remote] mDNS service info invalid: {}", e);
            let _ = daemon.shutdown();
            None
        }
    }
}
