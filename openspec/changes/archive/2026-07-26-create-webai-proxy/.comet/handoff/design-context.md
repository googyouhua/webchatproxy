# Comet Design Handoff

- Change: create-webai-proxy
- Phase: design
- Mode: compact
- Context hash: 0b541b26eff0e9ff1f6078786785c6147466e20ce3195104985dfa2ac7d2a1bb

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/create-webai-proxy/proposal.md

- Source: openspec/changes/create-webai-proxy/proposal.md
- Lines: 1-32
- SHA256: 7fee8e4372cc4328d471bef98db7ab9a4ed53ef09ac37e83327c273b44ebd3d7

```md
## Why

Terminal AI agents (OpenCode, etc.) and CLI tools need a standard HTTP API to interact with browser-based AI chat services (DeepSeek, etc.). Currently, ai-bridge provides this through MCP stdio protocol, which limits compatibility. An OpenAI-compatible HTTP API would allow any tool or SDK that speaks OpenAI's protocol to use browser-based AI chats without direct API keys.

## What Changes

- New Rust project `webai-proxy` — standalone HTTP server exposing `/v1/chat/completions`
- New Chrome extension (separate from ai-bridge) — connects to webai-proxy's WebSocket, operates browser AI pages
- Initial support: DeepSeek, with abstraction for future platforms
- SSE streaming + non-streaming response support
- Zero dependency on existing ai-bridge-mcp codebase

## Capabilities

### New Capabilities
- `openai-api`: OpenAI-compatible HTTP API endpoint (`/v1/chat/completions`), supporting both JSON and SSE streaming responses
- `browser-chat-bridge`: Chrome extension that connects to webai-proxy via WebSocket and operates browser AI chat pages via DOM/CDP
- `platform-abstraction`: Abstract interface for browser AI chat platforms, with DeepSeek as the initial implementation
- `sse-streaming`: Server-Sent Events streaming for real-time response delivery
- `auth`: Simple token-based authentication for HTTP and WebSocket connections

### Modified Capabilities

None — this is a new project, separate from existing ai-bridge.

## Impact

- New directory: `/home/snic/mcp/webai-proxy/` (Rust Cargo project)
- New Chrome extension in `webai-proxy/chrome-extension/`
- No changes to existing `ai-bridge/` codebase
- Default HTTP port TBD (suggest 4319, same as openbridge's style)
- Default WebSocket port TBD (suggest 9530)
```

## openspec/changes/create-webai-proxy/design.md

- Source: openspec/changes/create-webai-proxy/design.md
- Lines: 1-69
- SHA256: a7acf1a178fe1f006e32598ecd6d447d6b53635f592f4751060eee3481c88704

```md
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
```

## openspec/changes/create-webai-proxy/tasks.md

- Source: openspec/changes/create-webai-proxy/tasks.md
- Lines: 1-59
- SHA256: e59d47528cfb54d6f659f114b30fcbd253ca17ebceb423872f149ad966e541c2

```md
## 1. 项目脚手架

- [ ] 1.1 创建 webai-proxy Cargo 项目（Cargo.toml + 目录布局）
- [ ] 1.2 添加依赖（tokio, axum, tokio-tungstenite, serde, clap, futures-util, chrono）
- [ ] 1.3 实现文件日志模块（/tmp/webai-proxy.log）

## 2. 认证模块

- [ ] 2.1 实现 Token 认证（环境变量 WEBAI_PROXY_TOKEN + --token CLI 参数）
- [ ] 2.2 实现 axum middleware 检查 HTTP Authorization header
- [ ] 2.3 实现 WebSocket 连接 token 验证

## 3. WebSocket 服务

- [ ] 3.1 实现 WS 服务端（tokio-tungstenite），接受 Chrome 扩展连接
- [ ] 3.2 实现扩展连接管理（单连接 + pendingRequests 映射）
- [ ] 3.3 实现消息路由（requestId 匹配 + oneshot channel 响应）
- [ ] 3.4 实现扩展日志消息处理（[ext-log] 输出到文件）

## 4. OpenAI API 处理

- [ ] 4.1 定义 OpenAI 请求/响应类型（ChatCompletionRequest, ChatCompletionResponse 等）
- [ ] 4.2 实现 POST /v1/chat/completions handler（非流式 JSON 响应）
- [ ] 4.3 实现 SSE 流式响应（text/event-stream）
- [ ] 4.4 实现消息映射：OpenAI messages → new_session / send_message action
- [ ] 4.5 处理 system message 作为对话前缀

## 5. HTTP 服务

- [ ] 5.1 使用 axum 搭建 HTTP 服务（路由 + CORS + 中间件）
- [ ] 5.2 实现 GET /v1/models 端点（列出可用模型）
- [ ] 5.3 实现 GET /health 健康检查端点
- [ ] 5.4 添加错误处理（400/401/500 标准 JSON 错误）

## 6.平台抽象

- [ ] 6.1 定义 Platform trait（selectors, input, completion detection）
- [ ] 6.2 实现 DeepSeek 平台（复用 ai-bridge 的 selectors）
- [ ] 6.3 实现平台注册表（按名称查找）

## 7. Chrome 扩展

- [ ] 7.1 创建 manifest.json（MV3 配置）
- [ ] 7.2 实现 background.js（WebSocket 连接 + 消息路由 + 重连逻辑）
- [ ] 7.3 实现 content.js（DOM 操作：输入、发送、等待回复、提取）
- [ ] 7.4 实现默认配置（DeepSeek selectors）

## 8. CLI 入口

- [ ] 8.1 实现 clap CLI（serve 子命令）
- [ ] 8.2 参数支持：--ws-port, --http-port, --token, --log-file
- [ ] 8.3 启动时同时启动 HTTP 服务和 WS 服务

## 9. 验证

- [ ] 9.1 端到端测试：curl → HTTP → WS → Chrome 扩展 → DeepSeek
- [ ] 9.2 SSE 流式测试
- [ ] 9.3 认证测试（有效/无效/缺失 token）
- [ ] 9.4 重连测试（扩展断连后自动恢复）
```

