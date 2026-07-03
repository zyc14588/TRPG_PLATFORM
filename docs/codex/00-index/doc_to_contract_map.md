# Document To Contract Map

> Prompt IDs: CODEX-0009-00-INDEX-ec6ba70aa0, CODEX-0118-00-INDEX-215c4c75fb
> Role: documentation-or-traceability
> Current module: docs_governance::doc_to_contract_map

This file maps governance documents to the contracts they protect.

| Document | Protected contract |
|---|---|
| `CURRENT_TOP_LEVEL_DESIGN.md` | V1 product scope, Authority Contract, Agent Gateway, Event Store, Visibility |
| `AGENTS.md` | Codex output boundary and repair discipline |
| `CURRENT_*_MAP.md` | Current-safe prompt execution, module, and output naming |
| `batches/B001.md` | Batch-local prompt scope |
| `batches/B002.md` | Batch-local prompt scope for 00-index strict governance final |

## Rules

- A document contract is a governance constraint, not an implementation artifact.
- Documentation prompts may record a contract but cannot implement service code.

## Batch-002 Contract Coverage

`CODEX-0138-00-INDEX-2ac2272143` keeps Batch-002 aligned with the top-level
design red lines: no business-layer direct LLM calls, no agent direct database
writes, no Authority Contract mutation, no visibility leakage, and no formal
adjudication outside the tool, rules, state, and event-log path.
