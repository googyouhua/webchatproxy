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
