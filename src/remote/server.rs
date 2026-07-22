//! Embedded axum server: static client, REST endpoints, WebSocket sync.
//!
//! Runs on its own tokio runtime thread so the egui main thread is never
//! blocked. All engine mutation happens app-side: handlers only enqueue
//! `ClientMessage`s and wake the UI loop with a repaint request.

use super::protocol::{ClientMessage, RestChannelBody, RestCommandBody};
use anyhow::{Context as _, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Shared state handed to every handler.
#[derive(Clone)]
pub struct ServerShared {
    /// Commands flowing toward the app; drained once per frame.
    pub cmd_tx: tokio::sync::mpsc::UnboundedSender<ClientMessage>,
    /// Latest full snapshot JSON (the `snapshot` envelope), kept fresh by the app.
    pub snapshot: Arc<RwLock<String>>,
    /// Fan-out of incremental state messages to connected sockets.
    pub broadcast: tokio::sync::broadcast::Sender<String>,
    /// Optional shared PIN; empty means no auth.
    pub pin: String,
    /// Cloned egui context — lets remote commands wake an idle UI loop.
    pub egui_ctx: egui::Context,
    /// Connected WebSocket client count (for the desktop settings dialog).
    pub clients: Arc<AtomicUsize>,
}

impl ServerShared {
    fn authorized(&self, headers: &HeaderMap, query: &HashMap<String, String>) -> bool {
        if self.pin.is_empty() {
            return true;
        }
        let header_token = headers.get("x-easycue-token").and_then(|v| v.to_str().ok());
        let query_token = query.get("token").map(String::as_str);
        header_token == Some(self.pin.as_str()) || query_token == Some(self.pin.as_str())
    }

    /// Enqueue a command for the app and wake the UI loop.
    fn enqueue(&self, msg: ClientMessage) {
        if self.cmd_tx.send(msg).is_ok() {
            self.egui_ctx.request_repaint();
        }
    }
}

// --- Embedded client assets ----------------------------------------------

struct Asset {
    body: &'static [u8],
    mime: &'static str,
}

fn asset(path: &str) -> Option<Asset> {
    macro_rules! file {
        ($name:literal, $mime:literal) => {
            Asset {
                body: include_bytes!(concat!("../../remote_client/", $name)),
                mime: $mime,
            }
        };
    }
    Some(match path {
        "" | "index.html" => file!("index.html", "text/html; charset=utf-8"),
        "app.js" => file!("app.js", "application/javascript"),
        "framework7-bundle.min.css" => file!("framework7-bundle.min.css", "text/css"),
        "framework7-bundle.min.js" => {
            file!("framework7-bundle.min.js", "application/javascript")
        }
        "manifest.json" => file!("manifest.json", "application/manifest+json"),
        "sw.js" => file!("sw.js", "application/javascript"),
        "icon-192.png" => file!("icon-192.png", "image/png"),
        "icon-512.png" => file!("icon-512.png", "image/png"),
        _ => return None,
    })
}

async fn serve_index() -> Response {
    serve_asset_named("").await
}

async fn serve_asset(axum::extract::Path(file): axum::extract::Path<String>) -> Response {
    serve_asset_named(&file).await
}

async fn serve_asset_named(name: &str) -> Response {
    match asset(name) {
        Some(a) => (
            [
                (header::CONTENT_TYPE, a.mime),
                // Shell caching is the service worker's job, not HTTP's.
                (header::CACHE_CONTROL, "no-cache"),
            ],
            a.body,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// --- REST ------------------------------------------------------------------

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "invalid or missing token" })),
    )
        .into_response()
}

fn queued() -> Response {
    Json(serde_json::json!({ "ok": true, "queued": true })).into_response()
}

async fn api_ping(
    State(s): State<ServerShared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if !s.authorized(&headers, &q) {
        return unauthorized();
    }
    Json(serde_json::json!({ "ok": true, "app": "easycue3", "version": env!("CARGO_PKG_VERSION") }))
        .into_response()
}

