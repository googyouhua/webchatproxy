---
change: ai-bridge-native
design-doc: docs/superpowers/specs/2026-07-17-ai-bridge-native-design.md
base-ref: 943fdc9cec647ac1b95eb6ae64c6faebc4cd6cc0
---

# ai-bridge MCP Server (Rust) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite ai-bridge's MCP Server from Node.js to Rust as a single binary, keeping full compatibility with the Chrome extension.

**Architecture:** Single Cargo crate in `mcp-server/` with tokio async runtime. Uses `rmcp` (official MCP Rust SDK) for MCP stdio protocol and `tokio-tungstenite` for WebSocket communication with the Chrome extension. Token auth via `AI_BRIDGE_MCP_TOKEN` env var. Primary/secondary instance switching via port contention detection (same as existing JS behavior).

**Tech Stack:** Rust, tokio, tokio-tungstenite, rmcp, serde_json, clap

---

## File Structure

```
mcp-server/                          # NEW directory
├── Cargo.toml
└── src/
    ├── main.rs           # Entry: CLI parsing, service startup, graceful shutdown
    ├── ws_server.rs      # WebSocket server/client (primary/secondary mode), extension socket management
    ├── mcp_handler.rs    # RMCP ServerHandler: tool definitions, pending request routing
    ├── auth.rs           # Token generation/validation
    ├── log.rs            # Dual-write logger (file + stderr)
    └── types.rs          # Shared types: Config, PendingRequest, etc.
```

### Task 1: Cargo Crate Scaffold

**Files:**
- Create: `mcp-server/Cargo.toml`
- Create: `mcp-server/src/main.rs`
- Create: `mcp-server/src/lib.rs`
- Create: `mcp-server/src/log.rs`

- [ ] **1.1 Create Cargo.toml with all dependencies**

```toml
[package]
name = "ai-bridge-mcp"
version = "1.1.0"
edition = "2021"
description = "MCP Server for ai-bridge - connects terminal AI agents to browser-based chat AIs"

[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"
rmcp = { version = "0.16", features = ["server", "transport-io"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
```

- [ ] **1.2 Create main.rs with CLI arg parsing**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "ai-bridge-mcp", version = "1.1.0")]
struct Cli {
    #[arg(long, default_value = "9527")]
    ws_port: u16,
    #[arg(long, default_value = "/tmp/ai-bridge.log")]
    log_file: String,
    #[arg(long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // Determine token: arg > env > random
    let token = cli.token
        .or_else(|| std::env::var("AI_BRIDGE_MCP_TOKEN").ok())
        .unwrap_or_else(|| generate_random_token());
    // Start services
}
```

- [ ] **1.3 Create log.rs — dual-write logger**

```rust
use std::fs::OpenOptions;
use std::io::Write;

pub fn init_log(path: &str) {
    // Open or create log file in append mode
}

pub fn log(msg: &str) {
    // Write [ISO_TIMESTAMP] msg to stderr AND log file
}
```

- [ ] **1.4 Create lib.rs — re-export modules**

- [ ] **1.5 Verify: `cd mcp-server && cargo check` passes**

---

### Task 2: Auth Module

**Files:**
- Create: `mcp-server/src/auth.rs`

- [ ] **2.1 Implement token generation and validation**

```rust
pub struct Auth {
    token: String,
}

impl Auth {
    pub fn new(token: String) -> Self { Self { token } }

    pub fn generate_random() -> String {
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&rand::distributions::StandardHexadecimal)
            .take(32)
            .collect()
    }

    pub fn validate(&self, request_token: &str) -> bool {
        self.token == request_token
    }

    pub fn token(&self) -> &str { &self.token }
}
```

Dependency needed: `rand = "0.8"` in Cargo.toml.

- [ ] **2.2 Verify: `cargo check` passes**

---

### Task 3: Types Module

**Files:**
- Create: `mcp-server/src/types.rs`

- [ ] **3.1 Define shared types**

```rust
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use std::collections::HashMap;

pub type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub struct AppState {
    pub pending: PendingRequests,
    pub auth: Auth,
}
```

- [ ] **3.2 Verify: `cargo check` passes**

---

### Task 4: Log Module Finalize

**Files:**
- Modify: `mcp-server/src/log.rs`

- [ ] **4.1 Implement full log module with ISO timestamp**

```rust
use std::fs::OpenOptions;
use std::io::Write;

pub struct Logger {
    file: std::fs::File,
}

impl Logger {
    pub fn new(path: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("Failed to open log file");
        Logger { file }
    }

    pub fn log(&mut self, msg: &str) {
        let line = format!("[{}] {}", chrono::Utc::now().to_rfc3339(), msg);
        eprintln!("{}", line);
        let _ = writeln!(self.file, "{}", line);
    }
}
```

Dependency: add `chrono = { version = "0.4", features = ["serde"] }` to Cargo.toml.

- [ ] **4.2 Verify: `cargo check` passes**

---

### Task 5: WebSocket Server

**Files:**
- Create: `mcp-server/src/ws_server.rs`

- [ ] **5.1 Implement WebSocket server (primary mode)**

Bind to configured port, accept connections, validate token from URL query `?token=xxx`.

- [ ] **5.2 Implement Chrome extension connection handler**

Store `extension_socket`, parse incoming JSON messages, route by `requestId` to pending requests.

- [ ] **5.3 Implement secondary mode (port busy → connect as client)**

When `TcpListener::bind` fails with `EADDRINUSE`, connect as WebSocket client to `ws://127.0.0.1:{port}` with `?token=xxx&type=mcp`.

