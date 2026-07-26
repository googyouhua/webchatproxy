mod log;
mod auth;
mod types;
mod ws_server;
mod mcp_handler;

use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "ai-bridge-mcp", version = "1.1.0")]
enum Cli {
    /// Run the MCP server (default)
    #[command(name = "run")]
    Run {
        #[arg(long, default_value = "9527")]
        ws_port: u16,
        #[arg(long, default_value = "/tmp/ai-bridge.log")]
        log_file: String,
        #[arg(long)]
        token: Option<String>,
    },
    /// Install to OpenCode configuration
    #[command(name = "install")]
    Install {
        #[arg(long, default_value = "9527")]
        ws_port: u16,
        #[arg(long)]
        token: Option<String>,
    },
}

fn resolve_token(cli_token: Option<String>) -> String {
    cli_token
        .or_else(|| std::env::var("AI_BRIDGE_MCP_TOKEN").ok())
        .unwrap_or_else(auth::generate_random_token)
}

fn default_opencode_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/opencode")
}

fn find_opencode_config(config_dir: &Path) -> PathBuf {
    let candidates = ["opencode.jsonc", "opencode.json"];
    for name in &candidates {
        let p = config_dir.join(name);
        if p.exists() {
            return p;
        }
    }
    config_dir.join("opencode.jsonc")
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli {
        Cli::Run { ws_port, log_file, token } => {
            let mut logger = log::Logger::new(&log_file);
            logger.log("ai-bridge-mcp starting...");

            let token = resolve_token(token);
            let auth = auth::Auth::new(token);
            logger.log(&format!("Token: {}", auth.token()));

            let state = std::sync::Arc::new(types::AppState {
                pending: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
                auth,
                extension_socket: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
                sessions: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashSet::new(),
                )),
                results: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
                ticket_counter: std::sync::Arc::new(tokio::sync::Mutex::new(0)),
                busy_sessions: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashSet::new(),
                )),
            });

            let ws_handle = tokio::spawn(ws_server::start_ws_server(
                state.clone(),
                ws_port,
                log_file.clone(),
            ));

            let mcp_handle = tokio::spawn(mcp_handler::run_mcp_server(
                state.clone(),
                log_file.clone(),
            ));

            tokio::select! {
                _ = ws_handle => logger.log("WebSocket server exited"),
                _ = mcp_handle => logger.log("MCP server exited"),
            }

            logger.log("ai-bridge-mcp shutting down");
        }
        Cli::Install { ws_port, token } => {
            let token = resolve_token(token);
            let config_dir = default_opencode_config_dir();
            let config_file = find_opencode_config(&config_dir);

            let binary_path = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("ai-bridge-mcp"));

            let mcp_config = serde_json::json!({
                "type": "local",
                "command": [binary_path.to_string_lossy().to_string(), "run", "--ws-port", ws_port.to_string()],
                "environment": {
                    "AI_BRIDGE_MCP_TOKEN": token,
                },
                "timeout": 300000,
            });

            let config_content = serde_json::json!({
                "mcp": {
                    "ai-bridge": mcp_config
                }
            });

            println!("将添加以下 MCP 配置到 OpenCode:\n");
            println!("{}", serde_json::to_string_pretty(&config_content).unwrap());
            println!("\n配置文件: {}", config_file.display());

            if config_file.exists() {
                println!("该文件已存在，新配置会合并进去（只修改 mcp.ai-bridge 字段，不影响其他配置）。");
            } else {
                println!("该文件不存在，将创建新文件。");
            }

            print!("\n确认写入？(y/N): ");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer).unwrap();
            let answer = answer.trim().to_lowercase();

            if answer != "y" && answer != "yes" {
                println!("已取消。请手动将上面的配置添加到配置文件中。");
                std::process::exit(0);
            }

            // Ensure config directory exists
            std::fs::create_dir_all(&config_dir).unwrap_or_else(|e| {
                eprintln!("无法创建配置目录: {}", e);
                std::process::exit(1);
            });

            // Read existing config if present
            let mut existing: serde_json::Value = if config_file.exists() {
                let raw = std::fs::read_to_string(&config_file).unwrap_or_default();
                // Strip comments for JSONC support
                let stripped = raw.lines()
                    .filter(|l| !l.trim().starts_with("//"))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::from_str(&stripped).unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            };

            // If existing is Null or Object, merge
            if existing.is_null() {
                existing = serde_json::json!({});
            }

            if let Some(obj) = existing.as_object_mut() {
                if !obj.contains_key("mcp") {
                    obj.insert("mcp".to_string(), serde_json::json!({}));
                }
                if let Some(mcp) = obj.get_mut("mcp").and_then(|v| v.as_object_mut()) {
                    mcp.insert("ai-bridge".to_string(), mcp_config);
                }
            }

            // Backup existing file
            if config_file.exists() {
                let backup = config_file.with_extension("jsonc.backup");
                if let Err(e) = std::fs::copy(&config_file, &backup) {
                    eprintln!("备份失败: {}", e);
                }
                println!("已备份原文件到: {}", backup.display());
            }

            let output = serde_json::to_string_pretty(&existing).unwrap();
            std::fs::write(&config_file, output + "\n").unwrap_or_else(|e| {
                eprintln!("写入配置失败: {}", e);
                std::process::exit(1);
            });

            println!("\n配置完成！");
            println!("Chrome 插件请手动加载: chrome://extensions → 加载已解压的扩展程序 → 选择 chrome-extension/ 目录");
        }
    }
}
