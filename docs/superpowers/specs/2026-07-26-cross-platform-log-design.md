---
comet_change: cross-platform-support
role: technical-design
canonical_spec: openspec
archived-with: 2026-07-26-cross-platform-support
status: final
---

# Cross-Platform Logger Design

## Problem

webai-proxy 硬编码 `/tmp/webai-proxy.log` 作为日志路径，Windows/macOS 下不可用。且 ws.rs 多处独立创建 Logger 实例，无法统一配置。

## Solution

使用 `std::sync::OnceLock` 实现全局 Logger。

### log.rs

```rust
use std::sync::OnceLock;

static GLOBAL_LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init_global_logger(path: &str) {
    GLOBAL_LOGGER.get_or_init(|| Logger::new(path));
}

pub fn global_log(message: &str) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.log(message);
    } else {
        eprintln!("{}", message);
    }
}
```

### main.rs

默认日志路径：

```rust
fn default_log_path() -> String {
    let mut path = std::env::temp_dir();
    path.push("webai-proxy.log");
    path.to_string_lossy().to_string()
}
```

- Linux: `/tmp/webai-proxy.log`
- Windows: `C:\Users\<user>\AppData\Local\Temp\webai-proxy.log`
- macOS: `/tmp/webai-proxy.log`

启动时初始化：

```rust
log::init_global_logger(&log_file);
```

### ws.rs

所有 `Logger::new("/tmp/webai-proxy.log")` 替换为 `crate::log::global_log(...)`。函数签名不变。

## Files Changed

| File | Change |
|------|--------|
| `src/log.rs` | 新增 `OnceLock` 全局 Logger、`init_global_logger()`、`global_log()` |
| `src/main.rs` | 默认路径改为 `default_log_path()`，调用 `init_global_logger()` |
| `src/ws.rs` | 所有硬编码 Logger 替换为 `global_log()` |
| `USAGE.md` | 补充 Windows 节 |
