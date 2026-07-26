## Why

ai-bridge 的 MCP Server 目前依赖 Node.js 运行时（mcp-server/index.js），需要 `npm install` 才能运行。将其重写为 Rust 后变为单二进制，消除 Node.js 依赖，部署更简单（直接配置到 opencode.jsonc / Zap .mcp.json 的 command 字段），跨平台分发也更便捷。

## What Changes

- 用 Rust 重写 `mcp-server/index.js`，输出一个独立二进制
- 保持功能完全不变：
  - WebSocket 服务端（:9527），与 Chrome 插件通信
  - MCP stdio 服务端，暴露 `check_connection`、`new_session`、`ask_ai` 三个工具
  - Token 认证（`?token=xxx` 参数校验）
  - 主/副实例切换（端口被占时自动变客户端）
  - 日志写入 `/tmp/ai-bridge.log`
- 移除 `package.json`、`node_modules`、`npm install` 流程
- Chrome 插件（chrome-extension/）不变

## Capabilities

### New Capabilities

无 — 这是移植，不引入新能力

### Modified Capabilities

无

## Impact

- 依赖从 `@modelcontextprotocol/sdk` + `ws` + `zod` 变为 Cargo crate：`tokio` + `tokio-tungstenite` + `mcp-sdk-rust`（或手动实现 MCP 协议）
- 部署方式从 `npm install && node index.js` 变为直接运行二进制
- Chrome 插件不变，通信协议（WebSocket JSON 格式）不变
