# BATCH-044 Work Plan

Batch: `BATCH-044-12-extension-sdk`
Stage: `S12`
Scope: current batch only, `trpg-extension-sdk` SDK governance boundary plus supplemental and traceability records.

## Metadata Note

The user-provided batch fact says the recognized primary prompt count is `0`. The authoritative normalized/current-safe maps and `batches/B044.md` identify eight primary rows in this batch. Execution follows the normalized current-safe mapping, matching the BATCH-043 precedent, and records the mismatch as an upstream metadata risk.

## Prompt Map

| Prompt ID | Prompt file | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0103-12-EXTENSION-SDK-1322493559` | `codex-prompts/12-extension-sdk/P0001.md` | primary | `crates/trpg-extension-sdk/src/agent_pack_sdk.rs` | Rust module + contract test | `agent_pack_sdk_contract_tests` |
| `CODEX-0104-12-EXTENSION-SDK-1523d32137` | `codex-prompts/12-extension-sdk/P0002.md` | docs-governance | `docs/codex/12-extension-sdk/extension_compatibility_matrix.md` | Markdown governance matrix | Covered by compatibility matrix tests |
| `CODEX-0105-12-EXTENSION-SDK-914ad16fbe` | `codex-prompts/12-extension-sdk/P0003.md` | primary | `crates/trpg-extension-sdk/src/plugin_sdk.rs` | Rust module + contract test | `plugin_sdk_contract_tests` |
| `CODEX-0102-12-EXTENSION-SDK-77b0c49963` | `codex-prompts/12-extension-sdk/P0004.md` | docs-governance | `docs/codex/12-extension-sdk/m_12_extension_sdk.md` | Markdown module overview | Traceability review |
| `CODEX-0106-12-EXTENSION-SDK-34e4277c8c` | `codex-prompts/12-extension-sdk/P0005.md` | primary | `crates/trpg-extension-sdk/src/ruleset_pack_sdk.rs` | Rust module + contract test | `ruleset_pack_sdk_contract_tests` |
| `CODEX-0107-12-EXTENSION-SDK-18948e0a9e` | `codex-prompts/12-extension-sdk/P0006.md` | primary | `crates/trpg-extension-sdk/src/tool_provider_sdk.rs` | Rust module + contract test | `tool_provider_sdk_contract_tests` |
| `CODEX-0946-12-EXTENSION-SDK-f6fbec755d` | `codex-prompts/12-extension-sdk/P0007.md` | primary | `crates/trpg-extension-sdk/src/adr_0008_plugin_boundaries.rs` | Rust module + contract test | `adr_0008_plugin_boundaries_contract_tests` |
| `CODEX-0947-12-EXTENSION-SDK-f162f72984` | `codex-prompts/12-extension-sdk/P0008.md` | primary | `crates/trpg-extension-sdk/src/extension_compatibility_matrix.rs` | Rust module + contract test | `extension_compatibility_matrix_contract_tests`, `extension_compatibility_matrix` |
| `CODEX-0949-12-EXTENSION-SDK-1ce074b16d` | `codex-prompts/12-extension-sdk/P0009.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0949-12-EXTENSION-SDK-1ce074b16d.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0948-12-EXTENSION-SDK-caaf0a2d22` | `codex-prompts/12-extension-sdk/P0010.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0948-12-EXTENSION-SDK-caaf0a2d22.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0950-12-EXTENSION-SDK-b7ded3674b` | `codex-prompts/12-extension-sdk/P0011.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0950-12-EXTENSION-SDK-b7ded3674b.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0953-12-EXTENSION-SDK-7588c965bd` | `codex-prompts/12-extension-sdk/P0012.md` | primary | `crates/trpg-extension-sdk/src/sdk.rs` | Rust module + contract test | `sdk_contract_tests` |
| `CODEX-0954-12-EXTENSION-SDK-78491d1920` | `codex-prompts/12-extension-sdk/P0013.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0954-12-EXTENSION-SDK-78491d1920.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0951-12-EXTENSION-SDK-d8ce614fff` | `codex-prompts/12-extension-sdk/P0014.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0951-12-EXTENSION-SDK-d8ce614fff.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0952-12-EXTENSION-SDK-deb0e52024` | `codex-prompts/12-extension-sdk/P0015.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0952-12-EXTENSION-SDK-deb0e52024.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0955-12-EXTENSION-SDK-229403a365` | `codex-prompts/12-extension-sdk/P0016.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0955-12-EXTENSION-SDK-229403a365.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0956-12-EXTENSION-SDK-3843f407c8` | `codex-prompts/12-extension-sdk/P0017.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0956-12-EXTENSION-SDK-3843f407c8.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0957-12-EXTENSION-SDK-2c33b70efe` | `codex-prompts/12-extension-sdk/P0018.md` | primary | `crates/trpg-extension-sdk/src/readme.rs` | Rust shared module + contract test | `readme_contract_tests` |
| `CODEX-0958-12-EXTENSION-SDK-91b022044e` | `codex-prompts/12-extension-sdk/P0019.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0958-12-EXTENSION-SDK-91b022044e.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0959-12-EXTENSION-SDK-b434f7374b` | `codex-prompts/12-extension-sdk/P0020.md` | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0959-12-EXTENSION-SDK-b434f7374b.md` | Supplemental record only | Prompt traceability review |
| `CODEX-0960-12-EXTENSION-SDK-df830c1c96` | `codex-prompts/12-extension-sdk/P0021.md` | traceability | `docs/codex/12-extension-sdk/source_processing_record_docs_adr_adr_0008_plugin_boundaries.md` | Markdown record only | Prompt traceability review |
| `CODEX-0963-12-EXTENSION-SDK-267c30d804` | `codex-prompts/12-extension-sdk/P0022.md` | traceability | `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_12_extension_sdk_readme.md` | Markdown record only | Prompt traceability review |
| `CODEX-0962-12-EXTENSION-SDK-2b751bd7fa` | `codex-prompts/12-extension-sdk/P0023.md` | traceability | `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_12_extension_sdk_plugin_sdk.md` | Markdown record only | Prompt traceability review |
| `CODEX-0961-12-EXTENSION-SDK-3029ffee76` | `codex-prompts/12-extension-sdk/P0024.md` | traceability | `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_12_extension_sdk_extension_compatibility_matrix.md` | Markdown record only | Prompt traceability review |
| `CODEX-0964-12-EXTENSION-SDK-4c69cb7cf1` | `codex-prompts/12-extension-sdk/P0025.md` | traceability | `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_12_extension_sdk_tool_provider_sdk.md` | Markdown record only | Prompt traceability review |

## Checks

Minimum related checks:
- `cargo check -p trpg-extension-sdk --all-features`
- `cargo test -p trpg-extension-sdk --test plugin_sdk_contract_tests --test extension_compatibility_matrix_contract_tests --test s12_fixture_acceptance_contract_tests`

Stage checks:
- `cargo fmt --all -- --check`
- `cargo test -p trpg-extension-sdk --all-features`
- `cargo test -p trpg-extension-sdk --test extension_compatibility_matrix`
- `pnpm test --if-present`
- `pnpm build --if-present`
