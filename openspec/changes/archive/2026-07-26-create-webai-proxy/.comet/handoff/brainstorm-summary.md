# Brainstorm Summary

- Change: create-webai-proxy
- Date: 2026-07-25

## Confirmed Technical Approach

Architecture: Rust (axum) HTTP server + embedded tokio-tungstenite WS server in single process. Separate Chrome extension using CDP + Content Script dual-channel (same pattern as ai-bridge). Token auth via WEBAI_PROXY_TOKEN. Single-session reuse (new_session on first request, send_message on subsequent). Simulated SSE streaming (extension returns full response, server chunks into SSE). Only last user message sent (system prepended to first user). Session expiry returns error.

## Key Trade-offs and Risks

- Simulated streaming vs real token-by-token: much simpler but loses real-time feel
- Single session: simple but can't handle concurrent conversations
- Separate extension: user manages two Chrome extensions
- Background tab SPA issue: same problem ai-bridge solved with CDP, confirmed to reuse same approach

## Testing Strategy

- End-to-end: curl → HTTP → WS → extension → DeepSeek
- Non-streaming and streaming SSE paths
- Auth: valid/invalid/missing token
- Session expiry: trigger and verify error response

## Spec Patches

None.
