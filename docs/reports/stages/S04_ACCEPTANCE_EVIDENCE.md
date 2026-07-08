# S04 Acceptance Evidence

Stage: `S04-security-governance-policy`
Date: `2026-07-09`
Result: `PASS`

## Scope

This report consolidates:

- `evidence/batches/BATCH-035/ACCEPTANCE.md`
- `evidence/batches/BATCH-035/TEST_RESULTS.md`
- `evidence/batches/BATCH-036/ACCEPTANCE.md`
- `evidence/batches/BATCH-036/TEST_RESULTS.md`
- Current rerun output from the S04 validation pass
- S04 fixtures:
  - `fixtures/security/permission_matrix.v1.json.md`
  - `fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md`
  - `fixtures/visibility/visibility_redaction_matrix.v1.json.md`

## Acceptance Matrix

| Check | Result | Evidence | Command / fixture / prompt coverage |
|---|---:|---|---|
| Current-safe module/output names | PASS | New OpenFGA artifact is `policy/openfga/security_governance.fga`; Rust module remains `security_governance::policy_openfga_opa` | `CODEX-0085-09-SECURITY-GOVERNANCE-4517fccc2d`; `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`; `rg -n "V3\|V4\|V5\|V6\|v3\|v4\|v5\|v6" policy/openfga crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs` found only the existing negative assertion `!module.contains("v6")` |
| CODEX-0085 OpenFGA relation model | PASS | `policy/openfga/security_governance.fga` now declares `campaign` role relations and permission relations | `openfga_security_governance_model_matches_permission_matrix_fixture`; `fixtures/security/permission_matrix.v1.json.md` |
| Permission matrix allow/deny alignment | PASS | ALLOW rows map to role relations; DENY rows map to `no_grant` and are asserted not to grant the denied role | `cargo test -p trpg-security-governance --all-features`; `CODEX-0085`, `CODEX-0800`, `CODEX-0805`, B036 supplemental `CODEX-0812`, `CODEX-0813`, `CODEX-0817`, `CODEX-0834` |
| Authority Contract immutable/fork-only | PASS | Existing domain tests reject in-place authority changes and direct-agent authority bypass | `cargo test -p trpg-domain-core authority_contract --all-features`; `CODEX-0086`, `CODEX-0815` |
| Agent Gateway-only AI/provider boundary | PASS | No new direct LLM/provider call path was introduced; provider names outside agent runtime are provider-boundary validation only | `rg -n "OpenAI\|Ollama\|llama\.cpp\|chat\.completions\|responses\.create\|/v1/chat\|/api/generate" crates --glob "!crates/trpg-agent-runtime/**"`; `cargo test -p trpg-agent-runtime provider_boundary_blocks_prod_exposure_and_silent_cloud_fallback --all-features` |
| Tool Permission Gate and fail-closed policy | PASS | Tool gate focused tests pass; OpenFGA/OPA policy wrapper denies if either decision denies | `cargo test -p trpg-agent-runtime tool_gate --all-features`; `policy_openfga_opa_fails_closed`; `opa test policy/opa` |
| Visibility label propagation | PASS | S04 visibility fixture cases are bound to Rust assertions and OPA policy tests | `visibility_enforcement_points_redacts_stage_cases`; `fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md`; `fixtures/visibility/visibility_redaction_matrix.v1.json.md`; `opa test policy/opa` |
| Fact provenance and event log boundary | PASS | Audit metadata persists visibility and fact provenance through event envelopes; direct agent formal writes are denied before event append | `audit_log_contract_persists_audit_metadata`; `security_privacy_rejects_direct_agent_write_path`; `cargo test -p trpg-domain-core openfga_opa_visibility --all-features` |
| Formal game state write path | PASS | Direct-agent state writes are represented only as negative tests or deny branches | `rg -n "FormalWritePath::DirectAgent\|DirectAgentStateWrite\|WriteOfficialState" crates policy fixtures`; `security_privacy_rejects_direct_agent_write_path` |
| Restricted fixture leakage | PASS | Player-visible checks are redaction/negative-assertion paths; no accepted player-visible output includes `keeper_only`, `private_to_player`, or `ai_internal` | `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features`; `rg -n "player_visible\|player-visible\|public.*keeper_only\|public.*private_to_player\|public.*ai_internal\|export.*ai_internal\|sync.*ai_internal" fixtures crates docs/reports evidence/batches/BATCH-035 evidence/batches/BATCH-036 --glob "*.rs" --glob "*.md" --glob "*.json"` |
| B036 supplemental boundary | PASS | B036 remains documentation/supplemental only: 13 supplemental rows, 12 traceability rows, 0 primary rows | `evidence/batches/BATCH-036/ACCEPTANCE.md`; B036 Prompt IDs `CODEX-0810` through `CODEX-0834` |
| Cargo checks | PASS | S04 Rust checks pass after the OpenFGA contract repair | See `docs/reports/stages/S04_TEST_RESULTS.md` |
| OPA checks | PASS | OPA policy suite remains green | `opa test policy/opa`: `PASS: 11/11` |
| pnpm checks | N/A | No `package.json`, `pnpm-lock.yaml`, or `pnpm-workspace.yaml` was present in this repository snapshot; S04 scope is Rust/OPA policy | `rg --files -g "package.json" -g "pnpm-lock.yaml" -g "pnpm-workspace.yaml"` returned no matches |
| docker checks | N/A | No `Dockerfile` or docker compose manifest was present in this repository snapshot; S04 did not touch deployment containers | `rg --files -g "Dockerfile" -g "docker-compose.yml" -g "docker-compose.yaml"` returned no matches |

## Repair Evidence

| File | Result | Prompt coverage | Fixture coverage |
|---|---:|---|---|
| `policy/openfga/security_governance.fga` | PASS | `CODEX-0085-09-SECURITY-GOVERNANCE-4517fccc2d`; B036 supplemental `CODEX-0813-09-SECURITY-GOVERNANCE-637c915ed6` | `fixtures/security/permission_matrix.v1.json.md` |
| `crates/trpg-security-governance/tests/batch_035_security_governance_contract_tests.rs` | PASS | Adds narrow contract coverage for `CODEX-0085` without changing B036 supplemental outputs | `permission_matrix.v1.json.md` |

## Residual Notes

- `cargo` emitted a non-failing Windows path canonicalization warning for `C:\Users\zyc14588`.
- `git diff --check` passed and emitted only a non-failing LF-to-CRLF warning for the edited Rust test file.
- No B036 supplemental Markdown output was modified during the implementation repair.