async fn api_state(
    State(s): State<ServerShared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if !s.authorized(&headers, &q) {
        return unauthorized();
    }
    let body = s.snapshot.read().map(|g| g.clone()).unwrap_or_default();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

macro_rules! simple_post {
    ($fn_name:ident, $msg:expr) => {
        async fn $fn_name(
            State(s): State<ServerShared>,
            headers: HeaderMap,
            Query(q): Query<HashMap<String, String>>,
        ) -> Response {
            if !s.authorized(&headers, &q) {
                return unauthorized();
            }
            s.enqueue($msg);
            queued()
        }
    };
}

simple_post!(api_cue_go, ClientMessage::CueGo);
simple_post!(api_cue_back, ClientMessage::CueBack);
simple_post!(api_cue_stop, ClientMessage::CueStop);

async fn api_channel(
    State(s): State<ServerShared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<RestChannelBody>,
) -> Response {
    if !s.authorized(&headers, &q) {
        return unauthorized();
    }
    s.enqueue(ClientMessage::SetChannels {
        universe: body.universe,
        channels: vec![super::protocol::ChannelValue {
            channel: body.channel,
            value: body.value,
        }],
    });
    queued()
}

async fn api_command(
    State(s): State<ServerShared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<RestCommandBody>,
) -> Response {
    if !s.authorized(&headers, &q) {
        return unauthorized();
    }
    s.enqueue(ClientMessage::CommandLine {
        text: body.text,
        context: body.context,
    });
    queued()
}

// --- WebSocket ---------------------------------------------------------------

async fn ws_upgrade(
    State(s): State<ServerShared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !s.authorized(&headers, &q) {
        return unauthorized();
    }
    upgrade.on_upgrade(move |socket| ws_session(socket, s))
}

async fn ws_session(socket: WebSocket, s: ServerShared) {
    s.clients.fetch_add(1, Ordering::Relaxed);
    s.egui_ctx.request_repaint(); // let the desktop UI show the new client count
    log::info!("[remote] WebSocket client connected");

    let (mut tx, mut rx) = socket.split();
    let mut updates = s.broadcast.subscribe();

    // Full snapshot first so the client starts authoritative.
    let snap = s.snapshot.read().map(|g| g.clone()).unwrap_or_default();
    if !snap.is_empty() && tx.send(Message::Text(snap.into())).await.is_err() {
        s.clients.fetch_sub(1, Ordering::Relaxed);
        return;
    }

    loop {
        tokio::select! {
            update = updates.recv() => {
                match update {
                    Ok(json) => {
                        if tx.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        // Slow phone: resync with a fresh snapshot instead of dying.
                        log::warn!("[remote] client lagged {} messages — resyncing", n);
                        let snap = s.snapshot.read().map(|g| g.clone()).unwrap_or_default();
                        if tx.send(Message::Text(snap.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = rx.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(msg) => s.enqueue(msg),
                            Err(e) => log::warn!("[remote] bad client message: {} ({})", e, text),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {} // ping/pong/binary — ignore
                    Some(Err(e)) => {
                        log::debug!("[remote] WebSocket error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    s.clients.fetch_sub(1, Ordering::Relaxed);
    s.egui_ctx.request_repaint();
    log::info!("[remote] WebSocket client disconnected");
}

// --- Server lifecycle ----------------------------------------------------------

pub fn router(shared: ServerShared) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/{file}", get(serve_asset))
        .route("/api/ping", get(api_ping))
        .route("/api/state", get(api_state))
        .route("/api/cue/go", post(api_cue_go))
        .route("/api/cue/back", post(api_cue_back))
        .route("/api/cue/stop", post(api_cue_stop))
        .route("/api/channel", post(api_channel))
        .route("/api/command", post(api_command))
        .route("/ws", get(ws_upgrade))
        .with_state(shared)
}

/// Spawn the server on a dedicated runtime thread. Returns once the port is
/// bound (or fails fast if it can't be) with the shutdown handle and the
/// actually-bound port (differs from `port` when 0 was requested).
pub fn spawn(
    port: u16,
    shared: ServerShared,
) -> Result<(
    std::thread::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
    u16,
)> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u16>>();

    let handle = std::thread::Builder::new()
        .name("easycue3-remote".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .context("building tokio runtime for remote server")
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };

            runtime.block_on(async move {
                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
                let listener = match tokio::net::TcpListener::bind(addr)
                    .await
                    .with_context(|| format!("binding remote server to {}", addr))
                {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let bound_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
                let _ = ready_tx.send(Ok(bound_port));
                log::info!("[remote] server listening on 0.0.0.0:{}", bound_port);

                let app = router(shared);
                if let Err(e) = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await
                {
                    log::error!("[remote] server error: {}", e);
                }
                log::info!("[remote] server stopped");
            });
        })
        .context("spawning remote server thread")?;

    // Fail fast on bind errors so the settings dialog can show them.
    match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(bound_port)) => Ok((handle, shutdown_tx, bound_port)),
        Ok(Err(e)) => {
            let _ = handle.join();
            Err(e)
        }
        Err(_) => Err(anyhow::anyhow!("remote server did not start within 5s")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    struct TestServer {
        port: u16,
        cmd_rx: tokio::sync::mpsc::UnboundedReceiver<ClientMessage>,
        _shutdown: tokio::sync::oneshot::Sender<()>,
        _thread: std::thread::JoinHandle<()>,
        broadcast: tokio::sync::broadcast::Sender<String>,
    }

    fn start_test_server(pin: &str) -> TestServer {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (broadcast, _) = tokio::sync::broadcast::channel(16);
        let snapshot = Arc::new(RwLock::new(
            r#"{"type":"snapshot","payload":{"test":true}}"#.to_string(),
        ));
        let shared = ServerShared {
            cmd_tx,
            snapshot,
            broadcast: broadcast.clone(),
            pin: pin.to_string(),
            egui_ctx: egui::Context::default(),
            clients: Arc::new(AtomicUsize::new(0)),
        };
        let (thread, shutdown, port) = spawn(0, shared).expect("server should start");
        TestServer {
            port,
            cmd_rx,
            _shutdown: shutdown,
            _thread: thread,
            broadcast,
        }
    }

    /// Send one raw HTTP/1.1 request and return (status_line, body).
    fn http(port: u16, request: &str) -> (String, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
        let text = String::from_utf8_lossy(&response).to_string();
        let status = text.lines().next().unwrap_or_default().to_string();
        let body = text
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        (status, body)
    }

    fn get(port: u16, path: &str) -> (String, String) {
        http(
            port,
            &format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                path
            ),
        )
    }

    fn post(port: u16, path: &str, token: &str, body: &str) -> (String, String) {
        http(
            port,
            &format!(
                "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
                 x-easycue-token: {}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\n\r\n{}",
                path,
                token,
                body.len(),
                body
            ),
        )
    }

    #[test]
    fn serves_static_shell_without_auth() {
        let server = start_test_server("1234");
        let (status, body) = get(server.port, "/");
        assert!(status.contains("200"), "status was: {}", status);
        assert!(body.contains("EasyCue3 Remote"));
        let (status, _) = get(server.port, "/app.js");
        assert!(status.contains("200"));
        let (status, _) = get(server.port, "/nonexistent.txt");
        assert!(status.contains("404"));
    }

    #[test]
    fn rest_auth_and_state() {
        let server = start_test_server("1234");
        // Wrong/missing token → 401
        let (status, _) = get(server.port, "/api/ping");
        assert!(status.contains("401"), "status was: {}", status);
        let (status, _) = get(server.port, "/api/state?token=wrong");
        assert!(status.contains("401"));
        // Correct token via query param
        let (status, body) = get(server.port, "/api/state?token=1234");
        assert!(status.contains("200"));
        assert!(body.contains("snapshot"));
    }

    #[test]
    fn rest_commands_enqueue() {
        let mut server = start_test_server("");
        let (status, _) = post(server.port, "/api/cue/go", "", "");
        assert!(status.contains("200"), "status was: {}", status);
        let (status, _) = post(
            server.port,
            "/api/channel",
            "",
            r#"{"channel": 5, "value": 80}"#,
        );
        assert!(status.contains("200"), "status was: {}", status);
        let (status, _) = post(
            server.port,
            "/api/command",
            "",
            r#"{"text": "1@50", "context": "channel"}"#,
        );
        assert!(status.contains("200"), "status was: {}", status);

        assert!(matches!(server.cmd_rx.try_recv(), Ok(ClientMessage::CueGo)));
        match server.cmd_rx.try_recv() {
            Ok(ClientMessage::SetChannels { universe, channels }) => {
                assert_eq!(universe, 1);
                assert_eq!(channels.len(), 1);
                assert_eq!(channels[0].channel, 5);
                assert_eq!(channels[0].value, 80);
            }
            other => panic!("expected SetChannels, got {:?}", other),
        }
        match server.cmd_rx.try_recv() {
            Ok(ClientMessage::CommandLine { text, context }) => {
                assert_eq!(text, "1@50");
                assert_eq!(context, super::super::protocol::RemoteCmdContext::Channel);
            }
            other => panic!("expected CommandLine, got {:?}", other),
        }
    }

    /// Minimal WebSocket client: handshake, then read/write single text frames.
    struct WsClient {
        stream: TcpStream,
    }

    impl WsClient {
        fn connect(port: u16, path: &str) -> Result<Self, String> {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let request = format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\n\
                 Connection: Upgrade\r\nSec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==\r\n\
                 Sec-WebSocket-Version: 13\r\n\r\n",
                path
            );
            stream.write_all(request.as_bytes()).unwrap();
            // Read the HTTP response head byte-by-byte until \r\n\r\n.
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(1) => head.push(byte[0]),
                    _ => break,
                }
            }
            let head = String::from_utf8_lossy(&head).to_string();
            if head.contains("101") {
                Ok(Self { stream })
            } else {
                Err(head.lines().next().unwrap_or_default().to_string())
            }
        }

        fn read_text(&mut self) -> String {
            let mut hdr = [0u8; 2];
            self.stream.read_exact(&mut hdr).unwrap();
            assert_eq!(hdr[0], 0x81, "expected FIN+text frame");
            let mut len = (hdr[1] & 0x7f) as u64;
            if len == 126 {
                let mut ext = [0u8; 2];
                self.stream.read_exact(&mut ext).unwrap();
                len = u16::from_be_bytes(ext) as u64;
            } else if len == 127 {
                let mut ext = [0u8; 8];
                self.stream.read_exact(&mut ext).unwrap();
                len = u64::from_be_bytes(ext);
            }
            let mut payload = vec![0u8; len as usize];
            self.stream.read_exact(&mut payload).unwrap();
            String::from_utf8(payload).unwrap()
        }

        fn send_text(&mut self, text: &str) {
            let payload = text.as_bytes();
            let mask = [0x11u8, 0x22, 0x33, 0x44];
            let mut frame = vec![0x81u8];
            if payload.len() < 126 {
                frame.push(0x80 | payload.len() as u8);
            } else {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            }
            frame.extend_from_slice(&mask);
            for (i, b) in payload.iter().enumerate() {
                frame.push(b ^ mask[i % 4]);
            }
            self.stream.write_all(&frame).unwrap();
        }
    }

    #[test]
    fn websocket_snapshot_broadcast_and_commands() {
        let mut server = start_test_server("42");

        // Wrong token: upgrade refused.
        assert!(WsClient::connect(server.port, "/ws?token=nope").is_err());

        let mut ws = WsClient::connect(server.port, "/ws?token=42").expect("ws connect");
        // First message is always the cached snapshot.
        let first = ws.read_text();
        assert!(first.contains("snapshot"), "got: {}", first);

        // Broadcasts reach the socket.
        server
            .broadcast
            .send(r#"{"type":"playback","payload":{}}"#.to_string())
            .unwrap();
        let pushed = ws.read_text();
        assert!(pushed.contains("playback"));

        // Client command lands in the app-bound queue.
        ws.send_text(r#"{"type":"cue_goto","payload":{"number":5.5}}"#);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match server.cmd_rx.try_recv() {
                Ok(ClientMessage::CueGoto { number }) => {
                    assert!((number - 5.5).abs() < 0.001);
                    break;
                }
                Ok(other) => panic!("unexpected command: {:?}", other),
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("command never arrived: {}", e),
            }
        }
    }
}
