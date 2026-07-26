## ADDED Requirements

### Requirement: SSE response format
The streaming response SHALL use `text/event-stream` content type with OpenAI-compatible SSE format.

#### Scenario: Stream start
- **WHEN** a streaming request starts processing
- **THEN** the first SSE event SHALL be `data: {"choices":[{"delta":{"role":"assistant"},"index":0}]}`

#### Scenario: Content chunks
- **WHEN** content is being generated
- **THEN** each SSE event SHALL contain `data: {"choices":[{"delta":{"content":"..."},"index":0}]}` with incremental content

#### Scenario: Stream end
- **WHEN** generation is complete
- **THEN** the final SSE event SHALL be `data: [DONE]` followed by a blank line

### Requirement: Streaming timeout
The system SHALL handle long-running streaming connections without dropping.

#### Scenario: Keep-alive
- **WHEN** no data has been sent for 15 seconds during streaming
- **THEN** the system SHALL send a comment line (`: keep-alive`) to keep the connection open
