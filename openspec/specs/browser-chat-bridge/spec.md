# browser-chat-bridge Specification

## Purpose
TBD - created by archiving change create-webai-proxy. Update Purpose after archive.
## Requirements
### Requirement: WebSocket connection to webai-proxy
The Chrome extension SHALL establish a persistent WebSocket connection to the webai-proxy server.

#### Scenario: Automatic connection on startup
- **WHEN** the extension starts
- **THEN** it SHALL attempt to connect to the configured WebSocket URL

#### Scenario: Reconnection on disconnect
- **WHEN** the WebSocket connection is lost
- **THEN** the extension SHALL automatically retry with backoff

### Requirement: Message routing to browser AI page
The extension SHALL forward incoming messages from the WS to the appropriate browser AI page via DOM operations.

#### Scenario: New session message
- **WHEN** the extension receives a `new_session` action with `platform: deepseek`
- **THEN** it SHALL open/focus a DeepSeek tab, type the message, and click send

#### Scenario: Send message (existing session)
- **WHEN** the extension receives a `send_message` action with a session URL
- **THEN** it SHALL navigate/focus the existing session and send the follow-up message

### Requirement: Response delivery
The extension SHALL detect when the AI response is complete and send it back through the WebSocket.

#### Scenario: Detect response completion
- **WHEN** the AI page has finished generating a response
- **THEN** the extension SHALL extract the response text and send it back via WS with the matching requestId

### Requirement: Token authentication
The extension SHALL authenticate with the server using the configured token.

#### Scenario: Authenticated connection
- **WHEN** the extension connects to the WebSocket
- **THEN** it SHALL pass the token as a query parameter or during the handshake

### Requirement: Debug logging
The extension SHALL support debug logging similar to ai-bridge's extension.

#### Scenario: Log messages
- **WHEN** the extension performs an operation
- **THEN** it SHALL send structured log messages to the server for diagnostics

