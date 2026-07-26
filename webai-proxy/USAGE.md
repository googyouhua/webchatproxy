# webai-proxy

webai-proxy 是一个 OpenAI 兼容的 HTTP 代理服务，通过 Chrome 扩展操作 DeepSeek Web 版，将浏览器中的 AI 对话能力封装为标准 API。

## 架构

```
HTTP 客户端 → webai-proxy (Rust) → WebSocket → Chrome 扩展 → content script → DeepSeek 网页
                                                    (操作 DOM: 输入→发送→等待→提取)
```

- **Rust 服务器**: 监听 HTTP 请求（OpenAI Chat Completions 格式），通过 WebSocket 转发给浏览器扩展
- **Chrome 扩展**: 接收指令，在 DeepSeek 网页上自动输入、发送、等待并提取回复
- **content script**: 注入 DeepSeek 页面，执行 DOM 操作和结果提取

## 快速开始

### 1. 构建服务器

```bash
cd webai-proxy
cargo build --release
```

### 2. 启动服务器

```bash
./target/release/webai-proxy serve
```

默认 HTTP 端口 `4319`，WS 端口 `9530`，无认证。更多选项见配置章节。

### 3. 加载 Chrome 扩展

1. 打开 Chrome → `chrome://extensions`
2. 启用"开发者模式"
3. "加载已解压的扩展程序" → 选择 `webai-proxy/chrome-extension/`
4. 固定扩展图标到工具栏

### 4. 配置扩展

点击扩展图标，设置：
- **服务器地址**: `ws://localhost:9530`（与 `--ws-port` 一致）
- **Token**: 如果服务器设置了 `--token`，填写相同值
- 启用连接开关

扩展图标会显示连接状态：
- 绿色 `ON` — 已连接
- 橙色 `...` — 连接中/重试中
- 无 badge — 已断开

### 5. 打开 DeepSeek

浏览器中打开 https://chat.deepseek.com 并登录。保持标签页打开。

### 6. 调用 API

```bash
curl http://localhost:4319/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek","messages":[{"role":"user","content":"你好"}],"stream":false}'
```

## 配置

### 服务器 CLI 参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| `--http-port` | `WEBAI_PROXY_HTTP_PORT` | `4319` | HTTP API 端口 |
| `--ws-port` | `WEBAI_PROXY_WS_PORT` | `9530` | WebSocket 端口（与扩展通信） |
| `--token` | `WEBAI_PROXY_TOKEN` | `空` | API 鉴权 token，空=不鉴权 |
| `--log-file` | 无 | `/tmp/webai-proxy.log` | 日志文件路径 |

示例：

```bash
# 自定义端口 + 开启鉴权
./target/release/webai-proxy serve \
  --http-port 8080 \
  --ws-port 9090 \
  --token sk-my-secret

# 使用环境变量
export WEBAI_PROXY_TOKEN=sk-my-secret
./target/release/webai-proxy serve
```

### 扩展设置

通过弹出面板配置，保存在 `chrome.storage.local`：

- **服务器地址**: WS 连接地址，必须与服务器 `--ws-port` 一致
- **Token**: 与 `--token` 一致（留空则不鉴权）
- **启用连接**: 开关 WS 连接

## API 参考

### `GET /health`

健康检查，无需认证。

```bash
curl http://localhost:4319/health
# {"status":"ok"}
```

### `GET /v1/models`

列出可用模型。

```bash
curl http://localhost:4319/v1/models
# {"object":"list","data":[{"id":"deepseek","object":"model","created":1710000000,"owned_by":"webai-proxy"}]}
```

### `POST /v1/chat/completions`

OpenAI 兼容的对话补全接口。

#### 请求格式

```json
{
  "model": "deepseek",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "你好"}
  ],
  "stream": false
}
```

- `model`: 必须以 `deepseek` 开头（如 `deepseek`, `deepseek-chat`）
- `messages`: 对话消息列表，最后一条 `user` 消息作为问题
- `stream`: `true` 启用 SSE 流式输出，`false` 返回完整响应

#### 非流式响应

```json
{
  "id": "chatcmpl-550e8400-e29b-41d4-a716-446655440000",
  "object": "chat.completion",
  "created": 1710000000,
  "model": "deepseek",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "你好！有什么可以帮助你的吗？"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {}
}
```

#### 流式响应 (SSE)

`stream: true` 时返回 Server-Sent Events：

```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"你好"},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"！"},"finish_reason":null}]}

data: [DONE]
```

15 秒无数据时发送保持心跳（空行）。

#### 流式示例 (curl)

```bash
curl -N http://localhost:4319/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek","messages":[{"role":"user","content":"写一首诗"}],"stream":true}'
```

#### 认证

如果服务器设置了 `--token`，所有请求需要添加 Header：

```
Authorization: Bearer <token>
```

```bash
curl http://localhost:4319/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-my-secret" \
  -d '{"model":"deepseek","messages":[{"role":"user","content":"你好"}]}'
```

## 工作流程

1. HTTP 客户端发送请求到 `POST /v1/chat/completions`
2. Rust 服务器解析消息，生成唯一 `requestId`
3. 通过 WebSocket 发送给 Chrome 扩展：
   - 无活跃会话 → `action: "new_session"`
   - 已有会话 → `action: "send_message"`（附带 session URL，复用同一对话）
4. 扩展找到/打开 DeepSeek 标签页，转发给 content script
5. Content script 执行 DOM 操作：
   - 在输入框输入消息
   - 点击发送按钮
   - 等待 AI 回复（检测文本稳定性，3 轮连续一致）
   - 提取 Markdown 回复文本
6. 结果原路返回：content script → 扩展 → WebSocket → 服务器 → HTTP 响应

## 日志

服务器日志同时输出到 stderr 和日志文件（默认 `/tmp/webai-proxy.log`）：

```
[2026-07-26T12:00:00.123456789+08:00] HTTP server on 0.0.0.0:4319
[2026-07-26T12:00:00.123456789+08:00] WS server on 127.0.0.1:9530
[2026-07-26T12:00:05.123456789+08:00] msg: new_session deepseek
[2026-07-26T12:00:15.123456789+08:00] CS result: done 1234 chars
```

Chrome 扩展日志输出在 DevTools 的 Service Worker 控制台中。

## 常见问题

**扩展显示"未连接"**
- 确认服务器正在运行
- 确认扩展中服务器地址与 `--ws-port` 一致（默认 9530）
- 检查 Token 是否匹配

**请求返回 503 Extension not connected**
- 扩展未连接 WebSocket，打开扩展弹出面板检查状态

**请求返回 401 Unauthorized**
- 服务器设置了 `--token` 但请求未包含 `Authorization` header

**请求返回 400**
- `model` 不是以 `deepseek` 开头
- `messages` 为空或格式错误

**DeepSeek 页面未自动输入**
- 确认 DeepSeek 标签页已打开并登录
- 刷新 DeepSeek 页面，确保 content script 已注入
- 查看扩展 Service Worker 控制台日志（`chrome://extensions` → webai-proxy → 服务 worker → 控制台）

**响应返回空或极短**
- Content script 的文本稳定性检测可能误判，扩展的 `waitForResponse` 会在文本 3 秒不变时返回

**多轮对话未保持在同一会话**
- 服务器缓存 `active_session` URL，首次 `new_session` 后后续都复用。如果 DeepSeek 页面关闭或会话过期，服务器会自动检测并重新 `new_session`
