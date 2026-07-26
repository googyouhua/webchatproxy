use crate::openai::ChatCompletionRequest;
use crate::state::AppState;
use crate::ws;
use std::sync::Arc;
use uuid::Uuid;

pub fn build_message_content(request: &ChatCompletionRequest) -> Result<String, String> {
    let messages = &request.messages;
    let last_user = messages.iter().rev().find(|m| m.role == "user")
        .ok_or("No user message found")?;
    let system_prefix: String = messages.iter()
        .find(|m| m.role == "system")
        .map(|m| format!("{}\n\n", m.content))
        .unwrap_or_default();
    Ok(format!("{}{}", system_prefix, last_user.content))
}

pub fn parse_extension_response(
    response: &str,
    action: &str,
    active_session: &mut Option<String>,
) -> Result<String, String> {
    let parsed: serde_json::Value = serde_json::from_str(response)
        .map_err(|_| "Empty or invalid extension response".to_string())?;

    if !parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        let error = parsed.get("error").and_then(|v| v.as_str()).unwrap_or("Extension error");
        if error.contains("session") || error.contains("invalid") || error.contains("stale") {
            active_session.take();
        }
        return Err(error.to_string());
    }

    if action == "new_session" {
        if let Some(url) = parsed.pointer("/data/sessionUrl").and_then(|v| v.as_str()) {
            if url.starts_with("https://chat.deepseek.com/a/chat/s/") {
                *active_session = Some(url.to_string());
            }
        }
    }

    parsed.pointer("/data/response")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Empty or invalid extension response".to_string())
}

pub async fn send_to_extension(
    state: &Arc<AppState>,
    request: &ChatCompletionRequest,
) -> Result<String, String> {
    let content = build_message_content(request)?;
    let session_url = state.active_session.lock().await.clone();

    let (action, payload) = match session_url {
        Some(url) => (
            "send_message",
            serde_json::json!({
                "message": content,
                "platform": "deepseek",
                "sessionUrl": url,
            }),
        ),
        None => (
            "new_session",
            serde_json::json!({
                "message": content,
                "platform": "deepseek",
            }),
        ),
    };

    let request_id = Uuid::new_v4().to_string();
    let ws_msg = serde_json::json!({
        "requestId": request_id,
        "action": action,
        "payload": payload,
    });

    let rx = ws::send_request(state, &request_id, &ws_msg).await?;

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(600),
        rx,
    ).await
        .map_err(|_| "Timeout waiting for extension response".to_string())?
        .map_err(|_| "Response channel closed".to_string())?;

    let mut session_guard = state.active_session.lock().await;
    let result = parse_extension_response(&response, action, &mut session_guard);
    if let Err(ref err) = result {
        let resp_preview = if response.len() > 200 { format!("{}...", &response[..200]) } else { response.clone() };
        crate::log::global_log(&format!("bridge: extension error - {} (raw: {})", err, resp_preview));
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::openai::*;

    #[test]
    fn test_parse_success_response() {
        let json = r#"{"requestId":"abc","success":true,"data":{"response":"Hello! How can I help?","sessionUrl":"https://chat.deepseek.com/a/chat/s/123"},"error":null}"#;
        let mut session = None;
        let result = super::parse_extension_response(json, "send_message", &mut session);
        assert_eq!(result.unwrap(), "Hello! How can I help?");
        assert!(session.is_none(), "send_message should not save session");
    }

    #[test]
    fn test_parse_error_response() {
        let json = r#"{"requestId":"abc","success":false,"data":null,"error":"Extension error: session expired"}"#;
        let mut session = Some("https://chat.deepseek.com/a/chat/s/old".into());
        let result = super::parse_extension_response(json, "send_message", &mut session);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session expired"));
        assert!(session.is_none(), "error should clear stale session");
    }

    #[test]
    fn test_parse_missing_data_response() {
        let json = r#"{"requestId":"abc","success":true,"data":null,"error":null}"#;
        let mut session = None;
        let result = super::parse_extension_response(json, "send_message", &mut session);
        assert_eq!(result.unwrap_err(), "Empty or invalid extension response");
    }

    #[test]
    fn test_parse_new_session_saves_url() {
        let json = r#"{"requestId":"abc","success":true,"data":{"response":"Hello!","sessionUrl":"https://chat.deepseek.com/a/chat/s/abc123"},"error":null}"#;
        let mut session = None;
        let result = super::parse_extension_response(json, "new_session", &mut session);
        assert_eq!(result.unwrap(), "Hello!");
        assert_eq!(session.as_deref(), Some("https://chat.deepseek.com/a/chat/s/abc123"));
    }

    #[test]
    fn test_parse_very_long_response() {
        let long_text = "A".repeat(100_000);
        let json = format!(r#"{{"requestId":"abc","success":true,"data":{{"response":"{}","sessionUrl":"https://chat.deepseek.com/a/chat/s/123"}},"error":null}}"#, long_text);
        let mut session = None;
        let result = super::parse_extension_response(&json, "send_message", &mut session);
        assert_eq!(result.unwrap().len(), 100_000);
    }

    #[test]
    fn test_parse_response_with_json_injection() {
        let text = r#"Here's the JSON: {"key": "value"}. And done."#;
        let json = format!(r#"{{"requestId":"abc","success":true,"data":{{"response":"{}","sessionUrl":"url"}},"error":null}}"#, text.replace('"', "\\\""));
        let mut session = None;
        let result = super::parse_extension_response(&json, "send_message", &mut session);
        assert!(result.unwrap().contains("JSON"));
    }

    #[test]
    fn test_last_user_message_extracted() {
        let req = ChatCompletionRequest {
            model: "deepseek".into(),
            messages: vec![
                ChatMessage { role: "system".into(), content: "You are helpful".into() },
                ChatMessage { role: "user".into(), content: "Hi".into() },
                ChatMessage { role: "assistant".into(), content: "Hello!".into() },
                ChatMessage { role: "user".into(), content: "What is Rust?".into() },
            ],
            stream: false,
        };
        let result = super::build_message_content(&req).unwrap();
        assert_eq!(result, "You are helpful\n\nWhat is Rust?");
    }

    #[test]
    fn test_no_system_message() {
        let req = ChatCompletionRequest {
            model: "deepseek".into(),
            messages: vec![
                ChatMessage { role: "user".into(), content: "Hello".into() },
            ],
            stream: false,
        };
        let result = super::build_message_content(&req).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_no_user_message_returns_error() {
        let req = ChatCompletionRequest {
            model: "deepseek".into(),
            messages: vec![
                ChatMessage { role: "assistant".into(), content: "Hello there".into() },
            ],
            stream: false,
        };
        let result = super::build_message_content(&req);
        assert!(result.is_err());
    }
}
