# M10 Testing Quality

Batch: `BATCH-038-10-testing-quality`  
Stage: `S11 testing quality golden CI`  
Current crate: `trpg-testing`

## Scope

This category owns test-harness and documentation outputs for the strict governance testing layer. It verifies that V1 acceptance scenarios, golden scenario replay, visibility leakage tests, export diff checks, and model certification gates are represented as current-safe Rust contract tests and traceability documents.

## Current-Safe Outputs

- `crates/trpg-testing/src/benchmark_plan.rs`
- `crates/trpg-testing/src/model_certification_tests.rs`
- `crates/trpg-testing/src/replay_consistency_tests.rs`
- `crates/trpg-testing/src/test_strategy.rs`
- `crates/trpg-testing/src/testing_golden_ci.rs`
- `crates/trpg-testing/src/visibility_leakage_tests.rs`
- `crates/trpg-testing/src/decision_trace_map.rs`
- `crates/trpg-testing/src/contract_test_matrix.rs`
- `crates/trpg-testing/src/testing_golden_scenarios_ci.rs`
- `crates/trpg-testing/src/golden_scenario_ci.rs`
- `crates/trpg-testing/src/implementation_acceptance_checklist.rs`
- `crates/trpg-testing/src/readme.rs`

## Governance Boundary

- Formal test decisions are recorded through `CommandEnvelope -> EventStore` using `trpg-shared-kernel`.
- Visibility and fact provenance are preserved on event envelopes.
- Direct agent and direct business formal write paths remain forbidden by the shared kernel.
- Model certification checks reuse `trpg-agent-runtime`; testing-quality does not call providers or bare LLMs.
- Supplemental prompts are recorded as requirement documents and merged into primary module tests.

## Stage Evidence

Batch evidence is written under `evidence/batches/BATCH-038/`. The minimal check is `cargo test -p trpg-testing --all-features`; S11-specific checks cover golden scenario, visibility leakage, and model certification contract tests.
