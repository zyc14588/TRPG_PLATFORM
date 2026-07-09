# Supplemental Requirement: Tool Provider Model Boundary

Prompt ID: `CODEX-0958-12-EXTENSION-SDK-91b022044e`
Primary Prompt: `CODEX-0107-12-EXTENSION-SDK-18948e0a9e`
Current-safe module: `extension_sdk::tool_provider_sdk`

## Merge Result

Tool providers cannot become direct model providers. Direct LLM access remains a forbidden extension capability and must route through Agent Gateway and governed runtime boundaries.

## Boundary

This supplemental prompt owns no Rust src/test output.
