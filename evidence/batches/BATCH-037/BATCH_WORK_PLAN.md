# BATCH-037 Work Plan

## Scope

Batch: `BATCH-037-09-security-governance — Strict Governance Final`
Stage: `S04 — Security Governance`
Prompt count: 4
Primary prompt count: 0

Only current-safe Markdown and traceability outputs are in scope. Rust source, Rust tests, migrations, API handlers, NATS subjects, metrics, workflows, and event schemas are out of scope for this batch.

## Prompt Map

| Prompt ID | Target file | Allowed changes | Test responsibility |
|---|---|---|---|
| `CODEX-0835-09-SECURITY-GOVERNANCE-2f820bbc79` | `codex-prompts/09-security-governance/P0049.md` | Supplemental OpenFGA/OPA/Policy Gate merge instructions only. | Verify no Rust output ownership; list primary test assertions. |
| `CODEX-0836-09-SECURITY-GOVERNANCE-0d9e5f5ece` | `codex-prompts/09-security-governance/P0048.md` | Supplemental README/module governance merge instructions only. | Verify evidence duties and stage command recording. |
| `CODEX-0837-09-SECURITY-GOVERNANCE-5fa1a7157d` | `codex-prompts/09-security-governance/P0051.md` | Supplemental security/privacy/copyright merge instructions only. | Verify provider boundary, copyright, and redaction assertions are handed off. |
| `CODEX-0838-09-SECURITY-GOVERNANCE-a6f388563f` | `docs/codex/09-security-governance/strict_rework_audit.md` | Documentation/traceability audit only. | Verify prompt boundary, current-safe outputs, fixture trace, and remaining risks. |

## Checks Planned

- Minimal related checks:
  - `rg -n "CODEX-0835|CODEX-0836|CODEX-0837|CODEX-0838" docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
  - Supplemental boundary grep for Rust output ownership.
  - Current-safe audit document existence and prompt trace.
- Stage checks:
  - `cargo test -p trpg-security-governance --all-features`
  - `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features`
  - `opa test policy/opa`

## Non-Expansion Statement

This batch does not start BATCH-038 or any later testing-quality work. Any executable assertion gaps are handed off to the S04 primary prompts or later explicit test batches.
