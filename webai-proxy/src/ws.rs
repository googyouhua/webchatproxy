use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

use crate::state::{AppState, ExtSocket, next_conn_id};

pub async fn start_ws_server(state: Arc<AppState>, port: u16) {
    let addr = format!("127.0.0.1:{}", port);

    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            eprintln!("WS server listening on {}", addr);
            crate::log::global_log(&format!("WS server listening on {}", addr));

            while let Ok((stream, peer)) = listener.accept().await {
                crate::log::global_log(&format!("Connection from {}", peer));
                let state = state.clone();
                    tokio::spawn(async move {
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                crate::log::global_log("WS connection established");
                                handle_extension(state, ws_stream).await;
                            }
                            Err(e) => {
                                crate::log::global_log(&format!("WS handshake failed: {}", e));
                            }
                        }
                });
            }
        }
        Err(e) => {
            eprintln!("Failed to bind WS: {}", e);
        }
    }
}

async fn handle_extension(
    state: Arc<AppState>,
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    let (write, mut read) = ws_stream.split();
    let conn_id = next_conn_id();

    {
        let mut ext = state.extension_socket.lock().await;
        *ext = Some((conn_id, ExtSocket::Plain(write)));
    }

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if parsed.get("type").and_then(|v| v.as_str()) == Some("log") {
                        let msg = parsed.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        crate::log::global_log(&format!("[ext-log] {}", msg));
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
    }
}

pub async fn send_request(
    state: &Arc<AppState>,
    request_id: &str,
    payload: &serde_json::Value,
) -> Result<tokio::sync::oneshot::Receiver<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn test_pending_request_routing() {
        use crate::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let state = Arc::new(AppState {
            pending: Arc::new(Mutex::new(HashMap::new())),
            extension_socket: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
        });

        let (tx, rx) = oneshot::channel();
        state.pending.lock().await.insert("req-1".into(), tx);

        let msg = r#"{"requestId":"req-1","success":true,"data":{"response":"hello"}}"#;
        let parsed: serde_json::Value = serde_json::from_str(msg).unwrap();
        let request_id = parsed.get("requestId").and_then(|v| v.as_str()).unwrap().to_string();
        let mut pending = state.pending.lock().await;
        let sender = pending.remove(&request_id);
        drop(pending);

        assert!(sender.is_some(), "should find pending sender");

        sender.unwrap().send(msg.to_string()).unwrap();
        let result = rx.await.unwrap();
        assert!(result.contains("hello"));
    }
}
