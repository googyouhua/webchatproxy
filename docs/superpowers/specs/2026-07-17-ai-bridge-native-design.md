---
comet_change: ai-bridge-native
role: technical-design
canonical_spec: openspec
---

# ai-bridge-native: 技术设计

## 概述

将 ai-bridge 的 MCP Server（Node.js）用 Rust 移植为单二进制，保持与现有 Chrome 插件的 WebSocket 通信协议完全兼容。

## 架构

```
OpenCode / Zap (MCP Client)
    │  stdin/stdout (JSON-RPC over stdio)
    ▼
ai-bridge-native (Rust, 单二进制)
    ├── MCP Server (rmcp crate, transport-io)
    │   ├── check_connection
    │   ├── new_session
    │   └── ask_ai
    │
    └── WebSocket Server (:9527)
        └── Chrome Extension (background.js)
            └── Content Script → AI 聊天页面 DOM
```

## 项目结构

```
mcp-server/
├── Cargo.toml
└── src/
    ├── main.rs           # 入口：CLI 参数 + 启动
    ├── ws_server.rs      # WebSocket 服务端/客户端
    ├── mcp_handler.rs    # RMCP ServerHandler 实现
    ├── auth.rs           # Token 校验
    ├── log.rs            # 日志（文件 + stderr）
    └── types.rs          # 共享类型
```

## 依赖

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
rmcp = { version = "0.16", features = ["server", "transport-io"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

## 核心流程

### MCP 工具定义

使用 `rmcp` 宏声明三个工具：

```rust
#[tool(tool_box)]
impl ServerHandler for BridgeHandler {
    #[tool(description = "Check Chrome extension connection status")]
    async fn check_connection(&self) -> String { ... }

    #[tool(description = "Create a new AI chat session")]
    async fn new_session(&self, #[tool(description = "Initial message")] message: String,
                         #[tool(description = "Platform name, e.g. doubao, chatgpt, deepseek")] platform: String) -> String { ... }

    #[tool(description = "Continue an existing AI chat session")]
    async fn ask_ai(&self, #[tool(description = "Follow-up message")] message: String,
                    #[tool(description = "Session URL from new_session")] session_url: String,
                    #[tool(description = "Platform name, e.g. doubao, chatgpt, deepseek")] platform: String) -> String { ... }
}
```

### WebSocket 通信

- 与 Chrome 插件通过 WebSocket 通信，JSON 消息格式与现有 `background.js` 完全兼容
- 连接时校验 `?token=xxx` 参数
- `pendingRequests` 映射：MCP 请求 → WS 转发 → 等待回复 → 返回结果

### 主/副实例

- 主实例：启动 WebSocket 服务端绑定 `SYNC_WS_PORT`（默认 9527）
- 副实例：端口被占时作为 WebSocket 客户端连接主实例
- 主实例负责路由：自己的 `requestId` 直接处理，否则转发给副实例

### 日志

- 写入 `/tmp/ai-bridge.log`（环境变量 `AI_BRIDGE_LOG` 可覆盖）
- 同时输出到 stderr
- 格式：`[ISO_TIMESTAMP] message`

### Token

- 环境变量 `AI_BRIDGE_MCP_TOKEN`
- 未设置时自动生成 32 位 hex 随机串，打印到日志
- 通过 WebSocket URL query string 传入并校验

## CLI

```bash
ai-bridge-native [OPTIONS]

OPTIONS:
  --ws-port <PORT>      WebSocket 端口 [default: 9527]
  --log-file <PATH>     日志路径 [default: /tmp/ai-bridge.log]
  --token <TOKEN>       MCP Token [default: 环境变量 AI_BRIDGE_MCP_TOKEN]

SUBCOMMANDS:
  install               安装到 OpenCode 配置
```

## 测试策略

- 单元测试：各模块独立测试
- 集成测试：启动实例 + 模拟 WebSocket 客户端连接 + 验证消息往返
- 与现有 Chrome 插件联调
