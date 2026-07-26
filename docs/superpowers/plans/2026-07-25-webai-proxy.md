---
change: create-webai-proxy
design-doc: docs/superpowers/specs/2026-07-25-webai-proxy-design.md
base-ref: none
archived-with: 2026-07-26-create-webai-proxy
---

# webai-proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Build a standalone Rust HTTP server + Chrome extension that exposes browser-based DeepSeek through OpenAI-compatible API.

**Architecture:** Single Rust binary with axum HTTP server + embedded tokio-tungstenite WebSocket server. Chrome extension connects via WS, operates DeepSeek via CDP + content script. Requests flow: HTTP → WS → extension → DeepSeek → response back.

**Tech Stack:** Rust (axum, tokio-tungstenite, clap), Chrome Extension MV3

archived-with: 2026-07-26-create-webai-proxy
---

## File Map

```
webai-proxy/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI entry (clap: serve subcommand)
│   ├── server.rs         # axum HTTP setup + routes + middleware
│   ├── ws.rs             # WS server + extension connection + request routing
│   ├── openai.rs         # OpenAI request/response types
│   ├── bridge.rs         # HTTP request → WS message translation
│   ├── auth.rs           # Bearer token auth middleware + WS validation
│   ├── log.rs            # File + stderr logging
│   └── state.rs          # AppState, ExtSocket, shared types
├── chrome-extension/
│   ├── manifest.json     # MV3
│   ├── background.js     # WS connect/reconnect + message routing + CDP
│   ├── content.js        # DOM operations on DeepSeek
│   └── default-configs.js
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 1: Project Scaffold

**Files:**
- Create: `webai-proxy/Cargo.toml`
- Create: `webai-proxy/src/state.rs`
- Create: `webai-proxy/src/log.rs`

- [x] **Step 1: Create Cargo.toml**

```toml
[package]
name = "webai-proxy"
version = "0.1.0"
edition = "2021"
description = "OpenAI-compatible HTTP API for browser-based AI chats"

[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
axum = { version = "0.7", features = ["macros"] }
tower-http = { version = "0.5", features = ["cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
futures-util = "0.3"
```

- [x] **Step 2: Create src/log.rs**

```rust
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

pub struct Logger {
    path: String,
}

impl Logger {
    pub fn new(path: &str) -> Self {
        Self { path: path.to_string() }
    }

    pub fn log(&mut self, message: &str) {
        let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S%.9fZ");
        let line = format!("[{}] {}\n", ts, message);
        eprint!("{}", line);
        if let Ok(mut f) = OpenOptions::new()
            .create(true).append(true).open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
```

- [x] **Step 3: Create src/state.rs**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitSink;
use tokio_tungstenite::WebSocketStream;

pub type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub enum ExtSocket {
    Plain(SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>),
}

pub type ExtensionSocket = Arc<Mutex<Option<(u64, ExtSocket)>>>;

pub fn next_conn_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct AppState {
    pub pending: PendingRequests,
    pub extension_socket: ExtensionSocket,
    pub active_session: Arc<Mutex<Option<String>>>,
    pub auth_token: String,
}
```

- [x] **Step 4: Commit**

```bash
git init webai-proxy && cd webai-proxy
git add Cargo.toml src/state.rs src/log.rs
git commit -m "chore: scaffold webai-proxy Rust project"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 2: Auth Module

**Files:**
- Create: `webai-proxy/src/auth.rs`

- [x] **Step 1: Create src/auth.rs**

```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

pub fn extract_bearer_token(req: &Request) -> Option<String> {
    let header = req.headers().get("Authorization")?;
    let value = header.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_string())
}

pub async fn auth_middleware(
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let token = extract_bearer_token(&req).ok_or((
        StatusCode::UNAUTHORIZED,
        serde_json::json!({"error": "missing authorization header"}).to_string(),
    ))?;

    let expected = req.extensions().get::<String>()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "no token configured".into()))?;

    if token != *expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid token"}).to_string(),
        ));
    }

    Ok(next.run(req).await)
}
```

- [x] **Step 2: Commit**

```bash
git add src/auth.rs
git commit -m "feat: add Bearer token auth middleware"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 3: WebSocket Server

**Files:**
- Create: `webai-proxy/src/ws.rs`

- [x] **Step 1: Create src/ws.rs**

