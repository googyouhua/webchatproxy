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
