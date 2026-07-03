# BATCH-008 Work Plan

Batch: `BATCH-008-02-domain-core -- Strict Governance Final`
Stage: `S02-domain-core-authority-event-model`

## Scope Decision

- Required maps read before execution: `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`, `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, `CURRENT_TOKEN_REWRITE_TABLE.md`.
- Conflict noted: the user summary said primary prompt count was `0`, but active `batches/B008.md` declares 8 `primary-implementation` rows and 17 `supplemental-requirement` rows. This execution followed the active batch file after current-safe mapping.
- No `source-archive/**` material was used as a current implementation source.
- No SQLx migration, API handler, WebSocket contract, NATS subject, provider call, or direct LLM path was introduced.

## Prompt Mapping

| Prompt ID | Prompt file | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0254-02-DOMAIN-CORE-1f1ccc33b4` | `P0026.md` | primary | `crates/trpg-domain-core/src/domain_model.rs`, `tests/domain_model_contract_tests.rs` | Implement flat domain model wrapper over existing command/event guards. | Governed command accepted; direct agent state write denied. |
| `CODEX-0255-02-DOMAIN-CORE-3039902cd1` | `P0027.md` | primary | `crates/trpg-domain-core/src/domain_command_cqrs.rs`, `tests/domain_command_cqrs_contract_tests.rs` | Implement CQRS decision append wrapper. | Idempotency/version conflict and AI_KP actor authority negative case. |
| `CODEX-0256-02-DOMAIN-CORE-0c6ca0f189` | `P0025.md` | primary | `crates/trpg-domain-core/src/domain_event_sourcing_projection.rs`, `tests/domain_event_sourcing_projection_contract_tests.rs` | Implement canon event append/projection/replay wrappers. | Projection rebuild and visibility-filtered replay. |
| `CODEX-0257-02-DOMAIN-CORE-c1fa0e7a18` | `P0028.md` | primary | `crates/trpg-domain-core/src/domain_visibility_fact_provenance.rs`, `tests/domain_visibility_fact_provenance_contract_tests.rs` | Implement visibility/provenance helper facade. | Restrictive label, invalid fact source, AI internal redaction. |
| `CODEX-0258-02-DOMAIN-CORE-424a75fc5c` | `P0030.md` | supplemental | Merge to `authority_contract` primary from prior batch | No Rust ownership in B008. | Covered by existing authority tests and S02 fixture tests. |
| `CODEX-0259-02-DOMAIN-CORE-ddf80a8c3a` | `P0031.md` | supplemental | Merge to `command_cqrs` primary from prior batch | No Rust ownership in B008. | Covered by command CQRS tests. |
| `CODEX-0260-02-DOMAIN-CORE-1f9572bf20` | `P0032.md` | supplemental | Merge to `ddd` primary from prior batch | No Rust ownership in B008. | Covered by DDD fact-source boundary tests. |
| `CODEX-0261-02-DOMAIN-CORE-d099d1d27a` | `P0033.md` | supplemental | Merge to `visibility_fact_provenance` primary from prior batch | No Rust ownership in B008. | Covered by visibility/provenance tests. |
| `CODEX-0262-02-DOMAIN-CORE-021b18ca0b` | `P0034.md` | supplemental | Merge to `authority_contract_guard` primary from prior batch | No Rust ownership in B008. | Covered by authority guard tests. |
| `CODEX-0263-02-DOMAIN-CORE-22d36897c9` | `P0036.md` | supplemental | Merge to `command_cqrs_idempotency` primary from prior batch | No Rust ownership in B008. | Covered by idempotency tests. |
| `CODEX-0264-02-DOMAIN-CORE-57a1ce24f9` | `P0037.md` | supplemental | Merge to `decision_record_model` primary from prior batch | No Rust ownership in B008. | Covered by decision record tests. |
| `CODEX-0265-02-DOMAIN-CORE-1640e6d5e5` | `P0035.md` | supplemental | Merge to `domain_entities_value_objects` primary from prior batch | No Rust ownership in B008. | Covered by memory fact tests. |
| `CODEX-0266-02-DOMAIN-CORE-79546fd42d` | `P0038.md` | supplemental | Merge to `domain_policy_hooks` primary from prior batch | No Rust ownership in B008. | Covered by policy hook tests. |
| `CODEX-0267-02-DOMAIN-CORE-ba21ceee5a` | `P0040.md` | supplemental | Merge to `fork_canon_lineage` primary from prior batch | No Rust ownership in B008. | Covered by fork lineage tests. |
| `CODEX-0268-02-DOMAIN-CORE-f6844b3829` | `P0041.md` | primary | `crates/trpg-domain-core/src/readme.rs`, `tests/readme_contract_tests.rs` | Implement machine-checkable domain-core boundary snapshot. | Invariants and non-goals present. |
| `CODEX-0269-02-DOMAIN-CORE-79628611e3` | `P0039.md` | supplemental | Merge to `visibility_fact_provenance` primary from prior batch | No Rust ownership in B008. | Covered by visibility/provenance tests. |
| `CODEX-0270-02-DOMAIN-CORE-38f82f3348` | `P0042.md` | supplemental | Merge to `readme` primary | No separate Rust ownership. | Covered by readme contract test. |
| `CODEX-0271-02-DOMAIN-CORE-e156c15eea` | `P0043.md` | supplemental | Merge to `visibility_fact_provenance` primary from prior batch | No Rust ownership in B008. | Covered by visibility/provenance tests. |
| `CODEX-0272-02-DOMAIN-CORE-0ebf9d6c71` | `P0044.md` | primary | `crates/trpg-domain-core/src/visibility_enforcement_points.rs`, `tests/visibility_enforcement_points_contract_tests.rs` | Implement visibility enforcement point facade. | Restricted summary denied; matching private player allowed. |
| `CODEX-0273-02-DOMAIN-CORE-b0c90652e4` | `P0045.md` | primary | `crates/trpg-domain-core/src/openfga_opa_visibility.rs`, `tests/openfga_opa_visibility_contract_tests.rs` | Implement domain-level relation plus policy plus visibility decision. | Default deny and allow path tests. |
| `CODEX-0274-02-DOMAIN-CORE-1f8f871a84` | `P0046.md` | supplemental | Merge to `visibility_enforcement_points` primary | No separate Rust ownership. | Covered by visibility enforcement point tests. |
| `CODEX-0275-02-DOMAIN-CORE-473b0245b0` | `P0047.md` | primary | `crates/trpg-domain-core/src/visibility_leakage_tests.rs`, `tests/visibility_leakage_tests_contract_tests.rs` | Implement visibility leakage probe helper. | Keeper-only export non-leak and public export visible. |
| `CODEX-0276-02-DOMAIN-CORE-15e44ea9b8` | `P0048.md` | supplemental | Merge to `visibility_leakage_tests` primary | No separate Rust ownership. | Covered by leakage probe tests. |
| `CODEX-0277-02-DOMAIN-CORE-da9801ec9c` | `P0049.md` | supplemental | Merge to `domain_visibility_fact_provenance` primary | No separate Rust ownership. | Covered by domain visibility/provenance tests. |
| `CODEX-0278-02-DOMAIN-CORE-db1143a173` | `P0050.md` | supplemental | Merge to `authority_contract` primary from prior batch | No Rust ownership in B008. | Covered by authority tests. |

## Implementation Plan

1. Reuse existing B007 domain primitives instead of duplicating authority, event store, policy, or visibility logic.
2. Add only current-safe B008 flat modules and tests.
3. Export new modules from `crates/trpg-domain-core/src/lib.rs`.
4. Run minimal checks first, then S02 checks.
5. Record evidence under `evidence/batches/BATCH-008/`.