```rust
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

use crate::state::{AppState, ExtSocket, next_conn_id};
use crate::log::Logger;

pub async fn start_ws_server(state: Arc<AppState>, port: u16) {
    let addr = format!("127.0.0.1:{}", port);
    let mut logger = Logger::new("/tmp/webai-proxy.log");
    logger.log(&format!("WS server listening on {}", addr));

    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            while let Ok((stream, peer)) = listener.accept().await {
                logger.log(&format!("Connection from {}", peer));
                let state = state.clone();
                tokio::spawn(async move {
                    match accept_async(stream).await {
                        Ok(ws_stream) => {
                            let mut logger = Logger::new("/tmp/webai-proxy.log");
                            logger.log("WS connection established");
                            handle_extension(state, ws_stream).await;
                        }
                        Err(e) => {
                            let mut logger = Logger::new("/tmp/webai-proxy.log");
                            logger.log(&format!("WS handshake failed: {}", e));
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
                        let mut logger = Logger::new("/tmp/webai-proxy.log");
                        logger.log(&format!("[ext-log] {}", msg));
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
                    return Err(e.to_string());
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

async fn send_via_ext_socket(
    ext: &mut ExtSocket,
    payload: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match ext {
        ExtSocket::Plain(ref mut write) => {
            write.send(Message::Text(payload.into())).await?;
        }
    }
    Ok(())
}
```

- [x] **Step 2: Commit**

```bash
git add src/ws.rs
git commit -m "feat: add WebSocket server for extension connection"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 4: OpenAI Types

**Files:**
- Create: `webai-proxy/src/openai.rs`

- [x] **Step 1: Create src/openai.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct DeltaChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<DeltaChoice>,
}

#[derive(Debug, Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct ListModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

impl ChatCompletionResponse {
    pub fn new(model: &str, content: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".into(),
            created: now,
            model: model.into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content,
                },
                finish_reason: "stop".into(),
            }],
            usage: Usage::default(),
        }
    }
}
```

- [x] **Step 2: Commit**

```bash
git add src/openai.rs
git commit -m "feat: add OpenAI request/response types"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 5: Bridge — Message Translation

**Files:**
- Create: `webai-proxy/src/bridge.rs`

- [x] **Step 1: Create src/bridge.rs**

```rust
use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::openai::ChatCompletionRequest;
use crate::state::AppState;
use crate::ws;

pub async fn send_to_extension(
    state: &Arc<AppState>,
    request: &ChatCompletionRequest,
) -> Result<String, String> {
    let messages = &request.messages;
    let last_user = messages.iter().rev().find(|m| m.role == "user")
        .ok_or("No user message found")?;

    let system_prefix: String = messages.iter()
        .find(|m| m.role == "system")
        .map(|m| format!("{}\n\n", m.content))
        .unwrap_or_default();

    let content = format!("{}{}", system_prefix, last_user.content);

    let session_url = state.active_session.lock().await.clone();

    let (action, payload) = match session_url {
        Some(url) => (
            "send_message",
            serde_json::json!({
                "message": content,
                "platform": "deepseek",
                "sessionUrl": url,
            }),
        ),
        None => (
            "new_session",
            serde_json::json!({
                "message": content,
                "platform": "deepseek",
            }),
        ),
    };

    let request_id = Uuid::new_v4().to_string();
    let ws_msg = serde_json::json!({
        "requestId": request_id,
        "action": action,
        "payload": payload,
    });

    let rx = ws::send_request(state, &request_id, &ws_msg).await?;

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(180),
        rx,
    ).await
        .map_err(|_| "Timeout waiting for extension response".to_string())?
        .map_err(|_| "Response channel closed".to_string())?;

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
        if !parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            let error = parsed.get("error").and_then(|v| v.as_str()).unwrap_or("Extension error");
            if let Some(session) = state.active_session.lock().await.as_ref() {
                if error.contains("session") || error.contains("invalid") || error.contains("stale") {
                    state.active_session.lock().await.take();
                }
            }
            return Err(error.to_string());
        }

        if action == "new_session" {
            if let Some(url) = parsed.pointer("/data/sessionUrl").and_then(|v| v.as_str()) {
                if url.starts_with("https://chat.deepseek.com/a/chat/s/") {
                    *state.active_session.lock().await = Some(url.to_string());
                }
            }
        }

        if let Some(text) = parsed.pointer("/data/response").and_then(|v| v.as_str()) {
            return Ok(text.to_string());
        }
    }

    Err("Empty or invalid extension response".to_string())
}
```

- [x] **Step 2: Commit**

```bash
git add src/bridge.rs
git commit -m "feat: add HTTP→WS message bridge"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 6: HTTP Server (axum)

