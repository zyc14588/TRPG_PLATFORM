# BATCH-037 Evidence

## Inputs Read

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B037.md`
- `stages/s04-security-governance-policy/README.md`
- `stages/s04-security-governance-policy/START_PROMPT.md`
- `stages/s04-security-governance-policy/TEST_PLAN.md`
- `stages/s04-security-governance-policy/TEST_DATA.md`
- `stages/s04-security-governance-policy/ACCEPTANCE_PROMPT.md`
- `stages/s04-security-governance-policy/REPAIR_PROMPT.md`
- `codex-prompts/09-security-governance/P0048.md`
- `codex-prompts/09-security-governance/P0049.md`
- `codex-prompts/09-security-governance/P0051.md`
- `codex-prompts/09-security-governance/P0054.md`

## Changes

- Updated `codex-prompts/09-security-governance/P0048.md` with B037 README/module governance merge instructions.
- Updated `codex-prompts/09-security-governance/P0049.md` with B037 OpenFGA/OPA/Policy Gate merge instructions.
- Updated `codex-prompts/09-security-governance/P0051.md` with B037 security/privacy/copyright merge instructions.
- Added `docs/codex/09-security-governance/strict_rework_audit.md` as the documentation/traceability output for `CODEX-0838-09-SECURITY-GOVERNANCE-a6f388563f`.
- Added `evidence/batches/BATCH-037/BATCH_WORK_PLAN.md` and this evidence file.

## Command Results

| Command | Result | Evidence |
|---|---|---|
| `rg -n "CODEX-0835|CODEX-0836|CODEX-0837|CODEX-0838" docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` | PASS | 8 current-safe mapping rows found for the 4 prompt IDs across both maps. |
| `rg -n "crates/trpg-security-governance/(src|tests)" codex-prompts/09-security-governance/P0048.md codex-prompts/09-security-governance/P0049.md codex-prompts/09-security-governance/P0051.md docs/codex/09-security-governance/strict_rework_audit.md` | PASS | Exit code 1 with no matches; B037 outputs do not claim concrete Rust source/test ownership. |
| `rg -n "B037 final merge instruction|Strict Rework Audit|BATCH-037" ...` | PASS | B037 sections found in P0048/P0049/P0051, strict audit, and evidence files. |
| `cargo test -p trpg-security-governance --all-features` | PASS | 14 passed, 0 failed. |
| `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features` | PASS | 2 passed, 0 failed. |
| `opa test policy/opa` | PASS | 11/11 passed. |
| `git diff --check` | PASS | No whitespace errors; Git reported CRLF normalization warnings for the three modified prompt files. |

## Acceptance Notes

- B037 declared 4 prompts and 0 primary prompts; implementation changes were limited to supplemental Markdown, traceability Markdown, and batch evidence.
- No Rust source, Rust tests, migrations, API handlers, NATS subjects, metrics, workflows, or event schemas were created.
- `source-archive/**` was not used as an executable prompt source.
- No unresolved P0/P1 batch risk remains inside the B037 documentation scope.
