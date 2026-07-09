# BATCH-044 Acceptance Report

Batch: `BATCH-044-12-extension-sdk`
Stage: `S12`
Status: accepted for BATCH-044 Extension SDK primary scope, with Node/UI checks recorded as not applicable in this workspace state.

## Acceptance Checklist

- Required bootstrap, source guide, top-level design, normalized map, current-safe map, token rewrite table, stage files, test data, fixtures, and B044 prompts were read before implementation.
- Scope stayed within BATCH-044 implementation, tests, supplemental records, traceability records, and batch evidence.
- `source-archive/**` was not used as a current naming source.
- Eight normalized primary prompts were implemented:
  - `CODEX-0103-12-EXTENSION-SDK-1322493559`
  - `CODEX-0105-12-EXTENSION-SDK-914ad16fbe`
  - `CODEX-0106-12-EXTENSION-SDK-34e4277c8c`
  - `CODEX-0107-12-EXTENSION-SDK-18948e0a9e`
  - `CODEX-0946-12-EXTENSION-SDK-f6fbec755d`
  - `CODEX-0947-12-EXTENSION-SDK-f162f72984`
  - `CODEX-0953-12-EXTENSION-SDK-7588c965bd`
  - `CODEX-0957-12-EXTENSION-SDK-2c33b70efe`
- Supplemental prompts were recorded as supplemental requirements and did not own Rust output.
- Documentation-or-traceability prompts were limited to Markdown records.
- Extension SDK writes continue through governed command envelopes and Event Store append helpers.
- Plugins and extensions cannot request direct LLM, database write, Event Store append, internal Tool Gate bypass, Authority Contract mutation, forged dice, or restricted visibility disclosure.
- Tool provider and plugin paths require Tool Grant, OpenFGA, OPA, and Audit gates.
- Visibility redaction covers `private_to_player`, `ai_internal`, and `keeper_only` replay/export behavior.
- Compatibility reports require `extension_id`, `ruleset_version`, `tool_schema_version`, and `compatibility_result`.

## Verification

See `evidence/batches/BATCH-044/test-output.txt`.

Required Rust and S12 SDK checks passed:

- `cargo fmt --all -- --check`
- `cargo check -p trpg-extension-sdk --all-features`
- `cargo test -p trpg-extension-sdk --test plugin_sdk_contract_tests --test extension_compatibility_matrix_contract_tests --test s12_fixture_acceptance_contract_tests`
- `cargo test -p trpg-extension-sdk --all-features`
- `cargo test -p trpg-extension-sdk --test extension_compatibility_matrix`
- `cargo clippy -p trpg-extension-sdk --all-targets --all-features -- -D warnings`
- `cargo check --workspace --all-features`

## Residual Notes

- The external batch fact says recognized primary prompt count is `0`, while `batches/B044.md` and the normalized/current-safe maps identify eight primary prompts. This batch follows the higher-priority normalized mapping and keeps the mismatch as metadata cleanup.
- `pnpm test --if-present` and `pnpm build --if-present` are not applicable until a Node package manifest exists. The current workspace has no `package.json` or `pnpm-lock.yaml`.
- BATCH-044 did not start BATCH-045 or implement UI role surfaces beyond SDK fixture checks.
