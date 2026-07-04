# BATCH-014 Test Results

## Cargo Checks

| Command | Result | Evidence |
|---|---:|---|
| `cargo fmt --all` | PASS | Formatting completed; Cargo emitted only `warn: could not canonicalize path C:\Users\zyc14588`. |
| `cargo fmt --all -- --check` | PASS | Format check completed with the same non-blocking canonicalize warning. |
| `cargo check --workspace --all-features` | PASS | `trpg-runtime` checked successfully. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | No clippy warnings. |
| `cargo test -p trpg-runtime --all-features --jobs 1` | PASS | All runtime integration tests passed, including the 8 new B014 primary contract test files. |
| `cargo test --workspace --all-features --jobs 1` | PASS | Workspace tests and doc-tests passed. |
| `cargo test --test runtime_pending_decision --jobs 1` | PASS | `1 passed; 0 failed`. |
| `cargo test --test workflow_engine_contract --jobs 1` | PASS | `1 passed; 0 failed`. |
| `cargo test -p trpg-runtime --test batch_012_runtime_contract_tests --all-features --jobs 1` | PASS | `15 passed; 0 failed`; includes S06 fixture binding assertions. |

Note: an initial parallel `cargo test -p trpg-runtime --all-features` run hit Windows/MSVC `LNK1104` file-open errors while linking multiple integration test executables. The same test suite passed with `--jobs 1`, so the recorded acceptance run uses serialized linking to avoid environment file locks.

## B014 Primary Contract Tests

| Test file | Result | Required coverage |
|---|---:|---|
| `crates/trpg-runtime/tests/runtime_workflow_state_machines_contract_tests.rs` | PASS | Prompt ID, `BATCH_014_PRIMARY_MODULES`, Authority Contract immutability, Tool Gate, `Command -> Workflow -> Decision -> Event Store`, visibility redaction, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/capability_layer_impl_contract_tests.rs` | PASS | Prompt ID, Agent Gateway-only formal tool approval via runtime gate, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/pending_decision_impl_contract_tests.rs` | PASS | Prompt ID, pending decision readiness, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/realtime_room_sync_impl_contract_tests.rs` | PASS | Prompt ID, realtime sync uses visible replay, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/saga_transaction_impl_contract_tests.rs` | PASS | Prompt ID, saga compensation event append through Event Store, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/scheduler_service_impl_contract_tests.rs` | PASS | Prompt ID, due task selection and event append through Event Store, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/session_runtime_impl_contract_tests.rs` | PASS | Prompt ID, session start event append through Event Store, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |
| `crates/trpg-runtime/tests/workflow_engine_impl_contract_tests.rs` | PASS | Prompt ID, workflow advance event append through Event Store, Authority Contract, evented decision commit, visibility, fact provenance, direct-agent-write deny. |

## Static Governance Checks

| Check | Command | Result |
|---|---|---:|
| Direct provider call path in runtime | `rg -n "OpenAI\|openai\|Ollama\|ollama\|llama\|chat_completion\|responses\.create\|OPENAI_API_KEY\|reqwest\|ureq\|hyper::Client\|ProviderAdapter\|ModelProvider" crates/trpg-runtime/src crates/trpg-runtime/tests --glob "*.rs"` | PASS, no matches. |
| Historical V3/V4/V5/V6/hash naming in runtime Rust outputs | `rg -n "v3\|v4\|v5\|v6\|756708a3ff\|84ae510c22\|69e5c5de1b\|91e0a81c18\|16c1e0602b\|6bc59ee046\|42f8d6cb8f\|f1c226d808" crates/trpg-runtime/src crates/trpg-runtime/tests --glob "*.rs"` | PASS, no matches. |
| B014 current-safe module index | `rg -n "BATCH_014_PRIMARY_MODULES\|RuntimeWorkflowStateMachines\|CapabilityLayerImpl\|PendingDecisionImpl\|RealtimeRoomSyncImpl\|SagaTransactionImpl\|SchedulerServiceImpl\|SessionRuntimeImpl\|WorkflowEngineImpl" crates/trpg-runtime/src crates/trpg-runtime/tests --glob "*.rs"` | PASS, expected matches only. |
| Visibility leak scan | `rg -n "player_visible\|player-visible\|public.*keeper_only\|public.*private_to_player\|public.*ai_internal\|export.*ai_internal\|sync.*ai_internal" fixtures crates/trpg-runtime/tests docs/codex/03-runtime-orchestration evidence/batches/BATCH-014 --glob "*.md" --glob "*.json" --glob "*.rs"` | PASS, only negative fixture constraint match: `fixtures/agent/agent_tool_gate_cases.v1.json.md`. |
| pnpm/docker applicability | `rg --files -g "package.json" -g "pnpm-lock.yaml" -g "docker-compose*.yml" -g "docker-compose*.yaml" -g "Dockerfile"` | PASS, no files; not applicable to this Rust-only S06 batch. |

## Fixture Checks

- `cargo test -p trpg-runtime --test batch_012_runtime_contract_tests --all-features --jobs 1` binds `fixtures/stages/S06_stage_acceptance_fixture.v1.json.md` and `fixtures/stages/detailed/S06_decision_pipeline_commit_expected.current.json.md`.
- The fixture test asserts tool approval, decision commit, tool gate denial, HUMAN_KP draft-only enforcement, direct-agent-write deny, expected records, and S06 report references.