**Files:**
- Create: `webai-proxy/src/server.rs`

- [x] **Step 1: Create src/server.rs**

```rust
use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
    middleware,
    response::sse::KeepAlive,
};
use futures_util::stream::{self, Stream};
use std::convert::Infallible;
use tower_http::cors::CorsLayer;

use crate::bridge;
use crate::openai::*;
use crate::state::AppState;

#[derive(Clone)]
pub struct AppStateExt {
    pub state: Arc<AppState>,
    pub server_token: String,
}

pub fn create_router(app_state: AppStateExt) -> Router {
    let token = app_state.server_token.clone();

    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .route("/health", get(health))
        .layer(middleware::from_fn(move |req, next| {
            let token = token.clone();
            async move {
                let path = req.uri().path();
                if path == "/health" {
                    return Ok(next.run(req).await);
                }
                match crate::auth::extract_bearer_token(&req) {
                    Some(t) if t == token => {}
                    _ => {
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            serde_json::json!({"error": "invalid or missing token"}).to_string(),
                        ));
                    }
                }
                Ok(next.run(req).await)
            }
        }))
        .with_state(app_state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn list_models(
    State(ext): State<AppStateExt>,
) -> impl IntoResponse {
    let session_active = ext.state.active_session.lock().await.is_some();
    Json(ListModelsResponse {
        object: "list".into(),
        data: vec![ModelInfo {
            id: "deepseek".into(),
            object: "model".into(),
            created: 1710000000,
            owned_by: "webai-proxy".into(),
        }],
    })
}

async fn chat_completions(
    State(ext): State<AppStateExt>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<serde_json::Value>)> {
    if req.messages.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "messages is empty"}))));
    }

    if !req.model.starts_with("deepseek") {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "unsupported model"}))));
    }

    if ext.state.extension_socket.lock().await.is_none() {
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "extension not connected"}))));
    }

    let response_text = bridge::send_to_extension(&ext.state, &req).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e}))))?;

    if req.stream {
        let chunks = chunk_text(&response_text, 3);
        let stream = stream::iter(chunks.into_iter().map(move |chunk| {
            let delta = if response_text.starts_with(&chunk) {
                Delta { role: Some("assistant".into()), content: Some(chunk) }
            } else {
                Delta { role: None, content: Some(chunk) }
            };
            let chunk_data = ChatCompletionChunk {
                id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".into(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                model: req.model.clone(),
                choices: vec![DeltaChoice {
                    index: 0,
                    delta,
                    finish_reason: None,
                }],
            };
            Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&chunk_data).unwrap()))
        }));

        let done_event = Ok::<_, Infallible>(Event::default().data("[DONE]"));
        let full_stream = stream.chain(futures_util::stream::once(async { done_event.unwrap() }));

        Ok(Sse::new(full_stream)
            .keep_alive(KeepAlive::interval(std::time::Duration::from_secs(15)))
            .into_response())
    } else {
        let resp = ChatCompletionResponse::new(&req.model, response_text);
        Ok(Json(resp).into_response())
    }
}

fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars.chunks(chunk_size)
        .map(|c| c.iter().collect())
        .collect()
}
```

- [x] **Step 2: Commit**

```bash
git add src/server.rs
git commit -m "feat: add axum HTTP server with chat completions endpoint"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 7: CLI Entry Point

**Files:**
- Create: `webai-proxy/src/main.rs`

- [x] **Step 1: Create src/main.rs**

```rust
mod auth;
mod bridge;
mod log;
mod openai;
mod server;
mod state;
mod ws;

