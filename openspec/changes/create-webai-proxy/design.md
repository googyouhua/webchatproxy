## Context

webai-proxy is a new Rust HTTP server that exposes browser-based AI chats (DeepSeek initially) through an OpenAI-compatible `/v1/chat/completions` API. It operates as a standalone project with its own Chrome extension, completely separate from the existing ai-bridge-mcp infrastructure.

## Goals / Non-Goals

**Goals:**
- OpenAI-compatible HTTP API (`/v1/chat/completions`) with JSON and SSE responses
- New Chrome extension connecting to webai-proxy via WebSocket
- Token-based authentication for both HTTP and WS
- Platform abstraction for AI chat providers (DeepSeek initial)
- Streaming (SSE) support following OpenAI response format

**Non-Goals:**
- Direct API calls to AI providers (always through browser extension)
- Integration with existing ai-bridge-mcp or its Chrome extension
- Full OpenAI API compatibility beyond chat completions

## Decisions

### 1. HTTP Framework: axum
axum is built on tokio/hyper, same async runtime stack as the rest of the ai-bridge ecosystem, with excellent ergonomics for routing, middleware, and SSE.

### 2. Architecture: HTTP Server + Embedded WS Server
The Rust binary runs an axum HTTP server and a tokio-tungstenite WebSocket server in the same process. The Chrome extension connects to the WS server. HTTP requests are translated to WS messages and forwarded through the extension.

```
HTTP client → axum HTTP (OpenAI API) → Shared AppState → WS Server → Chrome Ext → Browser AI
```

### 3. OpenAI Message Mapping
- First user message in a request → maps to `new_session` action
- Subsequent messages with existing session → maps to `send_message` action
- System message → prepended as instruction to the first user message
- Response → extracted from extension reply, formatted as OpenAI chat completion

### 4. SSE Streaming
Streaming follows OpenAI's SSE format (`data: {"choices":[{"delta":{...}}]}`). The extension returns the full text when generation is complete; the server simulates streaming by chunking the response, or ideally the extension streams tokens back in real-time.

### 5. Authentication
Bearer token in HTTP `Authorization` header. Same token model as ai-bridge (`AI_BRIDGE_MCP_TOKEN` → `WEBAI_PROXY_TOKEN`). WS connections also validated with the token.

### 6. Project Structure
```
webai-proxy/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point (clap CLI: serve subcommand)
│   ├── server.rs         # axum HTTP server setup
│   ├── ws.rs             # WebSocket server for Chrome extension
│   ├── openai.rs         # OpenAI API request/response types + handler
│   ├── bridge.rs         # HTTP request → WS message translation
│   ├── platform.rs       # Platform abstraction trait
│   ├── platform/
│   │   └── deepseek.rs   # DeepSeek-specific selectors/logic
│   ├── auth.rs           # Token authentication
│   └── log.rs            # Logging
├── chrome-extension/
│   ├── manifest.json
│   ├── background.js     # WS connection + message routing
│   ├── content.js        # DOM operations on AI pages
│   └── default-configs.js
```

## Risks / Trade-offs

- **Extension management**: User must install and maintain a second Chrome extension alongside ai-bridge's. Mitigation: clear documentation.
- **Streaming fidelity**: Real per-token streaming requires the extension to forward tokens from the AI page in real-time, which is more complex. Simplification: buffer full response and simulate SSE chunks.
- **Port conflicts**: Default WS port may conflict. Mitigation: configurable ports.
