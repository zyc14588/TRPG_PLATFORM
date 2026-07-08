# S04 Test Results

Stage: `S04-security-governance-policy`
Date: `2026-07-09`
Result: `PASS`

## Command Results

| Command | Result | Evidence / fixture / Prompt ID coverage |
|---|---:|---|
| `cargo fmt --all` | PASS | Formatted the new `CODEX-0085` contract test only |
| `cargo fmt --all -- --check` | PASS | Formatting remains stable; non-failing Windows canonicalization warning only |
| `cargo check -p trpg-security-governance --all-features` | PASS | S04 security governance crate compiles |
| `cargo clippy -p trpg-security-governance --all-targets --all-features -- -D warnings` | PASS | S04 security governance crate has no clippy warnings |
| `cargo test -p trpg-security-governance --all-features` | PASS | 14 integration tests passed; covers `CODEX-0083`, `CODEX-0085`, `CODEX-0086`, `CODEX-0087`, `CODEX-0793`, `CODEX-0794`, `CODEX-0795`, `CODEX-0798`, `CODEX-0800`, `CODEX-0801`, `CODEX-0802`, `CODEX-0804`, `CODEX-0805` |
| `opa test policy/opa` | PASS | 11/11 OPA tests passed; covers OpenFGA/OPA fail-closed and S04 visibility/permission deny cases |
| `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features` | PASS | 2/2 visibility leakage tests passed; covers no player-visible leakage |
| `cargo test -p trpg-domain-core openfga_opa_visibility --all-features` | PASS | 2 focused OpenFGA/OPA visibility tests passed |
| `cargo test -p trpg-domain-core authority_contract --all-features` | PASS | Authority Contract immutable/fork-only and direct-agent denial focused tests passed |
| `cargo test -p trpg-agent-runtime tool_gate --all-features` | PASS | Tool gate focused tests passed; Agent Runtime cannot bypass tool gate |
| `cargo test -p trpg-agent-runtime human_kp_draft_only --all-features` | PASS | HUMAN_KP draft-only boundary passed |
| `cargo test -p trpg-agent-runtime provider_boundary_blocks_prod_exposure_and_silent_cloud_fallback --all-features` | PASS | Provider boundary and no silent cloud fallback focused test passed |
| `cargo test -p trpg-agent-runtime model_provider_local_cloud_impl_blocks_silent_local_to_cloud_fallback --all-features` | PASS | Local-to-cloud silent fallback denial passed |
| `cargo test -p trpg-agent-runtime model_provider_local_cloud_impl_requires_level4_for_ai_keeper --all-features` | PASS | Local model Level 4 requirement for AI Keeper passed |
| `rg -n "OpenAI\|Ollama\|llama\.cpp\|chat\.completions\|responses\.create\|/v1/chat\|/api/generate" crates --glob "!crates/trpg-agent-runtime/**"` | PASS | Only non-runtime hit was `crates/trpg-platform/src/deployment_ops.rs`, a provider-boundary validator, not a direct LLM call path |
| `rg -n "FormalWritePath::DirectAgent\|DirectAgentStateWrite\|WriteOfficialState" crates policy fixtures` | PASS | Matches are deny branches and negative tests; no accepted formal-state bypass found |
| `rg -n "player_visible\|player-visible\|public.*keeper_only\|public.*private_to_player\|public.*ai_internal\|export.*ai_internal\|sync.*ai_internal" fixtures crates docs/reports evidence/batches/BATCH-035 evidence/batches/BATCH-036 --glob "*.rs" --glob "*.md" --glob "*.json"` | PASS | Matches are redaction fields, negative assertions, or fixture-deny expectations |
| `rg --files -g "package.json" -g "pnpm-lock.yaml" -g "pnpm-workspace.yaml"` | N/A | No pnpm/project frontend manifest exists in this repository snapshot |
| `rg --files -g "Dockerfile" -g "docker-compose.yml" -g "docker-compose.yaml"` | N/A | No docker manifest exists in this repository snapshot |
| `git diff --check` | PASS | Whitespace/conflict-marker check passed; non-failing LF-to-CRLF warning only |

## Fixture Results

| Fixture | Result | Covered by |
|---|---:|---|
| `fixtures/security/permission_matrix.v1.json.md` | PASS | `policy_authz_matches_permission_matrix_fixture`; `openfga_security_governance_model_matches_permission_matrix_fixture` |
| `fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md` | PASS | `visibility_enforcement_points_redacts_stage_cases`; `opa test policy/opa` |
| `fixtures/visibility/visibility_redaction_matrix.v1.json.md` | PASS | `visibility_enforcement_points_redacts_stage_cases`; domain visibility tests |

## Observed Security Governance Tests

| Test | Result | Prompt / fixture coverage |
|---|---:|---|
| `openfga_security_governance_model_matches_permission_matrix_fixture` | PASS | `CODEX-0085`; `permission_matrix.v1.json.md`; `policy/openfga/security_governance.fga` |
| `policy_openfga_opa_fails_closed` | PASS | `CODEX-0085`, `CODEX-0793`; OpenFGA/OPA deny boundary |
| `policy_authz_matches_permission_matrix_fixture` | PASS | `CODEX-0800`, `CODEX-0805`; `permission_matrix.v1.json.md` |
| `visibility_enforcement_points_redacts_stage_cases` | PASS | `CODEX-0087`; S04 visibility/error fixtures |
| `security_privacy_rejects_direct_agent_write_path` | PASS | `CODEX-0086`; formal state write denial |
| `audit_log_contract_persists_audit_metadata` | PASS | `CODEX-0794`; visibility and fact provenance on event log |
| `privacy_copyright_blocks_ai_internal_export` | PASS | `CODEX-0802`; AI internal export denial |
| `security_privacy_copyright_denies_prod_placeholder_provider` | PASS | `CODEX-0798`; provider boundary |
| `permission_matrix_covers_provider_certification_and_fallback` | PASS | `CODEX-0805`; local model Level 4 and fallback rules |
