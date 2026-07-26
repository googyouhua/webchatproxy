use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::{accept_async, connect_async, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

use crate::types::{AppState, ExtSocket};

pub async fn start_ws_server(state: Arc<AppState>, port: u16, log_file: String) {
    let addr = format!("127.0.0.1:{}", port);

    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            let mut logger = crate::log::Logger::new(&log_file);
            logger.log(&format!("WebSocket server listening on {}", addr));

            while let Ok((stream, peer)) = listener.accept().await {
                logger.log(&format!("Connection from {}", peer));

                let state = state.clone();
                let lf = log_file.clone();
                tokio::spawn(async move {
                    let mut logger = crate::log::Logger::new(&lf);
                    match accept_async(stream).await {
                        Ok(ws_stream) => {
                            logger.log("WebSocket connection established");
                            handle_extension(state, ws_stream).await;
                        }
                        Err(e) => {
                            logger.log(&format!("WebSocket handshake failed: {}", e));
                        }
                    }
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let mut logger = crate::log::Logger::new(&log_file);
            logger.log(&format!("Port {} in use, connecting as secondary instance", port));
            connect_as_secondary(state, port, log_file).await;
        }
        Err(e) => {
            eprintln!("Failed to bind: {}", e);
        }
    }
}

async fn handle_extension(
    state: Arc<AppState>,
    ws_stream: WebSocketStream<tokio::net::TcpStream>,
) {
    let (write, mut read) = ws_stream.split();
    let conn_id = crate::types::next_conn_id();

    {
        let mut ext = state.extension_socket.lock().await;
        *ext = Some((conn_id, ExtSocket::Plain(write)));
    }

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let parsed: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => {
                        eprintln!("[ext-raw] {}", text);
                        continue;
                    }
                };

                if parsed.get("type").and_then(|v| v.as_str()) == Some("log") {
                    let msg = parsed.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    eprintln!("[ext-log] {}", msg);
                    let mut logger = crate::log::Logger::new("/tmp/ai-bridge.log");
                    logger.log(&format!("[ext-log] {}", msg));
                }

                if let Some(request_id) = parsed.get("requestId").and_then(|v| v.as_str()) {
                    let mut pending = state.pending.lock().await;
                    if let Some(sender) = pending.remove(request_id) {
                        let _ = sender.send(text);
                    } else {
                        eprintln!("[ext-unmatched] requestId={} text={}", request_id, text);
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    let mut ext = state.extension_socket.lock().await;
    if let Some((id, _)) = ext.as_ref() {
        if *id == conn_id {
            *ext = None;
        }
    }
}

async fn connect_as_secondary(
    state: Arc<AppState>,
    port: u16,
    _log_file: String,
) {
    let url = format!("ws://127.0.0.1:{}/?token={}&type=mcp", port, state.auth.token());
    let conn_id = crate::types::next_conn_id();

    match connect_async(&url).await {
        Ok((ws_stream, _)) => {
            let (write, mut read) = ws_stream.split();

            {
                let mut ext = state.extension_socket.lock().await;
                *ext = Some((conn_id, ExtSocket::Tls(write)));
            }

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            if parsed.get("type").and_then(|v| v.as_str()) == Some("log") {
                                let msg = parsed.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                eprintln!("[ext-log:secondary] {}", msg);
                                let mut logger = crate::log::Logger::new("/tmp/ai-bridge.log");
                                logger.log(&format!("[ext-log:secondary] {}", msg));
                            }
                            if let Some(request_id) = parsed.get("requestId").and_then(|v| v.as_str()) {
                                let mut pending = state.pending.lock().await;
                                if let Some(sender) = pending.remove(request_id) {
                                    let _ = sender.send(text);
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect as secondary: {}", e);
        }
    }

    let mut ext = state.extension_socket.lock().await;
    if let Some((id, _)) = ext.as_ref() {
        if *id == conn_id {
            *ext = None;
        }
    }
}

async fn send_via_ext_socket(
    ext: &mut ExtSocket,
    payload: &str,
) -> Result<(), String> {
    match ext {
        ExtSocket::Plain(ref mut write) => {
            write.send(Message::Text(payload.into()))
                .await
                .map_err(|e| format!("Send error: {}", e))
        }
        ExtSocket::Tls(ref mut write) => {
            write.send(Message::Text(payload.into()))
                .await
                .map_err(|e| format!("Send error: {}", e))
        }
    }
}

pub async fn send_to_extension_background(
    state: &Arc<AppState>,
    request_id: &str,
    payload: &serde_json::Value,
) -> Result<oneshot::Receiver<String>, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(request_id.to_string(), tx);
    }

    let payload_str = payload.to_string();

    {
        let mut ext_guard = state.extension_socket.lock().await;
        match ext_guard.as_mut() {
            Some((_, ext)) => {
                if let Err(e) = send_via_ext_socket(ext, &payload_str).await {
                    let mut pending = state.pending.lock().await;
                    pending.remove(request_id);
                    return Err(e);
                }
            }
            None => {
                let mut pending = state.pending.lock().await;
                pending.remove(request_id);
                return Err("No extension connected".to_string());
            }
        }
    }

    Ok(rx)
}

pub async fn send_to_extension(
    state: &Arc<AppState>,
    request_id: &str,
    payload: &serde_json::Value,
    timeout_secs: u64,
    _log_file: &str,
) -> Result<String, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(request_id.to_string(), tx);
    }

    let payload_str = payload.to_string();

    {
        let mut ext_guard = state.extension_socket.lock().await;
        match ext_guard.as_mut() {
            Some((_, ext)) => {
                if let Err(e) = send_via_ext_socket(ext, &payload_str).await {
                    let mut pending = state.pending.lock().await;
                    pending.remove(request_id);
                    return Err(e);
                }
            }
            None => {
                let mut pending = state.pending.lock().await;
                pending.remove(request_id);
                return Err("No extension connected".to_string());
            }
        }
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        rx,
    )
    .await
    {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Err("Response channel closed".to_string()),
        Err(_) => {
            let mut pending = state.pending.lock().await;
            pending.remove(request_id);
            Err("Timeout waiting for extension response".to_string())
        }
    }
}