- [ ] **5.4 Handle pendingRequests: send WS → wait oneshot → return to MCP handler**

```rust
pub async fn start_ws_server(state: Arc<AppState>, port: u16, log_file: String) {
    // Primary mode: bind and accept
    // On connection: validate token, classify as extension or mcp client
    // Extension: parse requestId, route to pending requests
    // MCP client: forward messages
}
```

- [ ] **5.5 Verify: `cargo check` passes**

---

### Task 6: MCP Handler (using rmcp)

**Files:**
- Create: `mcp-server/src/mcp_handler.rs`

- [ ] **6.1 Implement BridgeHandler with rmcp ServerHandler + tools**

```rust
use rmcp::{ServiceExt, tool, ServerHandler, ServerInfo};
use std::sync::Arc;

pub struct BridgeHandler {
    state: Arc<AppState>,
    ws_port: u16,
    log_file: String,
}

#[tool(tool_box)]
impl ServerHandler for BridgeHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Bridge between MCP and browser-based AI chat services".into()),
            ..Default::default()
        }
    }

    #[tool(description = "Check Chrome extension connection status")]
    async fn check_connection(&self) -> String {
        // Check if extension socket is connected
        "connected" // or "disconnected"
    }

    #[tool(description = "Create a new AI chat session")]
    async fn new_session(
        &self,
        #[tool(description = "Initial message to send")] message: String,
        #[tool(description = "Platform name: doubao, chatgpt, deepseek")] platform: String,
    ) -> String {
        // Send WS message to extension, wait for response
    }

    #[tool(description = "Continue an existing AI chat session")]
    async fn ask_ai(
        &self,
        #[tool(description = "Follow-up message")] message: String,
        #[tool(description = "Session URL returned by new_session")] session_url: String,
        #[tool(description = "Platform name: doubao, chatgpt, deepseek")] platform: String,
    ) -> String {
        // Send WS message with session_url, wait for response
    }
}

pub async fn run_mcp_server(state: Arc<AppState>, ws_port: u16, log_file: String) {
    let handler = BridgeHandler { state, ws_port, log_file };
    handler.serve(rmcp::transport::TokioChildProcess::new())
        .await
        .unwrap();
}
```

- [ ] **6.2 Implement waitForConnection logic**: check extension socket status, retry up to 30s

- [ ] **6.3 Verify: `cargo check` passes**

---

### Task 7: Main Entry — Wire Everything Together

**Files:**
- Modify: `mcp-server/src/main.rs`

- [ ] **7.1 Wire all services in main.rs**

```rust
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let log_file = cli.log_file;
    let mut logger = log::Logger::new(&log_file);
    logger.log("ai-bridge-mcp starting...");

    let token = resolve_token(cli.token);
    let auth = auth::Auth::new(token);
    let state = Arc::new(AppState {
        pending: Arc::new(Mutex::new(HashMap::new())),
        auth,
    });

    logger.log(&format!("Token: {}", state.auth.token()));

    // Start WebSocket server (primary or secondary)
    let ws_handle = tokio::spawn(ws_server::start_ws_server(
        state.clone(),
        cli.ws_port,
        log_file.clone(),
    ));

    // Start MCP stdio server
    let mcp_handle = tokio::spawn(mcp_handler::run_mcp_server(
        state.clone(),
        cli.ws_port,
        log_file.clone(),
    ));

    tokio::select! {
        _ = ws_handle => {},
        _ = mcp_handle => {},
    }
}
```

- [ ] **7.2 Verify: `cargo build` succeeds**

---

### Task 8: CLI Install Subcommand

**Files:**
- Modify: `mcp-server/src/main.rs`

- [ ] **8.1 Add `install` subcommand** — reads/writes opencode.jsonc to register this MCP server

```rust
#[derive(Parser)]
#[command(name = "ai-bridge-mcp")]
enum Cli {
    #[command(name = "install")]
    Install {
        #[arg(long, default_value = "9527")]
        ws_port: u16,
        #[arg(long)]
        token: Option<String>,
    },
    #[command(name = "run")]
    Run {
        #[arg(long, default_value = "9527")]
        ws_port: u16,
        #[arg(long, default_value = "/tmp/ai-bridge.log")]
        log_file: String,
        #[arg(long)]
        token: Option<String>,
    },
}
```

- [ ] **8.2 Verify: `cargo build` succeeds**

---

### Task 9: Integration Test

**Files:**
- Create: `mcp-server/tests/integration_test.rs`

- [ ] **9.1 Write integration test: start server, connect WS client, verify token auth**

```rust
#[tokio::test]
async fn test_token_auth() {
    // Start server with known token
    // Connect with wrong token → connection rejected
    // Connect with correct token → accepted
}

#[tokio::test]
async fn test_primary_secondary() {
    // Start primary on port X
    // Start secondary → connects as client
    // Drop primary → secondary detects disconnect
}
```

- [ ] **9.2 Verify: `cargo test` passes**

---

### Task 10: Build & Final Verification

- [ ] **10.1 `cargo build --release` succeeds**
- [ ] **10.2 Manual test: run binary, connect Chrome extension, verify end-to-end**
