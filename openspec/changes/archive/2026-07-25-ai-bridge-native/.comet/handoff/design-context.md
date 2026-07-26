# Comet Design Handoff

- Change: ai-bridge-native
- Phase: design
- Mode: compact
- Context hash: 2172eb35a1c99744ed8b695f459b09e177475930406ef75dedb847509d2819c6

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/ai-bridge-native/proposal.md

- Source: openspec/changes/ai-bridge-native/proposal.md
- Lines: 1-31
- SHA256: 0b9e257b8963947fcc3fa395b265fe58dce0eba50497dbb1f787b7e20b220918

```md
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
```

## openspec/changes/ai-bridge-native/design.md

- Source: openspec/changes/ai-bridge-native/design.md
- Lines: 1-65
- SHA256: ee6db86ee9aa92edec37916d5e830d0abd6dd4e1abafabdaefb6af4aafe4ae8a

```md
## Context

ai-bridge 的 MCP Server（mcp-server/index.js）是一个 Node.js 程序，通过 `@modelcontextprotocol/sdk` 提供 stdio MCP 服务，同时用 `ws` 库运行 WebSocket 服务端与 Chrome 插件通信。需要将其完整移植到 Rust，消除 Node.js 依赖。

## Goals / Non-Goals

**Goals:**
- 用 Rust 重写 MCP Server，输出单二进制
- 保持与 Chrome 插件的 WebSocket 通信协议完全兼容
- 保持 MCP 工具接口（`check_connection`、`new_session`、`ask_ai`）不变
- 保持 Token 认证机制不变
- 支持主/副实例切换

**Non-Goals:**
- 不改 Chrome 插件（js 代码不变）
- 不引入新功能
- 不做性能优化（保持原有逻辑即可）

## Decisions

### 项目结构
单 Cargo crate，整体架构：

```
mcp-server/
├── Cargo.toml
└── src/
    ├── main.rs          # 入口：解析参数，启动服务
    ├── ws_server.rs     # WebSocket 服务端/客户端
    ├── mcp_handler.rs   # MCP 工具定义 + stdio 处理
    ├── auth.rs          # Token 校验
    ├── log.rs           # 日志（文件 + stderr）
    └── types.rs         # 共享类型
```

### 依赖选择
| 组件 | Crate | 说明 |
|------|-------|------|
| 异步运行时 | `tokio` | 标准选择 |
| WebSocket | `tokio-tungstenite` | 与 Node.js `ws` 对应 |
| MCP 协议 | `rmcp` v0.16 | 官方 Rust SDK，支持 `server` + `transport-io` |
| JSON 序列化 | `serde_json` | |
| 参数解析 | `clap` |

### Token 认证
从 WebSocket URL query string 提取 `token` 参数，与 `AI_BRIDGE_MCP_TOKEN` 环境变量比对。逻辑同 Node.js 版：

```rust
// 连接时校验 token
let token = std::env::var("AI_BRIDGE_MCP_TOKEN")
    .unwrap_or_else(|_| generate_random_token());
// 拒绝不匹配的连接
```

### 主/副实例
同现有逻辑：
- 尝试绑定 WS 端口，成功 → 主实例
- 端口被占 → 副实例，作为 WebSocket 客户端连接主实例

### 日志
写入 `/tmp/ai-bridge.log` + stderr，格式相同。

## Risks / Trade-offs

- 随机 Token 只在启动时生成一次，副实例需从主实例同步或独立生成（同现有行为）
```

## openspec/changes/ai-bridge-native/tasks.md

- Source: openspec/changes/ai-bridge-native/tasks.md
- Lines: 1-36
- SHA256: 4564dd3c79cf628e7c16d24b1791a8c83c6249ae91a66a1998bc5e33fe8d2a9e

```md
## 1. 项目脚手架

- [ ] 1.1 创建 Cargo crate 结构（Cargo.toml + 目录布局）
- [ ] 1.2 添加依赖（tokio, tokio-tungstenite, serde_json, clap）

## 2. 日志模块

- [ ] 2.1 实现文件日志（/tmp/ai-bridge.log）+ stderr 双写
- [ ] 2.2 日志格式与 Node.js 版一致（ISO 时间戳 + 消息）

## 3. WebSocket 服务

- [ ] 3.1 实现主实例 WebSocket 服务端（端口 9527 / 来自 SYNC_WS_PORT 环境变量）
- [ ] 3.2 实现 Token 认证（从 URL query string 提取 token，与 AI_BRIDGE_MCP_TOKEN 比对）
- [ ] 3.3 实现 Chrome 插件连接管理（extensionSocket，消息转发）
- [ ] 3.4 实现副实例模式（端口被占时自动作为 WebSocket 客户端连接主实例）
- [ ] 3.5 实现 pendingRequests 映射 + 消息路由

## 4. MCP 协议

- [ ] 4.1 实现 MCP stdio JSON-RPC 传输层（stdin/stdout）
- [ ] 4.2 实现 check_connection 工具
- [ ] 4.3 实现 new_session 工具
- [ ] 4.4 实现 ask_ai 工具
- [ ] 4.5 实现 waitForConnection 逻辑（等待插件就绪最多 30s）

## 5. 安装/配置命令

- [ ] 5.1 实现 install 子命令（自动写入 opencode.jsonc）
- [ ] 5.2 支持参数：--ws-port, --log-file, --token

## 6. 验证

- [ ] 6.1 与现有 Chrome 插件联调测试
- [ ] 6.2 主/副实例切换测试
- [ ] 6.3 Token 认证测试
```

