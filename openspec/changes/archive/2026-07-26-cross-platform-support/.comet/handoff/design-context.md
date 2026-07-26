# Comet Design Handoff

- Change: cross-platform-support
- Phase: design
- Mode: compact
- Context hash: 44161d2def813fea8a56162e7967c4b22f7b95113d48030384e3c1c83521f213

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/cross-platform-support/proposal.md

- Source: openspec/changes/cross-platform-support/proposal.md
- Lines: 1-27
- SHA256: 12a5b20b18fe7c914513422ead4c840f161a35c25142731869d62517468fa926

```md
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
```

## openspec/changes/cross-platform-support/design.md

- Source: openspec/changes/cross-platform-support/design.md
- Lines: 1-39
- SHA256: 64ad4743d1d589f660c858be01e4d36b8e6ba9d08f6a63b73f5af06351bdc5ec

```md
## 设计

### 日志路径跨平台

```rust
// 默认日志路径
fn default_log_path() -> String {
    let mut path = std::env::temp_dir();
    path.push("webai-proxy.log");
    path.to_string_lossy().to_string()
}
```

**Linux**: `/tmp/webai-proxy.log`  
**Windows**: `C:\Users\<user>\AppData\Local\Temp\webai-proxy.log`  
**macOS**: `/tmp/webai-proxy.log`

### AppState 共享日志路径

state.rs 新增字段：

```rust
pub struct AppState {
    pub log_path: String,
    // ... 其他字段
}
```

所有 Logger 实例通过 `AppState.log_path` 获取路径，消除 ws.rs 中的硬编码。

### 文件修改清单

| 文件 | 修改 |
|------|------|
| `src/state.rs` | 新增 `log_path: String` 字段 |
| `src/main.rs` | 默认值 `"/tmp/webai-proxy.log"` → `default_log_path()`, 传给 AppState |
| `src/ws.rs` | 函数签名添加 `state: &AppState` 参数，Logger 通过 state.log_path 创建 |
| `src/log.rs` | 测试用例使用 `std::env::temp_dir()` |
| `USAGE.md` | 补充 Windows 节 |
```

## openspec/changes/cross-platform-support/tasks.md

- Source: openspec/changes/cross-platform-support/tasks.md
- Lines: 1-23
- SHA256: 0e1b9445f077b0b4e53225a5e8e0cfa88b54ecd9faaaf311205f851bcf147716

```md
## Tasks

### T1: 跨平台日志路径
- [ ] 在 `state.rs` 的 `AppState` 中添加 `log_path: String` 字段
- [ ] 在 `main.rs` 中添加 `default_log_path()` 函数（基于 `std::env::temp_dir()`）
- [ ] 修改 `main.rs` 中 `--log-file` 默认值为 `default_log_path()` 的调用结果
- [ ] 初始化 AppState 时传入 `log_path`

### T2: ws.rs 共享日志路径
- [ ] 修改 `start_ws_server` 签名接受 `state: &Arc<AppState>` 参数
- [ ] 替换 ws.rs 中所有 `Logger::new("/tmp/webai-proxy.log")` 为 `Logger::new(&state.log_path)`
- [ ] 更新 `main.rs` 中调用 `start_ws_server` 处传递 state 引用

### T3: log.rs 测试兼容性
- [ ] 测试用例中的 `/tmp/` 路径改为 `std::env::temp_dir()`

### T4: 文档更新
- [ ] USAGE.md 补充 Windows 构建说明和注意事项
- [ ] USAGE.md 注明日志路径在不同平台的位置

### T5: 验证
- [ ] Linux 下 `cargo build` 和 `cargo test` 通过
- [ ] curl 测试 HTTP 和 SSE 端点正常
```

