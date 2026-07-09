# Extension Compatibility Matrix Evidence

Prompt ID: `CODEX-0947-12-EXTENSION-SDK-f162f72984`
Module: `extension_sdk::extension_compatibility_matrix`

## Evidence

- Compatibility reports require `extension_id`, `ruleset_version`, `tool_schema_version`, and `compatibility_result`.
- The current matrix accepts `coc7_sample_extension` with `7e` and `tool_schema.v1`.
- Incompatible reports are rejected.

Tests:
- `cargo test -p trpg-extension-sdk --test extension_compatibility_matrix_contract_tests`
- `cargo test -p trpg-extension-sdk --test extension_compatibility_matrix`
