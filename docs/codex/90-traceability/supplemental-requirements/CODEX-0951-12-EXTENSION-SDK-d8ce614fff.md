# Supplemental Requirement: Compatibility Matrix Fields

Prompt ID: `CODEX-0951-12-EXTENSION-SDK-d8ce614fff`
Primary Prompt: `CODEX-0947-12-EXTENSION-SDK-f162f72984`
Current-safe module: `extension_sdk::extension_compatibility_matrix`

## Merge Result

Compatibility reports must include `extension_id`, `ruleset_version`, `tool_schema_version`, and `compatibility_result`; incompatible reports are rejected.

## Boundary

This supplemental prompt owns no Rust src/test output.
