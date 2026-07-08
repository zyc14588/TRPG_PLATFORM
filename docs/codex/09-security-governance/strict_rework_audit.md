# Strict Rework Audit — Security Governance

> Batch: `BATCH-037-09-security-governance`
> Prompt ID: `CODEX-0838-09-SECURITY-GOVERNANCE-a6f388563f`
> Output role: `documentation-or-traceability`
> Current-safe module: `security_governance::strict_rework_audit`
> Current-safe output: `docs/codex/09-security-governance/strict_rework_audit.md`

## Scope

This document records the final traceability audit for the S04 security governance prompts in B037. It is Markdown-only evidence and does not create Rust source, Rust tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflows.

## Prompt Boundary Matrix

| Prompt ID | Role | Current-safe output | Allowed action |
|---|---|---|---|
| `CODEX-0835-09-SECURITY-GOVERNANCE-2f820bbc79` | supplemental-requirement | `codex-prompts/09-security-governance/P0049.md` | Merge OpenFGA/OPA/Policy Gate constraints into primary `CODEX-0085-09-SECURITY-GOVERNANCE-4517fccc2d`. |
| `CODEX-0836-09-SECURITY-GOVERNANCE-0d9e5f5ece` | supplemental-requirement | `codex-prompts/09-security-governance/P0048.md` | Merge module README and evidence responsibilities into primary `CODEX-0804-09-SECURITY-GOVERNANCE-f8e9581ea3`. |
| `CODEX-0837-09-SECURITY-GOVERNANCE-5fa1a7157d` | supplemental-requirement | `codex-prompts/09-security-governance/P0051.md` | Merge security/privacy/copyright constraints into primary `CODEX-0086-09-SECURITY-GOVERNANCE-bb407cb7fc`. |
| `CODEX-0838-09-SECURITY-GOVERNANCE-a6f388563f` | documentation-or-traceability | `docs/codex/09-security-governance/strict_rework_audit.md` | Maintain this audit and batch traceability evidence only. |

## Current Governance Checks

| Gate | Required current behavior | B037 disposition |
|---|---|---|
| Primary ownership | Only primary prompts create concrete Rust output. | No primary prompt in B037; Rust output is out of scope. |
| Supplemental boundary | Supplemental prompts record merge instructions only. | P0048/P0049/P0051 keep code-impacting work as primary handoff. |
| Traceability boundary | Documentation prompt maintains Markdown audit only. | P0054 owns this document. |
| Provenance boundary | Historical source paths and hashes remain provenance only. | No current module/output/test/event/metric name is derived from source paths or hashes. |
| Governance red lines | Authority, Event Store, Visibility, Fact Provenance, Policy Gate, provider privacy boundary stay mandatory. | Recorded as must-merge constraints for the relevant primary prompts. |

## Required Primary Handoff

- `security_governance::policy_openfga_opa` must enforce default-deny Policy Gate, OpenFGA relationship checks, OPA context checks, audit logging, authority-mode separation, and visibility/provenance propagation.
- `security_governance::readme` must document S04 boundaries, evidence responsibilities, supplemental limitations, and the separation between platform moderation and game ruling authority.
- `security_governance::security_privacy` must enforce production provider security, no silent cloud fallback, copyright metadata, data retention boundaries, and export/summary/RAG/debug-log redaction.

## Fixture Trace

| Fixture | Required use |
|---|---|
| `fixtures/stages/S04_stage_acceptance_fixture.v1.json.md` | Stage document and evidence expectations. |
| `fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md` | Visibility redaction events, records, and expected errors. |
| `fixtures/security/permission_matrix.v1.json.md` | Role/action allow-deny policy matrix. |
| `fixtures/visibility/visibility_redaction_matrix.v1.json.md` | Redaction matrix for export, summary, RAG, and agent context. |
| `test-data/provider_model_certification_cases.md` | Provider privacy boundary and no-silent-fallback cases. |

## Remaining Risk

- B037 has no primary prompt, so it does not add new executable assertions; it records primary handoff requirements and runs existing stage checks.
- Existing S04 stage checks passed in this run: `cargo test -p trpg-security-governance --all-features`, `cargo test -p trpg-domain-core --test visibility_leakage_tests_contract_tests --all-features`, and `opa test policy/opa`.
