# Current documentation template

> BATCH-051 current-safe documentation output. This template is a governance
> aid only and cannot override the repository authority order.

## Current-safe metadata

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1072-99-APPENDIX-b9f2731490` | `codex-prompts/99-appendix/P0001.md` | `trpg-docs-governance` | `appendix_research::document_template` | `docs/codex/99-appendix/document_template.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-document-template-63097f74aa.v5-code-ready.md` | `f5e81f2d14c22ca703c1c6a1f0dba8903cd7d757b4878a4806755d53839a3e98` |
| `CODEX-1089-99-APPENDIX-668b316c7b` | `codex-prompts/99-appendix/P0018.md` | `trpg-docs-governance` | `appendix_research::document_template` | `docs/codex/99-appendix/document_template.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-document-template-cd55a430c7.v5-code-ready.md` | `0905942302ed080070705b5a29cf3f02f0ddba9f4a89d4b5717b436d1e40a932` |

## Allowed boundary

This artifact owns Markdown structure and traceability only. It does not own
Rust source or tests, migrations, handlers, schemas, subjects, metrics,
workflows, provider calls, or formal state writes. Historical paths and hashes
in the metadata table are provenance, not current authority or engineering
names.

## Required document sections

1. Purpose and current authority source.
2. Scope, owner, and explicit non-goals.
3. Current-safe Prompt ID, crate, module, and output mapping.
4. Requirements and retained governance invariants.
5. Test commands, observed results, and evidence paths.
6. Risks, unresolved questions, and handoff boundary.

Every claim must distinguish current requirements from provenance. A document
must not report an unrun command as PASS or promote a source hash, historical
path, or prior report to current authority.

## Governance red lines

- Authority Contract is immutable and fork-only; HUMAN_KP and AI_KP are
  campaign-level mutually exclusive.
- AI routes through Agent Gateway, Orchestrator/Runtime, and Provider Adapter;
  formal state uses tools, rules, state services, and the Event Store.
- Visibility Label and Fact Provenance propagate through derived and visible
  outputs.
- Tool Permission and Policy Gate checks remain default-deny and auditable.

## BATCH-051 disposition and test responsibility

The two BATCH-051 inputs are merged into this single current-safe artifact.
B051 checks both Prompt IDs, provenance metadata, current-safe map agreement,
Markdown structure, shared-target merging, and the documentation-only
boundary. No executable implementation test is owned by these prompts.

## Minimal usable Markdown template

```markdown
# <Document title>

> Status: <draft|accepted|superseded>
> Owner: <team or role>
> Scope: <one-sentence boundary>
> Authority: <canonical source>

## Purpose

<What this document decides or records.>

## Requirements or decisions

- <Requirement or decision>

## Validation

| Check | Method | Evidence | Status |
|---|---|---|---|
| <check> | <method> | <path or record> | <PASS|FAIL|BLOCKED> |

## Open questions

- <Question, owner, and follow-up condition>
```
