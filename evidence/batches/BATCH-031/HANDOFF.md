# BATCH-031 Handoff

## Changed Outputs

- Workspace: `Cargo.toml`, `Cargo.lock`
- New crate: `crates/trpg-platform`
- Evidence: `evidence/batches/BATCH-031`

## Next Batch Guidance

- Continue from the current-safe `trpg-platform` module names; do not introduce source-archive or previous version-token naming.
- Later S09 batches should provide runnable Docker Compose, object storage service wiring, admin health endpoint, and deployment smoke checks.
- Keep provider calls behind `Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter`; do not add direct model-provider clients to `trpg-platform`.
- Extend the current event-store-backed contracts instead of replacing them with projection/cache state.

## Known Risks

- B031 implemented contract-level platform infrastructure, not real Docker/object-storage/admin-health runtime wiring.
- `cargo test --workspace --all-features` can fail on Windows when test linking runs in parallel; `-j1` passed.
- Stage S09 Docker/healthz checks remain unexecuted until runnable compose/API artifacts exist in later batches.
