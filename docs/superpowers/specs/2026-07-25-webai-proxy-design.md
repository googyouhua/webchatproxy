---
comet_change: create-webai-proxy
role: technical-design
canonical_spec: openspec
archived-with: 2026-07-26-create-webai-proxy
status: final
---

# webai-proxy Technical Design

## Overview

webai-proxy bridges OpenAI-compatible HTTP API clients to browser-based AI chat services (DeepSeek) via a Chrome extension. It is a standalone Rust binary — no dependency on ai-bridge-mcp.

## Architecture

```
HTTP client (OpenCode / curl / SDK)
  │ POST /v1/chat/completions (OpenAI format, JSON body)
  ▼
┌─────────────────────────────────────────────┐
│  webai-proxy (single Rust binary)           │
│                                              │
│  ┌─────────┐   ┌──────────────────────┐     │
│  │ axum    │   │  AppState            │     │
│  │ HTTP    │──▶│  ┌────────────────┐  │     │
│  │ Server  │   │  │ session: Url   │  │     │
│  │         │   │  │ pending: Map   │  │     │
│  │ /v1/chat│   │  │ ext_socket: WS │  │     │
│  │ /v1/models│  └──────────────────┘  │     │
│  │ /health │                          │     │
│  └─────────┘                          │     │
│              ┌──────────────────────┐ │     │
│              │ tokio-tungstenite    │ │     │
│              │ WS Server (9530)     │ │     │
│              └──────────────────────┘ │     │
└──────────────────────┬──────────────────┘     │
                       │ WebSocket (token auth)
                       ▼
┌─────────────────────────────────────────────┐
│  Chrome Extension (MV3)                     │
│                                              │
│  background.js ─── WS ─── webai-proxy       │
│       │                                      │
│       ▼                                      │
│  content.js ─── DOM ─── DeepSeek page       │
│       │                                      │
│       ▼ (fallback)                           │
│  CDP (chrome.debugger)                       │
│       │                                      │
│       ▼                                      │
│  DeepSeek (chat.deepseek.com)               │
└─────────────────────────────────────────────┘
```

## Components

### Rust Binary (`webai-proxy`)

**HTTP Server (axum)**

| Route | Method | Description |
|-------|--------|-------------|
| `/v1/chat/completions` | POST | OpenAI-compatible chat completions |
| `/v1/models` | GET | List available models |
| `/health` | GET | Health check |

Request flow:

1. Middleware checks `Authorization: Bearer <token>` (401 if invalid/missing)
2. Parse body into `ChatCompletionRequest` (OpenAI format)
3. Extract last `role=user` message; prepend `role=system` if present
4. If no active session → send `new_session` action via WS
5. If active session → send `send_message` action via WS
6. Wait for extension response via oneshot channel
7. If streaming (stream=true): chunk response into SSE `data:` events
8. If non-streaming: return JSON `ChatCompletionResponse`
9. If session expired / extension error: return OpenAI-formatted error (non-retryable)

**WebSocket Server (tokio-tungstenite)**

- Listens on configurable port (default 9530)
- Accepts Chrome extension connection (single connection)
- Token authentication via `?token=` query parameter
- Messages use requestId-based routing (oneshot channel per request)
- Extension sends structured log messages prefixed with type

**Message protocol (WS):**

```json
// Server → Extension
{"requestId":"uuid","action":"new_session","payload":{"message":"...","platform":"deepseek"}}

// Extension → Server (response)
{"requestId":"uuid","success":true,"data":{"response":"...","sessionUrl":"https://chat.deepseek.com/a/chat/s/xxx"}}

// Server → Extension (send_message)
{"requestId":"uuid","action":"send_message","payload":{"message":"...","platform":"deepseek","sessionUrl":"..."}}

// Extension → Server (log)
{"type":"log","message":"..."}
```

**AppState:**

```rust
struct AppState {
    extension_socket: Mutex<Option<(u64, ExtSocket)>>,  // WS connection
    pending: Mutex<HashMap<String, oneshot::Sender<String>>>,  // requestId → response channel
    active_session: Mutex<Option<String>>,  // current DeepSeek session URL
}
```

### Chrome Extension (MV3)

Same dual-channel approach as ai-bridge:

- **background.js**: WS connection management (connect/reconnect), message routing to content script or CDP
- **content.js**: DOM operations on DeepSeek page (input, send, poll for response)
- **CDP fallback**: When SPA ignores DOM input in background tabs (known DeepSeek issue), uses `chrome.debugger` to attach and inject via `Runtime.evaluate`

**Configuration (via chrome.storage.local):**

- `wsUrl`: WebSocket server URL (default `ws://localhost:9530`)
- `token`: Authentication token

## Data Flow

### Non-Streaming Request

```
Client → POST /v1/chat/completions
  → server parses messages, extracts last user message
  → if no session: send WS new_session → extension → DeepSeek input
  → if has session: send WS send_message → extension → DeepSeek input
  → extension polls DOM → detects completion → returns via WS
  → server formats OpenAI JSON response → sends to client
```

### Streaming Request

```
Client → POST /v1/chat/completions (stream: true)
  → server parses messages
  → sends WS action → extension → DeepSeek
  → extension detects completion → returns full text via WS
  → server chunks response:
    data: {"choices":[{"delta":{"role":"assistant"},"index":0}]}
    data: {"choices":[{"delta":{"content":"你好"},"index":0}]}
    data: {"choices":[{"delta":{"content":"世界"},"index":0}]}
    data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}]}
    data: [DONE]
```

### Session Expiry

```
Extension detects session invalid (page closed, stale URL)
  → returns error in WS response
Server receives error
  → clears active session
  → returns OpenAI error response to client
Client retries with full message history
  → server sees no session → creates new_session
```

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| HTTP framework | axum | Tokio-native, SSE ergonomics, middleware support |
| WS framework | tokio-tungstenite | Same stack as ai-bridge, proven |
| Streaming | Simulated (chunk after full response) | Extension returns complete text; server splits into SSE |
| Session | Single, reused | Simple, matches single-user CLI use case |
| Multi-turn | Only last user message sent | DeepSeek page holds full context |
| Extension | CDP + Content Script | Reuses ai-bridge's proven dual-channel approach |
| Auth | Bearer token (WEBAI_PROXY_TOKEN) | Standard, same pattern as ai-bridge |

## Project Structure

```
webai-proxy/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI entry (clap: serve subcommand)
│   ├── server.rs         # axum HTTP setup + routes + CORS
│   ├── ws.rs             # WS server + extension connection + request routing
│   ├── openai.rs         # OpenAI request/response types + serialization
│   ├── bridge.rs         # HTTP request → WS message translation + message mapping
│   ├── auth.rs           # Token auth (middleware + WS validation)
│   ├── log.rs            # File + stderr logging
│   └── state.rs          # AppState + types
├── chrome-extension/
│   ├── manifest.json     # MV3
│   ├── background.js     # WS connect/reconnect + message routing + CDP
│   ├── content.js        # DOM operations (DeepSeek)
│   └── default-configs.js
```

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Streaming simulated (not real) | User sees delayed response | Simulated chunking provides progressive feedback |
| Single session concurrency | Concurrent requests break | Queue requests per session |
| Extension disconnects | Requests hang | Timeout + clear error to client |
| DeepSeek UI changes | DOM selectors break | Platform abstraction isolates selector logic |
| Two extensions to maintain | User confusion | Clear docs naming (ai-bridge ↔ webai-proxy) |
