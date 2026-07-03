# BATCH-003 Handoff

Batch `BATCH-003-01-foundation` completed with PASS evidence.

## Ready for Next Batch

- A root Cargo workspace exists with one member: `trpg-shared-kernel`.
- The shared-kernel crate exposes typed IDs, stable error codes, visibility
  labels, provenance, authority-mode validation, command envelopes, and an
  event-store append/replay contract.
- B003 primary modules and tests are in place and should be reused by later
  S01 batches instead of re-created.

## Do Not Reopen in Later Batches Without a Primary Prompt

- `shared_kernel::cargo_workspace`
- `shared_kernel::config_model`
- `shared_kernel::crate_ownership`
- `shared_kernel::dependency_direction`
- `shared_kernel::error_model`
- `shared_kernel::rust_coding_model`
- `shared_kernel::shared_kernel`
- `shared_kernel::rust_cargo_workspace`
- `shared_kernel::open_source_reference_matrix`

## Open Risks for Later Authorized Work

- No SQLx migrations, Axum handlers, OpenAPI documents, NATS consumers, or
  provider adapters were created in this batch.
- Later primary prompts must integrate this contract layer with real workflow,
  rule, state, event-store, projection, and provider-adapter crates without
  bypassing Agent Gateway or visibility/provenance propagation.
- The worktree contained unrelated pre-existing S00/B002 and docs changes
  before this batch started; they were not reverted or normalized here.

