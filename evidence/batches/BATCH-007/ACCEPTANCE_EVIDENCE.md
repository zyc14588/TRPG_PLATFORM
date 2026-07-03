# BATCH-007 Acceptance Evidence

## Result

PASS for current BATCH-007 scope.

## Changed Files

- `Cargo.toml`
- `Cargo.lock`
- `crates/trpg-domain-core/Cargo.toml`
- `crates/trpg-domain-core/src/*.rs`
- `crates/trpg-domain-core/tests/*_contract_tests.rs`
- `docs/codex/02-domain-core/m_02_domain_core.md`
- `codex-prompts/02-domain-core/P0010.md`
- `codex-prompts/02-domain-core/P0012.md`
- `codex-prompts/02-domain-core/P0013.md`
- `codex-prompts/02-domain-core/P0014.md`
- `codex-prompts/02-domain-core/P0015.md`
- `codex-prompts/02-domain-core/P0016.md`
- `codex-prompts/02-domain-core/P0021.md`
- `codex-prompts/02-domain-core/P0022.md`
- `codex-prompts/02-domain-core/P0023.md`
- `codex-prompts/02-domain-core/P0024.md`
- `codex-prompts/02-domain-core/P0029.md`
- `docs/reports/stages/S02_ACCEPTANCE_EVIDENCE.md`
- `docs/reports/stages/S02_TEST_RESULTS.md`
- `docs/reports/stages/S02_TRACEABILITY.md`
- `evidence/stages/S02/authority-contract-tests.txt`
- `evidence/stages/S02/fact-provenance-tests.txt`
- `evidence/batches/BATCH-007/*.md`

## Gates

| Gate | Evidence |
|---|---|
| Authority Contract immutable | `authority_contract_contract_tests.rs` |
| AI cannot directly write formal state | `command_cqrs_contract_tests.rs` |
| Formal state goes through command/event path | `command_cqrs_contract_tests.rs` |
| Event Store is canon; projection rebuild is read model | `event_sourcing_snapshot_projection_contract_tests.rs` |
| Visibility redaction | `visibility_fact_provenance_contract_tests.rs` |
| Fact Provenance / confirmed fact source | `domain_entities_value_objects_contract_tests.rs` and `visibility_fact_provenance_contract_tests.rs` |
| Fixture acceptance converted to executable assertions | `s02_fixture_acceptance_contract_tests.rs` |
| Supplemental prompts do not create Rust output | `codex-prompts/02-domain-core/P0010.md`, `P0012.md`, `P0013.md`, `P0014.md`, `P0015.md`, `P0016.md`, `P0021.md`, `P0022.md`, `P0023.md`, `P0024.md`, `P0029.md` |

## Prompt Row Conclusions

| Prompt ID | Role | Conclusion | Evidence |
|---|---|---|---|
| `CODEX-0023-02-DOMAIN-CORE-afd26a0bfd` | docs-governance | PASS | `docs/codex/02-domain-core/m_02_domain_core.md` |
| `CODEX-0024-02-DOMAIN-CORE-f28763f1b2` | primary | PASS | `authority_contract_guard.rs`, `authority_contract_guard_contract_tests.rs` |
| `CODEX-0025-02-DOMAIN-CORE-d6e83289f7` | primary | PASS | `command_cqrs_idempotency.rs`, `command_cqrs_idempotency_contract_tests.rs` |
| `CODEX-0026-02-DOMAIN-CORE-19aeeb927d` | primary | PASS | `decision_record_model.rs`, `decision_record_model_contract_tests.rs` |
| `CODEX-0027-02-DOMAIN-CORE-dd3272feaf` | primary | PASS | `domain_entities_value_objects.rs`, `domain_entities_value_objects_contract_tests.rs` |
| `CODEX-0028-02-DOMAIN-CORE-3acc117855` | primary | PASS | `domain_policy_hooks.rs`, `domain_policy_hooks_contract_tests.rs` |
| `CODEX-0029-02-DOMAIN-CORE-37d1d366c5` | primary | PASS | `fork_canon_lineage.rs`, `fork_canon_lineage_contract_tests.rs` |
| `CODEX-0030-02-DOMAIN-CORE-b694d9c49d` | primary | PASS | `visibility_fact_provenance.rs`, `visibility_fact_provenance_contract_tests.rs` |
| `CODEX-0237-02-DOMAIN-CORE-b5d89d5c05` | primary | PASS | `adr_0003_authority_contract_authority_contract.rs`, `adr_0003_authority_contract_authority_contract_contract_tests.rs` |
| `CODEX-0238-02-DOMAIN-CORE-84c2536bcd` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0010.md` |
| `CODEX-0239-02-DOMAIN-CORE-626c99ebbb` | primary | PASS | `command_authority_visibility.rs`, `command_authority_visibility_contract_tests.rs` |
| `CODEX-0240-02-DOMAIN-CORE-29037d1e55` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0012.md` |
| `CODEX-0241-02-DOMAIN-CORE-72131fafdf` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0013.md` |
| `CODEX-0242-02-DOMAIN-CORE-a2ed8f32e6` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0014.md` |
| `CODEX-0243-02-DOMAIN-CORE-79a1b0d106` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0015.md` |
| `CODEX-0244-02-DOMAIN-CORE-f4b75ec825` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0016.md` |
| `CODEX-0245-02-DOMAIN-CORE-34eeb364aa` | primary | PASS | `authority_contract.rs`, `authority_contract_contract_tests.rs` |
| `CODEX-0246-02-DOMAIN-CORE-87c3f50d0f` | primary | PASS | `command_cqrs.rs`, `command_cqrs_contract_tests.rs` |
| `CODEX-0247-02-DOMAIN-CORE-3837e9b57c` | primary | PASS | `ddd.rs`, `ddd_contract_tests.rs` |
| `CODEX-0248-02-DOMAIN-CORE-c3a01b2873` | primary | PASS | `event_sourcing_snapshot_projection.rs`, `event_sourcing_snapshot_projection_contract_tests.rs` |
| `CODEX-0249-02-DOMAIN-CORE-08e97c524e` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0021.md` |
| `CODEX-0250-02-DOMAIN-CORE-20bde1eea0` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0023.md` |
| `CODEX-0251-02-DOMAIN-CORE-0f46c363c7` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0022.md` |
| `CODEX-0252-02-DOMAIN-CORE-e1fbc903f4` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0024.md` |
| `CODEX-0253-02-DOMAIN-CORE-efa750909e` | supplemental | PASS | landed in `codex-prompts/02-domain-core/P0029.md` |

## Explicit Non-changes

- No SQLx migrations.
- No API handlers or OpenAPI schema.
- No NATS subject implementation.
- No model provider or direct LLM calls.
- No source-archive material used as current implementation naming.

## Remaining Risks

- Domain event store is in-memory for B007. SQLx transaction/outbox work
  belongs to later data/eventing batches.
- OpenFGA/OPA is represented by pure policy hooks only. Full policy service
  integration belongs to security governance batches.
- API/WebSocket/NATS contract implementation belongs to later API/realtime
  batches.

## Handoff

Next `02-domain-core` batches should extend this crate rather than duplicate
Authority, CommandEnvelope, Visibility, or FactProvenance primitives.
