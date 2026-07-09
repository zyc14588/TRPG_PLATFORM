# Supplemental Requirement: Tool Provider SDK Boundary

Prompt ID: `CODEX-0971-12-EXTENSION-SDK-f4f12f9055`
Primary Prompt: `CODEX-0107-12-EXTENSION-SDK-18948e0a9e`
Current-safe module: `extension_sdk::tool_provider_sdk`

## Merge Result

P0: Tool providers expose governed tools only. They cannot become model provider adapters, call LLMs directly, write official state, or bypass Agent Gateway and Tool Permission Gate.

P1: Tool provider compatibility evidence should stay tied to SDK compatibility reports and redacted UI snapshots for S12 acceptance.

## Boundary

This supplemental prompt owns no Rust src/test output. Implementation changes, if needed later, belong to the primary prompt above.

