# Strict Implementation Documentation Template

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| `CODEX-1078-99-APPENDIX-f1cfd42c3c` | `codex-prompts/99-appendix/P0009.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-generated-from-source-strict-docs-implementation-99-appendix-implementation-do-b3b83a7e16.v5-code-ready.md` | `cb52f6fa66c7fb0b28d97768bbc9e1fb200e39414a923b687c6ccbe3c5d33b00` | `trpg-docs-governance` | `appendix_research::docs_implementation_99_appendix_implementation_doc_template_strict` | `docs/codex/99-appendix/docs_implementation_99_appendix_implementation_doc_template_strict.md` |

The source path and hash are provenance only. The current-safe module and output in this table resolve conflicting historical wording.

## Allowed boundary

- Output role: `documentation-or-traceability`.
- This artifact defines a Markdown structure for documenting already-authorized implementation work and its evidence.
- It cannot authorize work, expand batch scope, create implementation artifacts, or replace stage acceptance.
- Concrete code and runtime outputs remain owned only by their authorized primary implementation prompts.

## Governance red lines

- Authority Contracts are immutable after creation; changes require a campaign fork.
- `HUMAN_KP` and `AI_KP` are mutually exclusive campaign authority modes.
- Business services and clients never call model providers directly; AI use follows the governed Agent Gateway and runtime.
- AI cannot write canonical state, fabricate dice, bypass rules/state/policy/audit controls, or disclose restricted visibility.
- Formal state changes follow `Command -> Workflow -> Decision -> Event Store -> Projection`; the Event Store is canonical.
- Visibility labels and fact provenance propagate through APIs, events, agent context, tools, retrieval, summaries, exports, replay, logs, and metrics.

## BATCH-051 disposition and test responsibility

- Disposition: materialize the P0009 current-safe documentation template without importing its historical implementation proposals.
- Required checks: metadata matches both current-safe maps, Markdown is structurally valid, governance red lines remain intact, and the template does not claim implementation ownership.
- Test responsibility is documentation validation only; project tests belong to the primary prompt that owns the documented change.

## Minimal usable Markdown template

```markdown
# <Implementation change title>

> Status: <planned|in-progress|verified|blocked>
> Owner: <team or role>
> Authorized scope: <stage, batch, and primary prompt>
> Canonical inputs: <current design and current-safe mapping>

## Outcome

<Observable result of the authorized change.>

## Scope and non-goals

- In scope: <item>
- Out of scope: <item>

## Current-safe mapping

| Prompt ID | Module or document | Output |
|---|---|---|
| <ID> | <current-safe name> | <path> |

## Governance impact

| Invariant | Impact | Evidence |
|---|---|---|
| <invariant> | <unchanged or explained change> | <path or record> |

## Verification

| Check | Method | Evidence | Status |
|---|---|---|---|
| <check> | <command or review> | <path or result> | <PASS|FAIL|BLOCKED> |

## Risks and handoff

- <Remaining risk, owner, and next authorized action>
```
