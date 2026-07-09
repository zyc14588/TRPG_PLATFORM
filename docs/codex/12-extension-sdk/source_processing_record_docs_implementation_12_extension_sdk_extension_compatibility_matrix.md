# Source Processing Record: Extension SDK Compatibility Matrix

Prompt ID: `CODEX-0961-12-EXTENSION-SDK-3029ffee76`
Prompt file: `codex-prompts/12-extension-sdk/P0024.md`
Current-safe output: `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_12_extension_sdk_extension_compatibility_matrix.md`

## Boundary

- Documentation-or-traceability output only.
- No Rust module, migration, event schema, metric, workflow, or test name is created by this prompt.
- Implementation ownership remains with `CODEX-0947-12-EXTENSION-SDK-f162f72984`.

## Covered Current Module

`extension_sdk::extension_compatibility_matrix` validates `extension_id`, `ruleset_version`, `tool_schema_version`, and `compatibility_result` before recording compatibility evidence.