use clap::Parser;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "webai-proxy", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the webai-proxy server
    Serve {
        /// HTTP server port
        #[arg(long, default_value = "4319", env = "WEBAI_PROXY_HTTP_PORT")]
        http_port: u16,
        /// WebSocket server port
        #[arg(long, default_value = "9530", env = "WEBAI_PROXY_WS_PORT")]
        ws_port: u16,
        /// Authentication token
        #[arg(long, env = "WEBAI_PROXY_TOKEN")]
        token: String,
        /// Log file path
        #[arg(long, default_value = "/tmp/webai-proxy.log")]
        log_file: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { http_port, ws_port, token, log_file } => {
            let state = Arc::new(state::AppState {
                pending: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                extension_socket: Arc::new(tokio::sync::Mutex::new(None)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
                auth_token: token.clone(),
            });

            let mut logger = log::Logger::new(&log_file);
            logger.log(&format!("Starting webai-proxy (HTTP={}, WS={})", http_port, ws_port));

            let ws_state = state.clone();
            tokio::spawn(async move {
                ws::start_ws_server(ws_state, ws_port).await;
            });

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            let app_state = server::AppStateExt {
                state: state.clone(),
                server_token: token,
            };
            let router = server::create_router(app_state);
            let addr = format!("0.0.0.0:{}", http_port);
            logger.log(&format!("HTTP server listening on {}", addr));

            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            axum::serve(listener, router).await.unwrap();
        }
    }
}
```

- [x] **Step 2: Build and verify compilation**

```bash
cd webai-proxy
cargo build 2>&1
```

Expected: `Compiling webai-proxy ... Finished dev [unoptimized + debuginfo]`

- [x] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add CLI entry point with serve subcommand"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 8: Chrome Extension — Manifest + Background

**Files:**
- Create: `webai-proxy/chrome-extension/manifest.json`
- Create: `webai-proxy/chrome-extension/background.js`

- [x] **Step 1: Create manifest.json**

```json
{
  "manifest_version": 3,
  "name": "webai-proxy",
  "version": "0.1.0",
  "description": "Bridge between webai-proxy HTTP server and browser AI chats",
  "permissions": [
    "storage",
    "alarms",
    "tabs",
    "debugger"
  ],
  "host_permissions": [
    "https://chat.deepseek.com/*"
  ],
  "background": {
    "service_worker": "background.js"
  },
  "content_scripts": [
    {
      "matches": ["https://chat.deepseek.com/*"],
      "js": ["default-configs.js", "content.js"],
      "run_at": "document_end"
    }
  ]
}
```

- [x] **Step 2: Create background.js**

```javascript
let ws = null;
let reconnectTimer = null;
const RECONNECT_INTERVAL = 5000;
let connectAttempts = 0;
let isManualDisconnect = false;

async function getSettings() {
  const defaults = {
    wsUrl: "ws://localhost:9530",
    token: "",
    enabled: true,
  };
  return await chrome.storage.local.get(defaults);
}

async function connect() {
  if (ws && ws.readyState === WebSocket.OPEN) return;
  const settings = await getSettings();
  if (!settings.enabled) return;

  connectAttempts++;
  const url = `${settings.wsUrl}?token=${encodeURIComponent(settings.token)}`;

  try {
    ws = new WebSocket(url);
  } catch (e) {
    scheduleReconnect();
    return;
  }

  ws.onopen = () => {
    connectAttempts = 0;
    chrome.storage.local.set({ wsConnected: true });
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      const { requestId, action, payload } = msg;

      chrome.storage.local.get("config_chat.deepseek.com", (result) => {
        const config = result["config_chat.deepseek.com"] || {};
        chrome.tabs.query({ url: "https://chat.deepseek.com/*" }, (tabs) => {
          if (tabs.length === 0) {
            chrome.tabs.create({ url: "https://chat.deepseek.com" }, (tab) => {
              waitForTab(tab.id, () => handleAction(tab.id, requestId, action, payload, config));
            });
          } else {
            const tab = tabs[0];
            chrome.tabs.update(tab.id, { active: true }, () => {
              handleAction(tab.id, requestId, action, payload, config);
            });
          }
        });
      });
    } catch (e) {
      wsLog("parse error: " + e.message);
    }
  };

  ws.onclose = () => {
    chrome.storage.local.set({ wsConnected: false });
    ws = null;
    scheduleReconnect();
  };

  ws.onerror = () => {};
}

function scheduleReconnect() {
  if (reconnectTimer) clearTimeout(reconnectTimer);
  reconnectTimer = setTimeout(connect, RECONNECT_INTERVAL);
}

function wsLog(message) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: "log", message }));
  }
}