## openspec/changes/create-webai-proxy/specs/auth/spec.md

- Source: openspec/changes/create-webai-proxy/specs/auth/spec.md
- Lines: 1-38
- SHA256: 2560a5220696ec605cf40e38682787e15bf405bea1333c781a5cd12c5aafca00

```md
## ADDED Requirements

### Requirement: Bearer token authentication for HTTP
All HTTP API endpoints SHALL require a valid Bearer token in the Authorization header.

#### Scenario: Valid token
- **WHEN** a request includes `Authorization: Bearer <valid_token>`
- **THEN** the request SHALL be processed normally

#### Scenario: Missing token
- **WHEN** a request has no Authorization header
- **THEN** the system SHALL return HTTP 401

#### Scenario: Invalid token
- **WHEN** a request has an Authorization header with an incorrect token
- **THEN** the system SHALL return HTTP 401

### Requirement: Token authentication for WebSocket
WebSocket connections SHALL authenticate using the token.

#### Scenario: WebSocket handshake with token
- **WHEN** a WS connection request includes the token as a query parameter
- **THEN** the system SHALL accept the connection

#### Scenario: WebSocket with invalid token
- **WHEN** a WS connection request has a missing or invalid token
- **THEN** the system SHALL reject the connection

### Requirement: Token configuration
The token SHALL be configurable via environment variable or CLI argument.

#### Scenario: Environment variable
- **WHEN** the `WEBAI_PROXY_TOKEN` environment variable is set
- **THEN** the system SHALL use its value as the authentication token

#### Scenario: CLI argument
- **WHEN** the `--token` CLI argument is provided
- **THEN** it SHALL override the environment variable value
```

## openspec/changes/create-webai-proxy/specs/browser-chat-bridge/spec.md

- Source: openspec/changes/create-webai-proxy/specs/browser-chat-bridge/spec.md
- Lines: 1-44
- SHA256: a2148d77e8d29f0294f425f25cd5a22aabd3f6c577e3a3374526342dae6e3882

```md
## ADDED Requirements

### Requirement: WebSocket connection to webai-proxy
The Chrome extension SHALL establish a persistent WebSocket connection to the webai-proxy server.

#### Scenario: Automatic connection on startup
- **WHEN** the extension starts
- **THEN** it SHALL attempt to connect to the configured WebSocket URL

#### Scenario: Reconnection on disconnect
- **WHEN** the WebSocket connection is lost
- **THEN** the extension SHALL automatically retry with backoff

### Requirement: Message routing to browser AI page
The extension SHALL forward incoming messages from the WS to the appropriate browser AI page via DOM operations.

#### Scenario: New session message
- **WHEN** the extension receives a `new_session` action with `platform: deepseek`
- **THEN** it SHALL open/focus a DeepSeek tab, type the message, and click send

#### Scenario: Send message (existing session)
- **WHEN** the extension receives a `send_message` action with a session URL
- **THEN** it SHALL navigate/focus the existing session and send the follow-up message

### Requirement: Response delivery
The extension SHALL detect when the AI response is complete and send it back through the WebSocket.

#### Scenario: Detect response completion
- **WHEN** the AI page has finished generating a response
- **THEN** the extension SHALL extract the response text and send it back via WS with the matching requestId

### Requirement: Token authentication
The extension SHALL authenticate with the server using the configured token.

#### Scenario: Authenticated connection
- **WHEN** the extension connects to the WebSocket
- **THEN** it SHALL pass the token as a query parameter or during the handshake

### Requirement: Debug logging
The extension SHALL support debug logging similar to ai-bridge's extension.

#### Scenario: Log messages
- **WHEN** the extension performs an operation
- **THEN** it SHALL send structured log messages to the server for diagnostics
```

## openspec/changes/create-webai-proxy/specs/openai-api/spec.md

