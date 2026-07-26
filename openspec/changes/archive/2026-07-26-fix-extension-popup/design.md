## 修改方案

### popup.html
1. `<h2>AI Bridge</h2>` → `<h2>webai-proxy</h2>`
2. `placeholder="ws://localhost:9527"` → `placeholder="ws://localhost:9530"`
3. 提示文字: "确保 MCP Server 已启动..." → "确保 webai-proxy 已启动，且 Token 与 webai-proxy 配置一致。"

### popup.js
1. 默认 serverUrl: `ws://localhost:9527` → `ws://localhost:9530`

### background.js
恢复 popup 通信所需的关键函数（与 ai-bridge 一致）：
- `updateConnectionState(state, detail)` — 写 chrome.storage.local.connectionState + 通知 popup
- `chrome.runtime.onMessage.addListener` — 处理 getStatus / reconnect / disconnect 消息
- `reconnectWs()` — 重新读取 settings 并连接 WS
- 确保 WS 连接/断连时调用 updateConnectionState 更新 popup 状态

### 影响文件
3 个文件，修改均为字符串或函数恢复，无架构变动。