function waitForTab(tabId, callback, tries = 0) {
  chrome.tabs.get(tabId, (tab) => {
    if (tab.status === "complete") {
      setTimeout(callback, 3000);
    } else if (tries < 30) {
      setTimeout(() => waitForTab(tabId, callback, tries + 1), 1000);
    }
  });
}

function handleAction(tabId, requestId, action, payload, config) {
  const actionMessage = { requestId, action, payload, config };

  chrome.tabs.sendMessage(tabId, actionMessage, (response) => {
    if (chrome.runtime.lastError) {
      tryCDP(tabId, requestId, action, payload, config);
    } else if (response && response.success) {
      ws.send(JSON.stringify({
        requestId,
        success: true,
        data: response.data,
      }));
    } else {
      ws.send(JSON.stringify({
        requestId,
        success: false,
        error: response ? response.error : "content script error",
      }));
    }
  });
}

async function ensureDebugger(tabId) {
  return new Promise((resolve) => {
    chrome.debugger.getTargets((targets) => {
      const attached = targets.some(t => t.tabId === tabId && t.attached);
      if (attached) {
        resolve();
      } else {
        chrome.debugger.attach({ tabId }, "1.3", () => {
          chrome.debugger.sendCommand({ tabId }, "Runtime.enable", {}, resolve);
        });
      }
    });
  });
}

async function tryCDP(tabId, requestId, action, payload, config) {
  try {
    await ensureDebugger(tabId);

    const message = payload.message || payload.text || "";

    await chrome.debugger.sendCommand({ tabId }, "Runtime.evaluate", {
      expression: `
        (() => {
          const input = document.querySelector('${config.inputSelector || "#chat-input"}');
          if (!input) return "no-input";
          const nativeSetter = Object.getOwnPropertyDescriptor(
            window.HTMLTextAreaElement.prototype, "value"
          ).set;
          nativeSetter.call(input, ${JSON.stringify(message)});
          input.dispatchEvent(new Event("input", { bubbles: true }));
          const btn = document.querySelector('${config.sendButtonSelector || "[data-testid=send-button]"}');
          if (btn) { btn.click(); return "cdp-sent"; }
          return "cdp-typed-no-btn";
        })()
      `,
    });

    let fullText = "";
    const maxPolls = 120;
    for (let i = 0; i < maxPolls; i++) {
      await new Promise(r => setTimeout(r, 1000));
      const result = await chrome.debugger.sendCommand({ tabId }, "Runtime.evaluate", {
        expression: `
          (() => {
            const els = document.querySelectorAll('${config.responseSelector || ".ds-message"}');
            if (!els.length) return JSON.stringify({ready: false, debug: "no-els"});
            const last = els[els.length - 1];
            const streaming = last.getAttribute('${config.streamingAttr || "data-streaming"}');
            if (streaming === 'true' || streaming === '') return JSON.stringify({ready: false, debug: "streaming"});
            const text = last.textContent.trim();
            return JSON.stringify({ready: true, text: text});
          })()
        `,
      });

      const data = JSON.parse(result.result.value);
      if (data.ready && data.text) {
        fullText = data.text;
        break;
      }
    }

    if (fullText) {
      ws.send(JSON.stringify({
        requestId,
        success: true,
        data: { response: cleanText(fullText), sessionUrl: tabId },
      }));
    } else {
      ws.send(JSON.stringify({
        requestId,
        success: false,
        error: "timeout waiting for response",
      }));
    }
  } catch (e) {
    ws.send(JSON.stringify({ requestId, success: false, error: e.message }));
  }
}

function cleanText(text) {
  return text;
}

connect();
```

- [x] **Step 3: Commit**

```bash
git add chrome-extension/manifest.json chrome-extension/background.js
git commit -m "feat: add Chrome extension with WS + CDP support"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 9: Chrome Extension — Content Script + DeepSeek Support

**Files:**
- Create: `webai-proxy/chrome-extension/content.js`
- Create: `webai-proxy/chrome-extension/default-configs.js`

- [x] **Step 1: Create default-configs.js**

```javascript
const WEBAI_PROXY_DEFAULT_CONFIGS = {
  "chat.deepseek.com": {
    inputSelector: "#chat-input",
    sendButtonSelector: ".ds-button.ds-button--primary",
    responseSelector: ".ds-message",
    streamingAttr: "data-streaming",
    newChatSelector: "span:has(> span:contains('开启新对话'))",
  },
};
```

