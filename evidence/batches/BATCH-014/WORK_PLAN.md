# BATCH-014 Work Plan

Batch: `BATCH-014-03-runtime-orchestration`
Stage: `S06-runtime-orchestration-decision-pipeline`
Declared prompt count: `25`
Effective primary prompt count: `8`

This repair run treats `P0052` and `P0061` through `P0067` as primary implementation prompts. The previous `primary count = 0` interpretation is not used.

## Scope

Allowed work:

- Implement the 8 current-safe Rust `src/` outputs declared by `batches/B014.md` and `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`.
- Add the 8 corresponding current-safe contract test files.
- Keep supplemental rows as Markdown-only supplemental requirement trace files.
- Keep traceability rows as Markdown-only source processing records.
- Update `evidence/batches/BATCH-014/` and rerun S06 cargo/fixture checks.

Out of scope:

- Changing acceptance rules.
- Changing prompt counts to avoid primary work.
- Adding direct LLM/provider paths outside Agent Runtime / Provider Adapter.
- Allowing formal state writes outside `Command -> Workflow -> Decision -> Event Store`.

## Primary Implementation Plan

| Prompt | Current-safe source | Contract test | Governance coverage |
|---|---|---|---|
| `P0052` / `CODEX-0377-03-RUNTIME-ORCHESTRATION-fc718c91e6` | `crates/trpg-runtime/src/runtime_workflow_state_machines.rs` | `crates/trpg-runtime/tests/runtime_workflow_state_machines_contract_tests.rs` | Authority Contract, Tool Gate, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0061` / `CODEX-0386-03-RUNTIME-ORCHESTRATION-027bb089fe` | `crates/trpg-runtime/src/capability_layer_impl.rs` | `crates/trpg-runtime/tests/capability_layer_impl_contract_tests.rs` | Authority Contract, Agent Gateway-only formal tool grant, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0062` / `CODEX-0387-03-RUNTIME-ORCHESTRATION-ff36c2cdcf` | `crates/trpg-runtime/src/pending_decision_impl.rs` | `crates/trpg-runtime/tests/pending_decision_impl_contract_tests.rs` | Authority Contract, pending decision gate, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0063` / `CODEX-0388-03-RUNTIME-ORCHESTRATION-705a854eb2` | `crates/trpg-runtime/src/realtime_room_sync_impl.rs` | `crates/trpg-runtime/tests/realtime_room_sync_impl_contract_tests.rs` | Authority Contract, sync visibility redaction, evented decision commit, fact provenance, direct-agent-write deny. |
| `P0064` / `CODEX-0389-03-RUNTIME-ORCHESTRATION-1b60a8b386` | `crates/trpg-runtime/src/saga_transaction_impl.rs` | `crates/trpg-runtime/tests/saga_transaction_impl_contract_tests.rs` | Authority Contract, saga event append through Event Store, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0065` / `CODEX-0390-03-RUNTIME-ORCHESTRATION-12323c9bd9` | `crates/trpg-runtime/src/scheduler_service_impl.rs` | `crates/trpg-runtime/tests/scheduler_service_impl_contract_tests.rs` | Authority Contract, scheduled task event append through Event Store, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0066` / `CODEX-0391-03-RUNTIME-ORCHESTRATION-daba262944` | `crates/trpg-runtime/src/session_runtime_impl.rs` | `crates/trpg-runtime/tests/session_runtime_impl_contract_tests.rs` | Authority Contract, session event append through Event Store, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `P0067` / `CODEX-0392-03-RUNTIME-ORCHESTRATION-1cb6fb735e` | `crates/trpg-runtime/src/workflow_engine_impl.rs` | `crates/trpg-runtime/tests/workflow_engine_impl_contract_tests.rs` | Authority Contract, workflow event append through Event Store, evented decision commit, visibility, fact provenance, direct-agent-write deny. |

## Planned Checks

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test -p trpg-runtime --all-features --jobs 1`
- `cargo test --workspace --all-features --jobs 1`
- `cargo test --test runtime_pending_decision --jobs 1`
- `cargo test --test workflow_engine_contract --jobs 1`
- `cargo test -p trpg-runtime --test batch_012_runtime_contract_tests --all-features --jobs 1`
- Runtime provider/direct-call grep.
- Runtime historical naming grep.
- Visibility leak grep across fixtures, runtime tests, trace docs, and BATCH-014 evidence.

`pnpm` and docker checks are not applicable to this S06 Rust runtime batch because the workspace has no `package.json`, `pnpm-lock.yaml`, `Dockerfile`, or `docker-compose*.yml` files.
