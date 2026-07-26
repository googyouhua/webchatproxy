use std::sync::Arc;
use rmcp::{
    ServiceExt, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError,
    schemars,
};
use uuid::Uuid;
use tokio::sync::oneshot;

use crate::types::AppState;
use crate::ws_server;

const EXTENSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
const INITIAL_WAIT_SECS: u64 = 5;
const CONNECTION_WAIT_MAX: u64 = 30;

#[derive(Clone)]
pub struct BridgeHandler {
    pub state: Arc<AppState>,
    pub log_file: String,
    pub tool_router: ToolRouter<Self>,
}

impl BridgeHandler {
    pub fn new(state: Arc<AppState>, log_file: String) -> Self {
        Self {
            state,
            log_file,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct NewSessionParams {
    pub message: String,
    pub platform: String,
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct ChatParams {
    pub message: String,
    pub session_url: String,
    pub platform: String,
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct GetResultParams {
    pub ticket_id: String,
    #[serde(default)]
    pub timeout: Option<u64>,
}

async fn wait_for_turn(state: &Arc<AppState>) -> Result<(), String> {
    let mut logger = crate::log::Logger::new("/tmp/ai-bridge.log");
    for i in 0..60 {
        let busy = state.busy_sessions.lock().await;
        if busy.is_empty() {
            break;
        }
        drop(busy);
        if i > 0 && i % 10 == 0 {
            logger.log(&format!("Waiting for turn ({}s)...", i));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Ok(())
}

async fn mark_busy(state: &Arc<AppState>, ticket_id: &str) {
    state.busy_sessions.lock().await.insert(ticket_id.to_string());
}

async fn mark_done(state: &Arc<AppState>, ticket_id: &str) {
    state.busy_sessions.lock().await.remove(ticket_id);
}

async fn wait_for_connection(state: &Arc<AppState>) -> Result<(), String> {
    let mut logger = crate::log::Logger::new("/tmp/ai-bridge.log");
    for i in 0..CONNECTION_WAIT_MAX {
        if state.extension_socket.lock().await.is_some() {
            return Ok(());
        }
        if i > 0 && i % 5 == 0 {
            logger.log(&format!("Waiting for extension connection ({}s)...", i));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err("Extension not connected after 30s".to_string())
}

async fn next_ticket(state: &Arc<AppState>) -> String {
    let mut counter = state.ticket_counter.lock().await;
    *counter += 1;
    format!("t_{}", *counter)
}

async fn wait_for_result(state: &Arc<AppState>, ticket_id: &str, max_secs: u64) -> Option<CallToolResult> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(max_secs);
    loop {
        let mut results = state.results.lock().await;
        if let Some(response) = results.remove(ticket_id) {
            drop(results);
            return Some(format_result(response, state).await.unwrap_or_else(|_|
                CallToolResult::success(vec![Content::text("error")])
            ));
        }
        drop(results);
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

fn spawn_extension_task(
    state: Arc<AppState>,
    action: &'static str,
    payload: serde_json::Value,
    log_file: String,
    ticket_id: String,
    save_session_url: bool,
) {
    tokio::spawn(async move {
        let request_id = Uuid::new_v4().to_string();
        let ws_msg = serde_json::json!({
            "requestId": request_id,
            "action": action,
            "payload": payload,
        });

        let rx = match ws_server::send_to_extension_background(
            &state,
            &request_id,
            &ws_msg,
        ).await {
            Ok(rx) => rx,
            Err(e) => {
                let mut results = state.results.lock().await;
                results.insert(ticket_id.clone(), format!("error: {}", e));
                mark_done(&state, &ticket_id).await;
                return;
            }
        };

        let mut logger = crate::log::Logger::new(&log_file);
        logger.log(&format!("Sent {} (ticket={}, request={})", action, ticket_id, request_id));

        let response = match tokio::time::timeout(EXTENSION_TIMEOUT, rx).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(_)) => {
                let mut results = state.results.lock().await;
                results.insert(ticket_id.clone(), "error: channel closed".to_string());
                mark_done(&state, &ticket_id).await;
                return;
            }
            Err(_) => {
                state.pending.lock().await.remove(&request_id);
                let mut results = state.results.lock().await;
                results.insert(ticket_id.clone(), "error: timeout".to_string());
                mark_done(&state, &ticket_id).await;
                return;
            }
        };

        if save_session_url {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if let Some(url) = parsed.pointer("/data/sessionUrl").and_then(|v| v.as_str()) {
                    if !url.is_empty() {
                        state.sessions.lock().await.insert(url.to_string());
                    }
                }
            }
        }

        let mut results = state.results.lock().await;
        results.insert(ticket_id.clone(), response);
        mark_done(&state, &ticket_id).await;
    });
}

async fn format_result(response: String, state: &Arc<AppState>) -> Result<CallToolResult, McpError> {
    if response.starts_with("error:") {
        return Ok(CallToolResult::success(vec![Content::text(&response[7..])]));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response)
        .unwrap_or(serde_json::Value::String(response));

    if !parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        let error = parsed.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        return Ok(CallToolResult::success(vec![Content::text(error.to_string())]));
    }

    let text = parsed.pointer("/data/response").and_then(|v| v.as_str())
        .or_else(|| parsed.pointer("/data/text").and_then(|v| v.as_str()))
        .or_else(|| parsed.pointer("/data").and_then(|v| v.as_str()))
        .unwrap_or("ok");

    if let Some(url) = parsed.pointer("/data/sessionUrl").and_then(|v| v.as_str()) {
        if !url.is_empty() {
            state.sessions.lock().await.insert(url.to_string());
            return Ok(CallToolResult::success(
                vec![Content::text(format!("[session: {}]\n{}", url, text))]
            ));
        }
    }

    Ok(CallToolResult::success(vec![Content::text(text.to_string())]))
}

#[tool_router]
impl BridgeHandler {
    #[tool(description = "Check Chrome extension connection status")]
    async fn check_connection(&self) -> Result<CallToolResult, McpError> {
        let connected = self.state.extension_socket.lock().await.is_some();
        let status = if connected { "connected" } else { "disconnected" };
        Ok(CallToolResult::success(vec![Content::text(status)]))
    }

    #[tool(description = "Start a new AI chat session. Supported platforms: doubao (豆包), chatgpt, deepseek. Waits up to 5s for a quick response, otherwise returns a ticket_id — call get_result to poll.")]
    async fn new_session(
        &self,
        params: Parameters<NewSessionParams>,
    ) -> Result<CallToolResult, McpError> {
        wait_for_connection(&self.state).await
            .map_err(|e| McpError::internal_error(e, None::<serde_json::Value>))?;
        wait_for_turn(&self.state).await
            .map_err(|e| McpError::internal_error(e, None::<serde_json::Value>))?;

        let ticket_id = next_ticket(&self.state).await;
        mark_busy(&self.state, &ticket_id).await;
        let payload = serde_json::json!({
            "message": params.0.message,
            "platform": params.0.platform,
        });

        spawn_extension_task(
            self.state.clone(),
            "new_session",
            payload,
            self.log_file.clone(),
            ticket_id.clone(),
            true,
        );

        let result = wait_for_result(&self.state, &ticket_id, INITIAL_WAIT_SECS).await;
        match result {
            Some(ctr) => Ok(ctr),
            None => Ok(CallToolResult::success(vec![Content::text(ticket_id)])),
        }
    }

    #[tool(description = "Continue an existing AI chat session. Supported platforms: doubao (豆包), chatgpt, deepseek. Waits up to 5s for a quick response, otherwise returns a ticket_id — call get_result to poll.")]
    async fn chat(
        &self,
        params: Parameters<ChatParams>,
    ) -> Result<CallToolResult, McpError> {
        wait_for_connection(&self.state).await
            .map_err(|e| McpError::internal_error(e, None::<serde_json::Value>))?;
        wait_for_turn(&self.state).await
            .map_err(|e| McpError::internal_error(e, None::<serde_json::Value>))?;

        let ticket_id = next_ticket(&self.state).await;
        mark_busy(&self.state, &ticket_id).await;
        let payload = serde_json::json!({
            "message": params.0.message,
            "platform": params.0.platform,
            "sessionUrl": params.0.session_url,
        });

        spawn_extension_task(
            self.state.clone(),
            "send_message",
            payload,
            self.log_file.clone(),
            ticket_id.clone(),
            false,
        );

        let result = wait_for_result(&self.state, &ticket_id, INITIAL_WAIT_SECS).await;
        match result {
            Some(ctr) => Ok(ctr),
            None => Ok(CallToolResult::success(vec![Content::text(ticket_id)])),
        }
    }

    #[tool(description = "Retrieve the result of a previous new_session or chat call using its ticket_id. Returns 'pending' if not ready yet, the AI response text if done, or an error message. Optional timeout (seconds, default 0) to keep waiting before returning 'pending'. Recommended: timeout=5.")]
    async fn get_result(
        &self,
        params: Parameters<GetResultParams>,
    ) -> Result<CallToolResult, McpError> {
        let wait = params.0.timeout.unwrap_or(0);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(wait);

        loop {
            let mut results = self.state.results.lock().await;
            if let Some(response) = results.remove(&params.0.ticket_id) {
                return format_result(response, &self.state).await;
            }
            drop(results);

            if tokio::time::Instant::now() >= deadline {
                return Ok(CallToolResult::success(vec![Content::text("pending")]));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    #[tool(description = "List all active AI chat session URLs created in this session.")]
    async fn list_sessions(&self) -> Result<CallToolResult, McpError> {
        let sessions = self.state.sessions.lock().await;
        if sessions.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text("No active sessions")]));
        }
        let lines: Vec<String> = sessions.iter().enumerate()
            .map(|(i, url)| format!("{}. {}", i + 1, url))
            .collect();
        Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
    }
}

#[tool_handler]
impl ServerHandler for BridgeHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Bridge between MCP and browser-based AI chat services. AI responses take 1-3 minutes. Use new_session/chat to start, then poll get_result(ticket_id) until it returns a non-'pending' response.".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run_mcp_server(state: Arc<AppState>, log_file: String) {
    let handler = BridgeHandler::new(state, log_file);
    match handler.serve(stdio()).await {
        Ok(service) => {
            if let Err(e) = service.waiting().await {
                eprintln!("MCP server runtime error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("MCP server error: {}", e);
        }
    }
}
