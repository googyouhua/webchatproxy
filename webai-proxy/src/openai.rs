use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct DeltaChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<DeltaChoice>,
}

#[derive(Debug, Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct ListModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

impl ChatCompletionResponse {
    pub fn new(model: &str, content: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".into(),
            created: now,
            model: model.into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content,
                },
                finish_reason: "stop".into(),
            }],
            usage: Usage::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_can_parse_request() {
        let json = r#"{"model":"deepseek","messages":[{"role":"user","content":"hi"}],"stream":false}"#;
        let req = serde_json::from_str::<super::ChatCompletionRequest>(json).unwrap();
        assert_eq!(req.model, "deepseek");
        assert_eq!(req.messages.len(), 1);
        assert!(!req.stream);
    }

    #[test]
    fn test_can_build_response() {
        let resp = super::ChatCompletionResponse::new("deepseek", "Hello world".into());
        assert_eq!(resp.model, "deepseek");
        assert_eq!(resp.choices[0].message.content, "Hello world");
        assert_eq!(resp.choices[0].finish_reason, "stop");
    }

    #[test]
    fn test_response_serializes() {
        let resp = super::ChatCompletionResponse::new("deepseek", "OK".into());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("chat.completion"));
        assert!(json.contains("\"OK\""));
    }

    #[test]
    fn test_streaming_chunk_serializes() {
        let chunk = super::ChatCompletionChunk {
            id: "chunk-1".into(),
            object: "chat.completion.chunk".into(),
            created: 1000,
            model: "deepseek".into(),
            choices: vec![super::DeltaChoice {
                index: 0,
                delta: super::Delta { role: Some("assistant".into()), content: Some("Hello".into()) },
                finish_reason: None,
            }],
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("delta"));
    }

    #[test]
    fn test_list_models() {
        let models = super::ListModelsResponse {
            object: "list".into(),
            data: vec![super::ModelInfo {
                id: "deepseek".into(),
                object: "model".into(),
                created: 1710000000,
                owned_by: "webai-proxy".into(),
            }],
        };
        let json = serde_json::to_string(&models).unwrap();
        assert!(json.contains("deepseek"));
    }
}
