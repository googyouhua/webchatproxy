> **Status**: implemented ✓

## Why

插件 popup 窗口标题显示 "AI Bridge"、服务器地址默认端口为 9527、提示文字提及 "MCP Server"，均为从 ai-bridge 复制后遗漏修改。同时 background.js 移除了 popup 消息处理和 connectionState 写入，导致 popup 无法显示连接状态。

## What Changes

- popup.html: 标题改为 "webai-proxy"，默认端口改为 9530，提示文字改为 webai-proxy
- popup.js: 默认服务器地址改为 ws://localhost:9530
- background.js: 恢复 popup 交互（getStatus/reconnect/disconnect），恢复 connectionState 存储

## Capabilities

### New Capabilities
无

### Modified Capabilities
无

## Impact

- chrome-extension/popup.html
- chrome-extension/popup.js
- chrome-extension/background.js
