# Brainstorm Summary

- Change: cross-platform-support
- Date: 2026-07-26

## Confirmed Technical Approach

使用 `OnceLock` 全局 Logger 替代 AppState 传递日志路径：
- `log.rs`: 新增 `static GLOBAL_LOGGER: OnceLock<Logger>` + `init_global_logger()` + `global_log()`
- `ws.rs`: 所有 `Logger::new("/tmp/...")` 替换为 `crate::log::global_log()`
- `main.rs`: `--log-file` 默认值改为 `default_log_path()`（基于 `std::env::temp_dir()`），调用 `log::init_global_logger()`
- 测试用例使用 `std::env::temp_dir()`

## Key Trade-offs and Risks

- 全局变量需在 WS 服务启动前初始化（main.rs 中顺序保证）
- OnceLock 在 Rust 1.70+ 标准库可用，无需第三方依赖
- ws.rs 函数签名不变，改动最小

## Testing Strategy

- Linux 下 `cargo test` 全部通过
- curl 测试 HTTP/SSE 端点

## Spec Patches

None
