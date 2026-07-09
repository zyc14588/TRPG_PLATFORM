# Supplemental Requirement: Extension Compatibility Matrix Governance

Prompt ID: `CODEX-0968-12-EXTENSION-SDK-e572d9864c`
Primary Prompt: `CODEX-0947-12-EXTENSION-SDK-f162f72984`
Current-safe module: `extension_sdk::extension_compatibility_matrix`

## Merge Result

P0: Compatibility checks must reject reports missing `extension_id`, `ruleset_version`, `tool_schema_version`, or `compatibility_result`.

P1: Compatibility evidence must remain an auditable SDK report and must not become a plugin-owned write path into Event Store or projection state.

## Boundary

This supplemental prompt owns no Rust src/test output. Implementation changes, if needed later, belong to the primary prompt above.

