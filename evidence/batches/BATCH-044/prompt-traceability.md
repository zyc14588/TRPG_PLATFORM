# BATCH-044 Prompt Traceability

Declared prompt count: `25`
Primary prompts executed: `8`
Docs-governance prompts recorded: `2`
Supplemental prompts recorded: `10`
Traceability prompts recorded: `5`

## Execution Boundary

- Primary implementation was limited to `trpg-extension-sdk` and B044 primary modules.
- Supplemental prompts were recorded under `docs/codex/90-traceability/supplemental-requirements/` and did not own Rust output.
- Documentation-or-traceability prompts were limited to Markdown records.
- `source-archive/**` and historic source paths were treated as provenance only.
- Current Rust module, event, test, metric, and output names use normalized current-safe names.

## Primary Evidence

| Prompt ID | Module | Test |
|---|---|---|
| `CODEX-0103-12-EXTENSION-SDK-1322493559` | `agent_pack_sdk` | `agent_pack_sdk_contract_tests` |
| `CODEX-0105-12-EXTENSION-SDK-914ad16fbe` | `plugin_sdk` | `plugin_sdk_contract_tests` |
| `CODEX-0106-12-EXTENSION-SDK-34e4277c8c` | `ruleset_pack_sdk` | `ruleset_pack_sdk_contract_tests` |
| `CODEX-0107-12-EXTENSION-SDK-18948e0a9e` | `tool_provider_sdk` | `tool_provider_sdk_contract_tests` |
| `CODEX-0946-12-EXTENSION-SDK-f6fbec755d` | `adr_0008_plugin_boundaries` | `adr_0008_plugin_boundaries_contract_tests` |
| `CODEX-0947-12-EXTENSION-SDK-f162f72984` | `extension_compatibility_matrix` | `extension_compatibility_matrix_contract_tests` |
| `CODEX-0953-12-EXTENSION-SDK-7588c965bd` | `sdk` | `sdk_contract_tests` |
| `CODEX-0957-12-EXTENSION-SDK-2c33b70efe` | `readme` | `readme_contract_tests` |

## Metadata Risk

The external batch fact states primary count `0`. The batch file and normalized maps state primary count `8`. This evidence follows the higher-priority normalized maps and leaves the upstream count mismatch for later metadata cleanup.
