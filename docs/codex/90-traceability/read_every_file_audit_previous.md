# read every file audit previous

> BATCH-050 current-safe traceability output. This page preserves a previous
> input traversal audit as provenance only; it is not a current inventory,
> manifest, acceptance result, or product implementation owner.

## Current-safe target

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1096-90-TRACEABILITY-7c9e6f6016` | `codex-prompts/90-traceability/P0107.md` | `trpg-docs-governance` | `traceability::input_traversal_audit_previous_provenance` | `docs/codex/90-traceability/read_every_file_audit_previous.md` | `docs/implementation/90-traceability/read-every-file-audit-v5.md` | `99b763f4edbe748fe243e3d8cf7d1993204e9a6fdbad9c758ed88f36539249be` |

## Allowed change boundary

- Maintain Markdown provenance, indexes, matrices, reports, validation notes,
  and batch evidence only.
- Do not create or modify Rust source or tests, migrations, API handlers,
  event schemas, NATS subjects, metrics, workflows, provider adapters, or
  formal state-write paths from this prompt.
- Historical version labels, source paths, counts, and hashes remain
  provenance only. Current execution names come from the normalized maps.

## Governance invariants retained

- Authority Contract remains immutable and fork-only; HUMAN_KP and AI_KP
  remain campaign-level mutually exclusive modes.
- AI capabilities route through Agent Gateway, Agent Orchestrator/Runtime, and
  Model Provider Adapter; AI does not write formal state directly.
- Formal decisions pass tools, rules, state services, and the event log through
  `Command -> Workflow -> Decision -> Event Store -> Projection`.
- Visibility Label and Fact Provenance remain mandatory across every derived
  read model and user-visible output.

## Batch disposition and test responsibility

- Disposition: retain the source audit as previous provenance without copying
  its historical file list or treating it as current acceptance evidence.
- BATCH-050 must verify the Prompt ID, source SHA, current-safe module/output,
  map agreement, Markdown structure, and docs-only boundary.
- S00 governance responsibility is checked by
  `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-governance-boundary.ps1`.