- [x] **Step 2: Create content.js**

```javascript
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message.action) return;

  const { requestId, action, payload, config } = message;

  if (action === "new_session") {
    handleNewSession(requestId, payload, config, sendResponse);
  } else if (action === "send_message") {
    handleSendMessage(requestId, payload, config, sendResponse);
  }

  return true;
});

async function handleNewSession(requestId, payload, config, sendResponse) {
  try {
    const clicked = tryNewChat();
    if (!clicked) {
      sendResponse({ success: false, error: "no els for new session" });
      return;
    }
    await sleep(1500);

    const result = await typeAndSend(payload.message, config);
    if (!result.success) {
      sendResponse({ success: false, error: result.error });
      return;
    }

    const response = await waitForResponse(config);
    if (!response) {
      sendResponse({ success: false, error: "no response" });
      return;
    }

    const sessionUrl = window.location.href;
    sendResponse({
      success: true,
      data: { response, sessionUrl },
    });
  } catch (e) {
    sendResponse({ success: false, error: e.message });
  }
}

async function handleSendMessage(requestId, payload, config, sendResponse) {
  try {
    const result = await typeAndSend(payload.message, config);
    if (!result.success) {
      sendResponse({ success: false, error: result.error });
      return;
    }
    const response = await waitForResponse(config);
    sendResponse({
      success: true,
      data: { response: response || "" },
    });
  } catch (e) {
    sendResponse({ success: false, error: e.message });
  }
}

function tryNewChat() {
  const spans = document.querySelectorAll("span");
  for (const span of spans) {
    if (span.textContent.includes("开启新对话")) {
      span.click();
      return true;
    }
  }
  const link = document.querySelector('a[href="/"]');
  if (link) {
    link.click();
    return true;
  }
  return false;
}

async function typeAndSend(text, config) {
  const input = document.querySelector(config.inputSelector || "#chat-input");
  if (!input) return { success: false, error: "no input" };

  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));

  await sleep(500);

  const btn = document.querySelector(config.sendButtonSelector || ".ds-button.ds-button--primary");
  if (btn) {
    btn.click();
    return { success: true };
  }

  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, bubbles: true }));

  await sleep(1000);
  return { success: true };
}

async function waitForResponse(config) {
  const maxPolls = 120;
  let lastText = "";

  for (let i = 0; i < maxPolls; i++) {
    await sleep(1000);
    const els = document.querySelectorAll(config.responseSelector || ".ds-message");
    if (!els.length) continue;

    const last = els[els.length - 1];
    const streaming = last.getAttribute(config.streamingAttr || "data-streaming");
    if (streaming === "true" || streaming === "") continue;

    const text = last.textContent.trim();
    if (text && text === lastText) {
      return cleanText(text);
    }
    lastText = text;
  }

  return null;
}

function cleanText(text) {
  return text
    .replace(/图表|代码|下载|全屏|复制/g, "")
    .replace(/\s*-\d+-\s*/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}
```

- [x] **Step 3: Commit**

```bash
git add chrome-extension/content.js chrome-extension/default-configs.js
git commit -m "feat: add content script for DeepSeek DOM operations"
```

archived-with: 2026-07-26-create-webai-proxy
---

### Task 10: Integration Test

- [x] **Step 1: Build Rust binary**

```bash
cd webai-proxy && cargo build --release 2>&1
```

Expected: `Compiling webai-proxy ... Finished release [optimized] target(s)`

- [x] **Step 2: Quick health check**

```bash
cd webai-proxy
WEBAI_PROXY_TOKEN=test cargo run -- serve --token test &
sleep 2
curl -s http://localhost:4319/health
```

Expected: `{"status":"ok"}`

- [x] **Step 3: Auth test**

```bash
curl -s http://localhost:4319/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"deepseek","messages":[{"role":"user","content":"hi"}]}'
```

Expected: `401` (no token)

- [x] **Step 4: Models endpoint**

```bash
curl -s http://localhost:4319/v1/models \
  -H 'Authorization: Bearer test'
```

Expected: model list JSON

- [x] **Step 5: Stop the server**

```bash
kill %1 2>/dev/null; wait 2>/dev/null
```
