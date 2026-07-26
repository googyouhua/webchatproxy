use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_util::stream;
use futures_util::StreamExt;
use std::convert::Infallible;
use tower_http::cors::CorsLayer;

use crate::bridge;
use crate::openai::*;
use crate::state::AppState;

#[derive(Clone)]
pub struct AppStateExt {
    pub state: Arc<AppState>,
    pub server_token: String,
}

pub fn create_router(app_state: AppStateExt) -> Router {
    let token = app_state.server_token.clone();

    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .route("/health", get(health))
        .layer(axum::middleware::from_fn(move |req: Request, next: Next| {
            let token = token.clone();
            async move {
                let path = req.uri().path();
                if path == "/health" {
                    return Ok(next.run(req).await);
                }
                if !token.is_empty() {
                    match crate::auth::extract_bearer_token(req.headers()) {
                        Some(t) if t == token => {}
                        _ => {
                            return Err((
                                StatusCode::UNAUTHORIZED,
                                serde_json::json!({"error": "invalid or missing token"}).to_string(),
                            ));
                        }
                    }
                }
                Ok(next.run(req).await)
            }
        }))
        .layer(CorsLayer::permissive())
        .with_state(app_state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn list_models(
    State(_ext): State<AppStateExt>,
) -> impl IntoResponse {
    Json(ListModelsResponse {
        object: "list".into(),
        data: vec![ModelInfo {
            id: "deepseek".into(),
            object: "model".into(),
            created: 1710000000,
            owned_by: "webai-proxy".into(),
        }],
    })
}

async fn chat_completions(
    State(ext): State<AppStateExt>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<serde_json::Value>)> {
    if req.messages.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "messages is empty"}))));
    }

    if !req.model.starts_with("deepseek") {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "unsupported model"}))));
    }

    if ext.state.extension_socket.lock().await.is_none() {
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "extension not connected"}))));
    }

    let response_text = bridge::send_to_extension(&ext.state, &req).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e}))))?;

    if req.stream {
        let chunks = chunk_text(&response_text, 3);
        let stream = stream::iter(chunks.into_iter().map(move |chunk| {
            let chunk_data = ChatCompletionChunk {
                id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                object: "chat.completion.chunk".into(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                model: req.model.clone(),
                choices: vec![DeltaChoice {
                    index: 0,
                    delta: Delta { role: None, content: Some(chunk) },
                    finish_reason: None,
                }],
            };
            Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&chunk_data).unwrap()))
        }));

        let done_event = Event::default().data("[DONE]");
        let full_stream = stream.chain(futures_util::stream::once(async { Ok::<_, Infallible>(done_event) }));

        Ok(Sse::new(full_stream)
            .keep_alive(axum::response::sse::KeepAlive::default().interval(std::time::Duration::from_secs(15)))
            .into_response())
    } else {
        let resp = ChatCompletionResponse::new(&req.model, response_text);
        Ok(Json(resp).into_response())
    }
}

fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars.chunks(chunk_size)
        .map(|c| c.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_chunk_text_small() {
        let result = super::chunk_text("abc", 2);
        assert_eq!(result, vec!["ab".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_chunk_text_exact() {
        let result = super::chunk_text("hello", 5);
        assert_eq!(result, vec!["hello".to_string()]);
    }

    #[test]
    fn test_chunk_text_overflow() {
        let result = super::chunk_text("x", 5);
        assert_eq!(result, vec!["x".to_string()]);
    }

    #[test]
    fn test_health_response_format() {
        let json = serde_json::json!({"status": "ok"});
        assert_eq!(json["status"], "ok");
    }
}
