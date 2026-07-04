# Supplemental Requirement Merge

- Prompt ID: `CODEX-0417-03-RUNTIME-ORCHESTRATION-5294d2dac2`
- Prompt file: `codex-prompts/03-runtime-orchestration/P0092.md`
- Primary Prompt: `CODEX-0335-03-RUNTIME-ORCHESTRATION-0ca4a1c995`
- Current module: `runtime_orchestration::adr_0007_internal_workflow_vs_temporal`
- Status: pending merge by primary prompt; no BATCH-015 Rust output claimed.

Merge instructions for the primary prompt:

- Keep internal workflow governance as the V1 runtime authority; any future Temporal-style integration must remain an adapter behind the current-safe workflow boundary.
- Preserve Authority Contract immutability, KP-mode exclusivity, and fork-only changes.
- Preserve Command -> Workflow -> Decision -> Event Store -> Projection for every formal state change.
- Reject direct database writes, naked provider calls, and agent writes outside the Agent Gateway / Runtime / Provider Adapter boundary.

Suggested test assertions for the primary prompt:

- Authority mismatch or attempted contract mutation is rejected without appending events.
- Workflow decisions append canonical events before projections or realtime outputs are observed.
- Visibility Label and Fact Provenance survive replay and exported audit records.

This supplemental prompt remains traceability-only and does not declare a Rust source, test, migration, API handler, event schema, NATS subject, metric, workflow, or formal write path.
