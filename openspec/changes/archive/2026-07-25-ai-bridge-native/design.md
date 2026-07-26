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
