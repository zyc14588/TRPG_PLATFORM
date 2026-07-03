# Foundation Shared Kernel Trace

Batch: `BATCH-003-01-foundation`
Stage: `S01`

This document records the current-safe foundation target for the first shared
kernel implementation batch. Historical source paths, old version labels, and
hash fragments remain provenance only and are not used as Rust module names,
test names, event names, metric labels, migrations, NATS subjects, or workflow
names.

## Current Implementation Targets

- `shared_kernel::cargo_workspace`
- `shared_kernel::config_model`
- `shared_kernel::crate_ownership`
- `shared_kernel::dependency_direction`
- `shared_kernel::error_model`
- `shared_kernel::rust_coding_model`
- `shared_kernel::shared_kernel`
- `shared_kernel::rust_cargo_workspace`
- `shared_kernel::open_source_reference_matrix`

## Governance Boundary

- Business, KP service, rules engine, frontend, and this shared kernel do not
  call model providers directly.
- Agent output can only be represented as proposal/tool-result provenance; it
  cannot directly append formal state.
- Formal facts are appended through typed command envelopes and an event-store
  contract that carries idempotency, expected version, actor, correlation,
  causation, visibility, and fact provenance.
- Authority mode is campaign-scoped and immutable for a contract version; a
  mode change requires a forked contract version.
