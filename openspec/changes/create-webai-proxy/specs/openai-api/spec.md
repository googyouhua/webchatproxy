## ADDED Requirements

### Requirement: OpenAI-compatible chat completions endpoint
The system SHALL expose a `POST /v1/chat/completions` endpoint that accepts OpenAI-compatible request format.

#### Scenario: Basic non-streaming request
- **WHEN** a client sends POST to `/v1/chat/completions` with `{"model":"deepseek","messages":[{"role":"user","content":"Hello"}]}`
- **THEN** the system SHALL return a JSON response with `choices[0].message.content` containing the AI reply

#### Scenario: Streaming request
- **WHEN** a client sends POST to `/v1/chat/completions` with `{"stream":true,"messages":[{"role":"user","content":"Hello"}]}`
- **THEN** the system SHALL return `text/event-stream` SSE response with per-chunk delta content

#### Scenario: Authentication required
- **WHEN** a request to `/v1/chat/completions` does not include a valid `Authorization: Bearer <token>` header
- **THEN** the system SHALL return HTTP 401 Unauthorized

#### Scenario: Unknown model
- **WHEN** a request specifies a model not recognized by the system
- **THEN** the system SHALL return a 400 error with a descriptive message

#### Scenario: System message handling
- **WHEN** the messages array contains a `system` role message
- **THEN** the system SHALL include it as an instruction/prefix to the conversation

### Requirement: Response format (non-streaming)
The non-streaming response SHALL follow OpenAI's chat completion JSON format.

#### Scenario: Non-streaming response structure
- **WHEN** a non-streaming request completes
- **THEN** the response SHALL contain `id`, `object`, `created`, `model`, `choices` array, and `usage` fields

### Requirement: Response format (streaming)
The streaming response SHALL follow OpenAI's SSE chat completion chunk format.

#### Scenario: Streaming response structure
- **WHEN** a streaming request is made
- **THEN** each SSE data line SHALL contain a JSON object with `choices[0].delta` (content deltas) and final `choices[0].finish_reason`
