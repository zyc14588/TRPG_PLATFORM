# BATCH-008 Acceptance Evidence

Stage: `S02`
Conclusion: PASS for current B008 scope.

## Changed Files

- `crates/trpg-domain-core/src/lib.rs`
- `crates/trpg-domain-core/src/domain_model.rs`
- `crates/trpg-domain-core/src/domain_command_cqrs.rs`
- `crates/trpg-domain-core/src/domain_event_sourcing_projection.rs`
- `crates/trpg-domain-core/src/domain_visibility_fact_provenance.rs`
- `crates/trpg-domain-core/src/readme.rs`
- `crates/trpg-domain-core/src/visibility_enforcement_points.rs`
- `crates/trpg-domain-core/src/openfga_opa_visibility.rs`
- `crates/trpg-domain-core/src/visibility_leakage_tests.rs`
- `crates/trpg-domain-core/tests/domain_model_contract_tests.rs`
- `crates/trpg-domain-core/tests/domain_command_cqrs_contract_tests.rs`
- `crates/trpg-domain-core/tests/domain_event_sourcing_projection_contract_tests.rs`
- `crates/trpg-domain-core/tests/domain_visibility_fact_provenance_contract_tests.rs`
- `crates/trpg-domain-core/tests/readme_contract_tests.rs`
- `crates/trpg-domain-core/tests/visibility_enforcement_points_contract_tests.rs`
- `crates/trpg-domain-core/tests/openfga_opa_visibility_contract_tests.rs`
- `crates/trpg-domain-core/tests/visibility_leakage_tests_contract_tests.rs`
- `evidence/batches/BATCH-008/WORK_PLAN.md`
- `evidence/batches/BATCH-008/TEST_RESULTS.md`
- `evidence/batches/BATCH-008/ACCEPTANCE_EVIDENCE.md`

## Acceptance Checklist

| Gate | Result | Evidence |
|---|---|---|
| Authority Contract immutable | PASS | Existing authority tests plus B008 domain command tests passed. |
| HUMAN_KP / AI_KP mutually exclusive | PASS | AI_KP direct AI keeper formal write is rejected in B008 tests. |
| Formal state via Command / Decision / Event Store | PASS | B008 command and projection wrappers append only through `EventStore`. |
| Projection is rebuildable read model | PASS | `domain_event_sourcing_projection_rebuilds_from_canon_events` passed. |
| Visibility label propagation | PASS | Visibility replay, enforcement point, and leakage tests passed. |
| Fact provenance boundary | PASS | Agent draft source cannot become confirmed fact. |
| No direct LLM/provider call | PASS | Red-line scan found no provider calls in B008 files. |
| No historical output naming | PASS | New Rust files use current-safe module tails only. |

## Risks And Handoff

- B008 adds domain-level facades and contract tests only. Real SQLx Event Store, OpenFGA/OPA adapters, API handlers, NATS subjects, and provider boundaries remain later batch/stage responsibilities.
- Keep Cargo `-j 1` on Windows if parallel test linking repeats `LNK1104`.
- Next B009+ work should build on these current-safe modules instead of adding source-path-like aliases.
