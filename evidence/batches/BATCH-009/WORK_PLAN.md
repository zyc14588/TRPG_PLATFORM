# BATCH-009 Work Plan

Batch: `BATCH-009-02-domain-core -- Strict Governance Final`
Stage: `S02-domain-core-authority-event-model`

## Scope Decision

- Required inputs were read before execution: `AGENTS.md`, `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`, `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`, `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`, normalized prompt map, current-safe module/output map, token rewrite table, operator guides, S02 stage prompts, and `batches/B009.md`.
- The user summary said primary prompt count was `0`; active `batches/B009.md` declares 8 `primary-implementation` rows and 17 `documentation-or-traceability` rows. This execution followed the active batch file plus current-safe maps.
- `source-archive/**` was not used as executable prompt material. Historical paths and hashes remain provenance only.
- No SQLx migration, API handler, NATS subject, workflow engine, metric label, provider call, or direct LLM access was introduced.

## Prompt Mapping

| Prompt ID | Prompt file | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0279-02-DOMAIN-CORE-4761a0a9b6` | `P0051.md` | primary | `crates/trpg-domain-core/src/authority_contract_impl.rs`, `tests/authority_contract_impl_contract_tests.rs` | Add current-safe authority facade over locked contract, fork, and event append. | Authority violation leaves Event Store empty; visibility/provenance replay retained. |
| `CODEX-0280-02-DOMAIN-CORE-c81df4c435` | `P0052.md` | primary | `crates/trpg-domain-core/src/character_combat_san_chase_impl.rs`, `tests/character_combat_san_chase_impl_contract_tests.rs` | Add character/combat/sanity/chase decision facade over governed command append. | Authority violation; KeeperOnly replay and provenance retention. |
| `CODEX-0281-02-DOMAIN-CORE-1e4096357b` | `P0053.md` | primary | `crates/trpg-domain-core/src/command_cqrs_impl.rs`, `tests/command_cqrs_impl_contract_tests.rs` | Add CQRS implementation facade using existing decision append. | Authority violation; command event carries fact source and provenance. |
| `CODEX-0282-02-DOMAIN-CORE-b1fe69de22` | `P0054.md` | primary | `crates/trpg-domain-core/src/domain_model_impl.rs`, `tests/domain_model_impl_contract_tests.rs` | Add domain model facade and direct-write rejection wrapper. | Direct agent write denied; visibility/provenance replay retained. |
| `CODEX-0283-02-DOMAIN-CORE-d29066e385` | `P0055.md` | primary | `crates/trpg-domain-core/src/event_sourcing_projection_impl.rs`, `tests/event_sourcing_projection_impl_contract_tests.rs` | Add projection facade over canon event append, rebuild, and visible replay. | Authority violation; projection rebuild and replay visibility. |
| `CODEX-0284-02-DOMAIN-CORE-370ec69864` | `P0056.md` | primary | `crates/trpg-domain-core/src/investigation_clue_npc_time_impl.rs`, `tests/investigation_clue_npc_time_impl_contract_tests.rs` | Add investigation/clue/NPC/time decision facade. | Authority violation; clue/fact provenance and visibility replay. |
| `CODEX-0285-02-DOMAIN-CORE-d1c3bee3b7` | `P0057.md` | primary | `crates/trpg-domain-core/src/rule_runtime_coc7_impl.rs`, `tests/rule_runtime_coc7_impl_contract_tests.rs` | Add COC7 rule runtime decision facade. | Authority violation; dice/rule fact source and visibility replay. |
| `CODEX-0286-02-DOMAIN-CORE-590846948a` | `P0058.md` | primary | `crates/trpg-domain-core/src/visibility_fact_provenance_impl.rs`, `tests/visibility_fact_provenance_impl_contract_tests.rs` | Add visibility/provenance facade over confirmation, redaction, and fact promotion event append. | Untrusted fact source denied; visibility/provenance replay retained. |
| `CODEX-0287-02-DOMAIN-CORE-1da26d2a42` | `P0059.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_adr_adr_0003_authority_contract.md` | Markdown traceability only. | Covered by prompt coverage and S02 authority tests. |
| `CODEX-0288-02-DOMAIN-CORE-e61cec44ff` | `P0063.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_domain_authority_contract.md` | Markdown traceability only. | Covered by authority tests. |
| `CODEX-0289-02-DOMAIN-CORE-23561fde42` | `P0060.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_domain_command_cqrs.md` | Markdown traceability only. | Covered by command CQRS tests. |
| `CODEX-0290-02-DOMAIN-CORE-fa002dbfe2` | `P0064.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_domain_domain_model.md` | Markdown traceability only. | Covered by domain model tests. |
| `CODEX-0291-02-DOMAIN-CORE-78582c41f8` | `P0061.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_domain_event_sourcing_projection.md` | Markdown traceability only. | Covered by projection tests. |
| `CODEX-0292-02-DOMAIN-CORE-df5328e594` | `P0062.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_domain_visibility_fact_provenance.md` | Markdown traceability only. | Covered by visibility/provenance tests. |
| `CODEX-0293-02-DOMAIN-CORE-ca4a907d8a` | `P0074.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_authority_contract_guard.md` | Markdown traceability only. | Covered by authority guard tests. |
| `CODEX-0294-02-DOMAIN-CORE-058696937e` | `P0065.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_command_cqrs_idempotency.md` | Markdown traceability only. | Covered by idempotency tests. |
| `CODEX-0295-02-DOMAIN-CORE-f05a65dd2d` | `P0077.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_decision_record_model.md` | Markdown traceability only. | Covered by decision record tests. |
| `CODEX-0296-02-DOMAIN-CORE-430bf5db34` | `P0070.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_domain_entities_value_objects.md` | Markdown traceability only. | Covered by entity/value-object tests. |
| `CODEX-0297-02-DOMAIN-CORE-db3f90a478` | `P0076.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_domain_policy_hooks.md` | Markdown traceability only. | Covered by policy hook tests. |
| `CODEX-0298-02-DOMAIN-CORE-aefe84a19e` | `P0073.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_fork_canon_lineage.md` | Markdown traceability only. | Covered by fork lineage tests. |
| `CODEX-0299-02-DOMAIN-CORE-d62ef1220a` | `P0075.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_readme.md` | Markdown traceability only. | Covered by readme tests. |
| `CODEX-0300-02-DOMAIN-CORE-969be566cc` | `P0072.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_02_domain_core_visibility_fact_provenance.md` | Markdown traceability only. | Covered by visibility/provenance tests. |
| `CODEX-0301-02-DOMAIN-CORE-2e9f7b393f` | `P0068.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_visibility_fact_provenance.md` | Markdown traceability only. | Covered by prompt coverage and visibility tests. |
| `CODEX-0302-02-DOMAIN-CORE-145844a6a3` | `P0067.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_event_sourcing_projection.md` | Markdown traceability only. | Covered by prompt coverage and projection tests. |
| `CODEX-0303-02-DOMAIN-CORE-0cadd5b28e` | `P0066.md` | traceability | `docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_domain_model.md` | Markdown traceability only. | Covered by prompt coverage and domain model tests. |

## Execution Plan

1. Reuse existing domain-core authority, command CQRS, event-store, projection, visibility, and fact-provenance primitives.
2. Add only the 8 BATCH-009 current-safe primary modules plus their contract tests.
3. Add traceability-only Markdown records for the 17 documentation prompts.
4. Run minimal related check first, then S02 stage checks.
5. Record results and acceptance evidence under `evidence/batches/BATCH-009/`.
