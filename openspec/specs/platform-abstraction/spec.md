# platform-abstraction Specification

## Purpose
TBD - created by archiving change create-webai-proxy. Update Purpose after archive.
## Requirements
### Requirement: Platform trait
The system SHALL define an abstract trait/interface for browser AI chat platforms.

#### Scenario: Define platform interface
- **WHEN** a new platform is added
- **THEN** it SHALL implement the platform trait with selectors, input methods, and completion detection logic

#### Scenario: Platform registration
- **WHEN** a platform implementation exists
- **THEN** it SHALL be registered in the platform registry for lookup by name

### Requirement: DeepSeek initial implementation
The system SHALL include a DeepSeek (chat.deepseek.com) platform implementation as the first platform.

#### Scenario: DeepSeek selectors
- **WHEN** operating on a DeepSeek page
- **THEN** the system SHALL use the correct selectors for input field, send button, and message elements

#### Scenario: DeepSeek session URL
- **WHEN** a new DeepSeek session is created
- **THEN** the system SHALL return the session URL for follow-up messages

### Requirement: Extensible platform design
The system SHALL be designed so that adding a new platform does not require changes to core HTTP or WebSocket logic.

#### Scenario: Add new platform
- **WHEN** a developer adds a new platform implementation
- **THEN** only the platform module needs to be added; no core routing or HTTP changes required

