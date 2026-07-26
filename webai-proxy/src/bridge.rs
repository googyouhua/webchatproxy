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

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
        if !parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            let error = parsed.get("error").and_then(|v| v.as_str()).unwrap_or("Extension error");
            if error.contains("session") || error.contains("invalid") || error.contains("stale") {
                state.active_session.lock().await.take();
            }
            return Err(error.to_string());
        }

        if action == "new_session" {
            if let Some(url) = parsed.pointer("/data/sessionUrl").and_then(|v| v.as_str()) {
                if url.starts_with("https://chat.deepseek.com/a/chat/s/") {
                    *state.active_session.lock().await = Some(url.to_string());
                }
            }
        }

        if let Some(text) = parsed.pointer("/data/response").and_then(|v| v.as_str()) {
            return Ok(text.to_string());
        }
    }

    Err("Empty or invalid extension response".to_string())
}

#[cfg(test)]
mod tests {
    use crate::openai::*;

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
