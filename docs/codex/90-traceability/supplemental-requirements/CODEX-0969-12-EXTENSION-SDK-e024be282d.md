# Supplemental Requirement: Plugin SDK Governed Writes

Prompt ID: `CODEX-0969-12-EXTENSION-SDK-e024be282d`
Primary Prompt: `CODEX-0105-12-EXTENSION-SDK-914ad16fbe`
Current-safe module: `extension_sdk::plugin_sdk`

## Merge Result

P0: Plugin SDK commands must retain idempotency key, expected version, actor, authority mode, visibility, fact provenance, correlation id, and causation id.

P0: Direct LLM calls, direct database writes, direct Event Store append, internal Tool Gate access, Authority Contract mutation, dice forging, and restricted visibility reveal remain forbidden capabilities.

## Boundary

This supplemental prompt owns no Rust src/test output. Implementation changes, if needed later, belong to the primary prompt above.

