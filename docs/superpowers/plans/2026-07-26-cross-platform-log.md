---
change: cross-platform-support
design-doc: docs/superpowers/specs/2026-07-26-cross-platform-log-design.md
base-ref: 172a2a68a8e5b91dc189802de032280bb9df9d1d
archived-with: 2026-07-26-cross-platform-support
---

# Implementation Plan: Cross-Platform Logger

## Overview

将硬编码的 `/tmp/webai-proxy.log` 日志路径改为跨平台实现，使用 `OnceLock` 全局 Logger。

## Tasks

### T1: log.rs — 全局 Logger

Add `static GLOBAL_LOGGER: OnceLock<Logger>` + `init_global_logger(path)` + `global_log(msg)`.

- `init_global_logger`: calls `GLOBAL_LOGGER.get_or_init(|| Logger::new(path))`
- `global_log`: `GLOBAL_LOGGER.get().map(|l| l.log(msg))` else `eprintln!`
- Import `std::sync::OnceLock`
- Update tests: `/tmp/` → `std::env::temp_dir()`

### T2: main.rs — 默认路径 + 初始化

- Add `fn default_log_path()` returning `std::env::temp_dir() + "webai-proxy.log"`
- Change `--log-file` default: `default_value = "/tmp/webai-proxy.log"` → `default_value = default_log_path()` is not possible with clap attribute — instead use `default_value_t` or set default in `log_file` assignment
- Call `log::init_global_logger(&log_file)` instead of `Logger::new(&log_file)`

### T3: ws.rs — 替换硬编码 Logger

Replace all 4 occurrences of `Logger::new("/tmp/webai-proxy.log")` with `crate::log::global_log(...)`.

Remove `use crate::log::Logger;` if no longer used.

### T4: USAGE.md — Windows 说明

- Add Windows section: build with `cargo build`, run server
- Note log path on different platforms
- Ensure curl examples work on Windows (PowerShell escaping)

### T5: 验证

- `cargo test` passes
- `cargo build` succeeds
- curl test: `curl http://localhost:4319/health`
