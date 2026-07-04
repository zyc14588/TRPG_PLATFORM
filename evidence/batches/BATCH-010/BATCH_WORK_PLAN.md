# BATCH-010 Work Plan

Batch: BATCH-010-02-domain-core - Strict Governance Final  
Batch file: batches/B010.md  
Generated: 2026-07-04  

## Scope Resolution

- User-supplied fact said primary prompt count was 0.
- `batches/B010.md`, `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`, `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, and `docs/codex/02-domain-core/per-file-prompt-manifest.md` identify 5 current-safe primary implementation prompts in this batch.
- Execution follows the repository authority order: B010 plus normalized/current-safe maps.
- `source-archive/**` was not used as an executable prompt source.

## Prompt Mapping

| Prompt ID | Role | Current-safe target | Allowed change range | Test responsibility |
|---|---|---|---|---|
| CODEX-0304 / P0069 | documentation-or-traceability | docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_command_cqrs.md | Markdown traceability only | Markdown self-check; command behavior covered by domain-core contract tests |
| CODEX-0305 / P0071 | documentation-or-traceability | docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_authority_contract.md | Markdown traceability only | Markdown self-check; authority behavior covered by B010 authority tests |
| CODEX-0306 / P0078 | documentation-or-traceability | docs/codex/02-domain-core/source_processing_record_docs_implementation_09_security_governance_visibility_enforcement_points.md | Markdown traceability only | Markdown self-check; visibility replay tests |
| CODEX-0307 / P0079 | documentation-or-traceability | docs/codex/02-domain-core/source_processing_record_docs_implementation_10_testing_quality_visibility_leakage_tests.md | Markdown traceability only | Markdown self-check; visibility leakage tests |
| CODEX-0308 / P0080 | primary-implementation | crates/trpg-domain-core/src/adr_0003_authority_contract.rs; crates/trpg-domain-core/tests/adr_0003_authority_contract_contract_tests.rs | Authority ADR wrapper, fork-only guard, event-store append path | Authority violation/no event, direct-agent/no event, replay visibility/provenance |
| CODEX-0309 / P0081 | supplemental-requirement | codex-prompts/02-domain-core/P0081.md | Requirement merge only; no Rust output | Covered through CODEX-0308 authority tests |
| CODEX-0310 / P0082 | primary-implementation | crates/trpg-domain-core/src/character_combat_san_chase.rs; crates/trpg-domain-core/tests/character_combat_san_chase_contract_tests.rs | Character/combat/SAN/chase decision wrapper over formal command append | Authority violation/no event, replay visibility/provenance |
| CODEX-0311 / P0083 | supplemental-requirement | codex-prompts/02-domain-core/P0083.md | Requirement merge only; no Rust output | Covered through command and primary wrapper tests |
| CODEX-0312 / P0084 | supplemental-requirement | codex-prompts/02-domain-core/P0084.md | Requirement merge only; no Rust output | Covered by existing domain model and S02 fixture tests |
| CODEX-0313 / P0085 | primary-implementation | crates/trpg-domain-core/src/event_sourcing_projection.rs; crates/trpg-domain-core/tests/event_sourcing_projection_contract_tests.rs | Canon event append/rebuild/replay wrapper | Authority violation/no event, projection rebuild, replay visibility/provenance |
| CODEX-0314 / P0086 | primary-implementation | crates/trpg-domain-core/src/investigation_clue_npc_time.rs; crates/trpg-domain-core/tests/investigation_clue_npc_time_contract_tests.rs | Investigation/clue/NPC/time decision wrapper over formal command append | Authority violation/no event, replay visibility/provenance |
| CODEX-0315 / P0087 | primary-implementation | crates/trpg-domain-core/src/rule_runtime_coc7.rs; crates/trpg-domain-core/tests/rule_runtime_coc7_contract_tests.rs | COC7 rule runtime decision wrapper over formal command append | Authority violation/no event, dice source, replay visibility/provenance |
| CODEX-0316 / P0088 | supplemental-requirement | codex-prompts/02-domain-core/P0088.md | Requirement merge only; no Rust output | Covered through visibility/provenance replay tests |
| CODEX-0317 / P0089 | supplemental-requirement | codex-prompts/02-domain-core/P0089.md | Requirement merge only; no Rust output | Covered by authority guard and B010 authority tests |
| CODEX-0318 / P0090 | supplemental-requirement | codex-prompts/02-domain-core/P0090.md | Requirement merge only; no Rust output | Covered by existing idempotency/expected-version tests |
| CODEX-0319 / P0091 | supplemental-requirement | codex-prompts/02-domain-core/P0091.md | Requirement merge only; no Rust output | Covered by decision record and S02 fixture tests |
| CODEX-0320 / P0092 | supplemental-requirement | codex-prompts/02-domain-core/P0092.md | Requirement merge only; no Rust output | Covered by domain entity/value object tests |
| CODEX-0321 / P0093 | supplemental-requirement | codex-prompts/02-domain-core/P0093.md | Requirement merge only; no Rust output | Covered by policy hook and authority tests |
| CODEX-0322 / P0094 | supplemental-requirement | codex-prompts/02-domain-core/P0094.md | Requirement merge only; no Rust output | Covered by fork lineage and S02 fixture tests |
| CODEX-0323 / P0095 | supplemental-requirement | codex-prompts/02-domain-core/P0095.md | Requirement merge only; no Rust output | Covered by readme contract test |
| CODEX-0324 / P0096 | supplemental-requirement | codex-prompts/02-domain-core/P0096.md | Requirement merge only; no Rust output | Covered by visibility/provenance tests |
| CODEX-0325 / P0097 | supplemental-requirement | codex-prompts/02-domain-core/P0097.md | Requirement merge only; no Rust output | Covered by visibility enforcement tests |
| CODEX-0326 / P0098 | supplemental-requirement | codex-prompts/02-domain-core/P0098.md | Requirement merge only; no Rust output | Covered by visibility leakage tests |
| CODEX-0327 / P0099 | supplemental-requirement | codex-prompts/02-domain-core/P0099.md | Requirement merge only; no Rust output | Covered through existing authority_contract_impl and B010 authority tests |
| CODEX-0328 / P0105 | supplemental-requirement | codex-prompts/02-domain-core/P0105.md | Requirement merge only; no Rust output | Covered through existing and B010 character/combat/SAN/chase tests |

## Execution Plan

1. Add missing documentation-only source processing records for the four traceability prompts.
2. Add current-safe primary module files for the five B010 primary prompts and export them from `lib.rs`.
3. Add current-safe contract test files matching B010 declared outputs.
4. Run focused B010 tests first.
5. Run the S02 stage checks from `stages/s02-domain-core-authority-event-model/TEST_PLAN.md`.
6. Record command outputs in `evidence/batches/BATCH-010/TEST_RESULTS.md` and summarize acceptance in `evidence/batches/BATCH-010/ACCEPTANCE_EVIDENCE.md`.
