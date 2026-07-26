## Why

webai-proxy 的 Rust 服务器在日志路径上硬编码了 Unix 路径 `/tmp/webai-proxy.log`，且 WS 模块多处独立创建 Logger 实例而非使用共享配置，导致 Windows 和 macOS 上日志不可用。需要修复这些平台兼容性问题并补充构建/运行文档，使 webai-proxy 能在 Windows/Linux/macOS 上无缝工作。

## What Changes

- 日志默认路径从硬编码 `/tmp/webai-proxy.log` 改为使用 `std::env::temp_dir()` 实现跨平台
- ws.rs 中多处硬编码的 Logger 改为共享 `AppState.log_path`
- main.rs 中 `0.0.0.0` 绑定已跨平台，无需修改
- 更新 USAGE.md 补充 Windows 构建和运行说明
- 更新 log.rs 测试用例使用临时目录而非 `/tmp/`

## Capabilities

### New Capabilities
- `cross-platform-log`: 跨平台日志路径支持，基于 `std::env::temp_dir()`

### Modified Capabilities
- `openai-api`: 日志文件路径配置变为跨平台默认值

## Impact

- src/main.rs: 日志默认路径改为动态计算
- src/ws.rs: 多处硬编码日志路径改为使用共享 log_path
- src/state.rs: AppState 新增 log_path 字段
- src/log.rs: 测试用例使用临时目录
- USAGE.md: 补充 Windows 构建/运行说明
