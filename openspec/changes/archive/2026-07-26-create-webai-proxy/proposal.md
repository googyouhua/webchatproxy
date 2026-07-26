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
