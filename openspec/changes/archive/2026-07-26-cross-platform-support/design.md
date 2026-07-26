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