- Source: openspec/changes/create-webai-proxy/specs/openai-api/spec.md
- Lines: 1-38
- SHA256: d40ce0b1fb5094b33b0652a89d38a4a5e4fef41421958da22f6b36730322a969

```md
## ADDED Requirements

### Requirement: OpenAI-compatible chat completions endpoint
The system SHALL expose a `POST /v1/chat/completions` endpoint that accepts OpenAI-compatible request format.

#### Scenario: Basic non-streaming request
- **WHEN** a client sends POST to `/v1/chat/completions` with `{"model":"deepseek","messages":[{"role":"user","content":"Hello"}]}`
- **THEN** the system SHALL return a JSON response with `choices[0].message.content` containing the AI reply

#### Scenario: Streaming request
- **WHEN** a client sends POST to `/v1/chat/completions` with `{"stream":true,"messages":[{"role":"user","content":"Hello"}]}`
- **THEN** the system SHALL return `text/event-stream` SSE response with per-chunk delta content

#### Scenario: Authentication required
- **WHEN** a request to `/v1/chat/completions` does not include a valid `Authorization: Bearer <token>` header
- **THEN** the system SHALL return HTTP 401 Unauthorized

#### Scenario: Unknown model
- **WHEN** a request specifies a model not recognized by the system
- **THEN** the system SHALL return a 400 error with a descriptive message

#### Scenario: System message handling
- **WHEN** the messages array contains a `system` role message
- **THEN** the system SHALL include it as an instruction/prefix to the conversation

### Requirement: Response format (non-streaming)
The non-streaming response SHALL follow OpenAI's chat completion JSON format.

#### Scenario: Non-streaming response structure
- **WHEN** a non-streaming request completes
- **THEN** the response SHALL contain `id`, `object`, `created`, `model`, `choices` array, and `usage` fields

### Requirement: Response format (streaming)
The streaming response SHALL follow OpenAI's SSE chat completion chunk format.

#### Scenario: Streaming response structure
- **WHEN** a streaming request is made
- **THEN** each SSE data line SHALL contain a JSON object with `choices[0].delta` (content deltas) and final `choices[0].finish_reason`
```

## openspec/changes/create-webai-proxy/specs/platform-abstraction/spec.md

- Source: openspec/changes/create-webai-proxy/specs/platform-abstraction/spec.md
- Lines: 1-30
- SHA256: 5092f7e41a68b4f500dbccae635e4b0957ff9abc868faf97a851968d8e333365

```md
## ADDED Requirements

### Requirement: Platform trait
The system SHALL define an abstract trait/interface for browser AI chat platforms.

#### Scenario: Define platform interface
- **WHEN** a new platform is added
- **THEN** it SHALL implement the platform trait with selectors, input methods, and completion detection logic

#### Scenario: Platform registration
- **WHEN** a platform implementation exists
- **THEN** it SHALL be registered in the platform registry for lookup by name

### Requirement: DeepSeek initial implementation
The system SHALL include a DeepSeek (chat.deepseek.com) platform implementation as the first platform.

#### Scenario: DeepSeek selectors
- **WHEN** operating on a DeepSeek page
- **THEN** the system SHALL use the correct selectors for input field, send button, and message elements

#### Scenario: DeepSeek session URL
- **WHEN** a new DeepSeek session is created
- **THEN** the system SHALL return the session URL for follow-up messages

### Requirement: Extensible platform design
The system SHALL be designed so that adding a new platform does not require changes to core HTTP or WebSocket logic.

#### Scenario: Add new platform
- **WHEN** a developer adds a new platform implementation
- **THEN** only the platform module needs to be added; no core routing or HTTP changes required
```

## openspec/changes/create-webai-proxy/specs/sse-streaming/spec.md

- Source: openspec/changes/create-webai-proxy/specs/sse-streaming/spec.md
- Lines: 1-23
- SHA256: 99f45ec862ef26d738da826a583744a243d5399bef7410f9b8d36a33c28aeadb

```md
## ADDED Requirements

### Requirement: SSE response format
The streaming response SHALL use `text/event-stream` content type with OpenAI-compatible SSE format.

#### Scenario: Stream start
- **WHEN** a streaming request starts processing
- **THEN** the first SSE event SHALL be `data: {"choices":[{"delta":{"role":"assistant"},"index":0}]}`

#### Scenario: Content chunks
- **WHEN** content is being generated
- **THEN** each SSE event SHALL contain `data: {"choices":[{"delta":{"content":"..."},"index":0}]}` with incremental content

#### Scenario: Stream end
- **WHEN** generation is complete
- **THEN** the final SSE event SHALL be `data: [DONE]` followed by a blank line

### Requirement: Streaming timeout
The system SHALL handle long-running streaming connections without dropping.

#### Scenario: Keep-alive
- **WHEN** no data has been sent for 15 seconds during streaming
- **THEN** the system SHALL send a comment line (`: keep-alive`) to keep the connection open
```

