## ADDED Requirements

### Requirement: Bearer token authentication for HTTP
All HTTP API endpoints SHALL require a valid Bearer token in the Authorization header.

#### Scenario: Valid token
- **WHEN** a request includes `Authorization: Bearer <valid_token>`
- **THEN** the request SHALL be processed normally

#### Scenario: Missing token
- **WHEN** a request has no Authorization header
- **THEN** the system SHALL return HTTP 401

#### Scenario: Invalid token
- **WHEN** a request has an Authorization header with an incorrect token
- **THEN** the system SHALL return HTTP 401

### Requirement: Token authentication for WebSocket
WebSocket connections SHALL authenticate using the token.

#### Scenario: WebSocket handshake with token
- **WHEN** a WS connection request includes the token as a query parameter
- **THEN** the system SHALL accept the connection

#### Scenario: WebSocket with invalid token
- **WHEN** a WS connection request has a missing or invalid token
- **THEN** the system SHALL reject the connection

### Requirement: Token configuration
The token SHALL be configurable via environment variable or CLI argument.

#### Scenario: Environment variable
- **WHEN** the `WEBAI_PROXY_TOKEN` environment variable is set
- **THEN** the system SHALL use its value as the authentication token

#### Scenario: CLI argument
- **WHEN** the `--token` CLI argument is provided
- **THEN** it SHALL override the environment variable value
