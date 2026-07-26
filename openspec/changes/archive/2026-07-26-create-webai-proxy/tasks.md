## 1. 项目脚手架

- [x] 1.1 创建 webai-proxy Cargo 项目（Cargo.toml + 目录布局）
- [x] 1.2 添加依赖（tokio, axum, tokio-tungstenite, serde, clap, futures-util, chrono）
- [x] 1.3 实现文件日志模块（/tmp/webai-proxy.log）

## 2. 认证模块

- [x] 2.1 实现 Token 认证（环境变量 WEBAI_PROXY_TOKEN + --token CLI 参数）
- [x] 2.2 实现 axum middleware 检查 HTTP Authorization header
- [x] 2.3 实现 WebSocket 连接 token 验证

## 3. WebSocket 服务

- [x] 3.1 实现 WS 服务端（tokio-tungstenite），接受 Chrome 扩展连接
- [x] 3.2 实现扩展连接管理（单连接 + pendingRequests 映射）
- [x] 3.3 实现消息路由（requestId 匹配 + oneshot channel 响应）
- [x] 3.4 实现扩展日志消息处理（[ext-log] 输出到文件）

## 4. OpenAI API 处理

- [x] 4.1 定义 OpenAI 请求/响应类型（ChatCompletionRequest, ChatCompletionResponse 等）
- [x] 4.2 实现 POST /v1/chat/completions handler（非流式 JSON 响应）
- [x] 4.3 实现 SSE 流式响应（text/event-stream）
- [x] 4.4 实现消息映射：OpenAI messages → new_session / send_message action
- [x] 4.5 处理 system message 作为对话前缀

## 5. HTTP 服务

- [x] 5.1 使用 axum 搭建 HTTP 服务（路由 + CORS + 中间件）
- [x] 5.2 实现 GET /v1/models 端点（列出可用模型）
- [x] 5.3 实现 GET /health 健康检查端点
- [x] 5.4 添加错误处理（400/401/500 标准 JSON 错误）

## 6.平台抽象

- [x] 6.1 定义 Platform trait（selectors, input, completion detection）
- [x] 6.2 实现 DeepSeek 平台（复用 ai-bridge 的 selectors）
- [x] 6.3 实现平台注册表（按名称查找）

## 7. Chrome 扩展

- [x] 7.1 创建 manifest.json（MV3 配置）
- [x] 7.2 实现 background.js（WebSocket 连接 + 消息路由 + 重连逻辑）
- [x] 7.3 实现 content.js（DOM 操作：输入、发送、等待回复、提取）
- [x] 7.4 实现默认配置（DeepSeek selectors）

## 8. CLI 入口

- [x] 8.1 实现 clap CLI（serve 子命令）
- [x] 8.2 参数支持：--ws-port, --http-port, --token, --log-file
- [x] 8.3 启动时同时启动 HTTP 服务和 WS 服务

## 9. 验证

- [x] 9.1 端到端测试：curl → HTTP → WS → Chrome 扩展 → DeepSeek
- [x] 9.2 SSE 流式测试
- [x] 9.3 认证测试（有效/无效/缺失 token）
- [x] 9.4 重连测试（扩展断连后自动恢复）
