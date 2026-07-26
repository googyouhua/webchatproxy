mod auth;
mod bridge;
mod log;
mod openai;
mod server;
mod state;
mod ws;

use clap::Parser;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "webai-proxy", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the webai-proxy server
    Serve {
        /// HTTP server port
        #[arg(long, default_value = "4319", env = "WEBAI_PROXY_HTTP_PORT")]
        http_port: u16,
        /// WebSocket server port
        #[arg(long, default_value = "9530", env = "WEBAI_PROXY_WS_PORT")]
        ws_port: u16,
        /// Authentication token (optional, empty = no auth)
        #[arg(long, env = "WEBAI_PROXY_TOKEN", default_value = "")]
        token: String,
        /// Log file path (default: OS temp dir + webai-proxy.log)
        #[arg(long, env = "WEBAI_PROXY_LOG_FILE")]
        log_file: Option<String>,
        /// Auto-generate a random auth token and print to stdout
        #[arg(long, default_value_t = false)]
        gen_token: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { http_port, ws_port, token, log_file, gen_token } => {
            let token = if gen_token && token.is_empty() {
                let t = auth::generate_token();
                println!("Generated auth token: {}", t);
                t
            } else {
                token
            };
            let state = Arc::new(state::AppState {
                pending: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
                extension_socket: Arc::new(tokio::sync::Mutex::new(None)),
                active_session: Arc::new(tokio::sync::Mutex::new(None)),
            });

            let log_path = log_file.unwrap_or_else(log::default_log_path);
            log::init_global_logger(&log_path);
            log::global_log(&format!("Starting webai-proxy (HTTP={}, WS={})", http_port, ws_port));

            let ws_state = state.clone();
            tokio::spawn(async move {
                ws::start_ws_server(ws_state, ws_port).await;
            });

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            let app_state = server::AppStateExt {
                state: state.clone(),
                server_token: token,
            };
            let router = server::create_router(app_state);
            let addr = format!("0.0.0.0:{}", http_port);
            log::global_log(&format!("HTTP server listening on {}", addr));

            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            axum::serve(listener, router).await.unwrap();
        }
    }
}
