## Tasks

### T1: 修改 popup.html 显示文本
- `<h2>AI Bridge</h2>` → `<h2>webai-proxy</h2>`
- 服务器地址 placeholder: 9527 → 9530
- 底部提示文字: "MCP Server" → "webai-proxy"

### T2: 修改 popup.js 默认端口
- 默认 serverUrl: `ws://localhost:9527` → `ws://localhost:9530`

### T3: 恢复 background.js 的 popup 通信 + Badge
- 添加 `updateConnectionState(state, detail)` 函数
- 添加 `chrome.runtime.onMessage` 监听器处理 getStatus/reconnect/disconnect
- 在 WS 连接/断开/重试时调用 updateConnectionState
- 添加 `updateBadge(text, color)` 函数
- 在 updateConnectionState 中根据 state 设置 badge: "ON" 绿色/#4CAF50, "..." 橙色/#FF9800, 断开清空
