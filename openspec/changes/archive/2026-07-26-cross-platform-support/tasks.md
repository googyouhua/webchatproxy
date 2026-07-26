## Tasks

### T1: 跨平台日志路径
- [x] 在 `log.rs` 中添加 `default_log_path()` 函数（基于 `std::env::temp_dir()`）
- [x] 在 `log.rs` 中添加 `OnceLock` 全局 Logger（`init_global_logger` + `global_log`）
- [x] 修改 `main.rs` 中 `--log-file` 默认值为 `Option<String>`，运行时选择默认路径
- [x] 初始化全局 Logger、调用 `global_log` 替代直接 Logger 创建

### T2: ws.rs 共享日志路径
- [x] 替换 ws.rs 中所有 `Logger::new("/tmp/webai-proxy.log")` 为 `crate::log::global_log()`
- [x] 移除硬编码路径依赖，使用 OnceLock 全局单例（无需修改函数签名）

### T3: log.rs 测试兼容性
- [x] 测试用例中的 `/tmp/` 路径改为 `std::env::temp_dir()`
- [x] 新增全局 Logger 测试（`test_global_log`、`test_default_log_path_uses_temp_dir`）

### T4: 文档更新
- [x] USAGE.md 补充 Windows 构建说明和注意事项
- [x] USAGE.md 注明日志路径在不同平台的位置

### T5: 额外功能
- [x] 添加 `--gen-token` 参数：不指定 `--token` 时自动生成随机 token
- [x] 添加 GitHub CI release workflow（三平台构建 + cargo test + 发布）
- [x] `auth.rs` 新增 `generate_token()` 函数及测试

### T6: 验证
- [x] Linux 下 `cargo build` 和 `cargo test` 通过（28 tests）
- [x] curl 测试 HTTP 端点正常
- [x] `--gen-token` 功能验证通过
- [x] 自定义日志路径 `--log-file` 验证通过
