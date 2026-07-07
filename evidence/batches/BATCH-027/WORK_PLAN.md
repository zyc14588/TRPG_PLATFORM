# BATCH-027 Work Plan

Batch: BATCH-027-06-data-eventing - Strict Governance Final
Stage: S03 data-eventing persistence
Date: 2026-07-06

## Governance Inputs Read

- AGENTS.md
- CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
- SOURCE_BUNDLE_INTEGRATION_GUIDE.md
- docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md
- CODEX_MASTER_EXECUTION_GUIDE.md
- CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md
- CODEX_STRICT_OPERATION_CHECKLIST.md
- codex-operator-guides/README.md
- docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
- docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
- docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
- docs/codex/00-index/codex-persistent-context.md
- docs/codex/00-index/codex-prompt-boundary.md
- V1_ACCEPTANCE_EVIDENCE_MATRIX.md
- PER_STAGE_FIXTURE_EXPANSION_PLAN.md
- batches/B027.md
- stages/s03-data-eventing-persistence/{README.md,START_PROMPT.md,TEST_PLAN.md,TEST_DATA.md,ACCEPTANCE_PROMPT.md,REPAIR_PROMPT.md}
- docs/codex/06-data-eventing/{AGENTS.md,README.md,per-file-prompt-manifest.md,codex-module-code-prompt.md,codex-module-test-prompt.md,codex-module-review-prompt.md,m_06_data_eventing.md}
- codex-prompts/06-data-eventing/P0069.md, P0071.md, P0075.md, P0078.md, P0079.md, P0081.md through P0100.md where referenced by B027

## Repair Scope Reconciliation

Strict acceptance found that B027 and the current-safe maps identify two primary implementation rows that had not been implemented in the prior evidence:

- CODEX-0661-06-DATA-EVENTING-d4c088ceeb -> crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs.rs and crates/trpg-data-eventing/tests/adr_0002_event_sourcing_cqrs_contract_tests.rs
- CODEX-0663-06-DATA-EVENTING-80272d7032 -> crates/trpg-data-eventing/src/adr_0005_postgres_pgvector.rs and crates/trpg-data-eventing/tests/adr_0005_postgres_pgvector_contract_tests.rs

This repair is limited to those two failed primary rows, their registration in crates/trpg-data-eventing/src/lib.rs, target tests, and B027 evidence correction. The 10 documentation-or-traceability records from the earlier B027 pass are retained. The 13 supplemental prompts remain prompt-only merge inputs and do not expand implementation scope.

## Prompt Map

| Prompt file | Prompt ID | Role this run | Current-safe target | Allowed changes | Test responsibility |
| --- | --- | --- | --- | --- | --- |
| P0075 | CODEX-0651-06-DATA-EVENTING-845286620c | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_readme.md | Traceability Markdown only | Markdown/path self-check; no code test ownership |
| P0069 | CODEX-0652-06-DATA-EVENTING-0c28e0b59a | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_snapshot_strategy.md | Traceability Markdown only | Markdown/path self-check; projection tests remain S03-owned |
| P0078 | CODEX-0653-06-DATA-EVENTING-b747cb4ad7 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_event_bus_nats.md | Traceability Markdown only | Markdown/path self-check; NATS tests remain primary-owned |
| P0079 | CODEX-0654-06-DATA-EVENTING-c650c81a4b | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_persistence_postgresql.md | Traceability Markdown only | Markdown/path self-check; SQLx tests remain S03-owned |
| P0071 | CODEX-0655-06-DATA-EVENTING-39e1d5396f | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_cache_redis.md | Traceability Markdown only | Markdown/path self-check; cache tests remain primary-owned |
| P0083 | CODEX-0656-06-DATA-EVENTING-8cf7c65caf | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_platform_cache_redis.md | Traceability Markdown only | Markdown/path self-check |
| P0082 | CODEX-0657-06-DATA-EVENTING-8cd5928199 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_platform_event_bus_nats.md | Traceability Markdown only | Markdown/path self-check |
| P0081 | CODEX-0658-06-DATA-EVENTING-7a272232f2 | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_platform_persistence_postgresql.md | Traceability Markdown only | Markdown/path self-check |
| P0084 | CODEX-0659-06-DATA-EVENTING-faa099023a | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_schemas_event_json_schema.md | Traceability Markdown only | Markdown/path self-check |
| P0085 | CODEX-0660-06-DATA-EVENTING-d6b6be75de | documentation-or-traceability | docs/codex/06-data-eventing/source_processing_record_docs_implementation_07_api_realtime_contracts_nats_subject_contracts.md | Traceability Markdown only | Markdown/path self-check |
| P0086 | CODEX-0661-06-DATA-EVENTING-d4c088ceeb | primary-implementation repaired | crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs.rs; crates/trpg-data-eventing/tests/adr_0002_event_sourcing_cqrs_contract_tests.rs | Add current-safe Rust module, lib registration, and target contract tests only | adr_0002_event_sourcing_cqrs_contract_tests plus S03 data-eventing gates |
| P0087 | CODEX-0662-06-DATA-EVENTING-ad325e6c77 | supplemental-requirement | primary owner CODEX-0586-06-DATA-EVENTING-5d2fcb8dee | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0088 | CODEX-0663-06-DATA-EVENTING-80272d7032 | primary-implementation repaired | crates/trpg-data-eventing/src/adr_0005_postgres_pgvector.rs; crates/trpg-data-eventing/tests/adr_0005_postgres_pgvector_contract_tests.rs | Add current-safe Rust module, lib registration, and target contract tests only | adr_0005_postgres_pgvector_contract_tests plus S03 data-eventing gates |
| P0089 | CODEX-0664-06-DATA-EVENTING-2b1e211e5a | supplemental-requirement | primary owner CODEX-0627-06-DATA-EVENTING-e54d49d1d8 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0090 | CODEX-0665-06-DATA-EVENTING-3151bf167f | supplemental-requirement | primary owner CODEX-0606-06-DATA-EVENTING-96df5cfdb1 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0091 | CODEX-0666-06-DATA-EVENTING-bbd9b75f9a | supplemental-requirement | primary owner CODEX-0057-06-DATA-EVENTING-069ff7204e | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0092 | CODEX-0667-06-DATA-EVENTING-43e7cb1324 | supplemental-requirement | primary owner CODEX-0058-06-DATA-EVENTING-524ce53bca | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0094 | CODEX-0668-06-DATA-EVENTING-57480abead | supplemental-requirement | primary owner CODEX-0059-06-DATA-EVENTING-8ceec1d689 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0093 | CODEX-0669-06-DATA-EVENTING-1f17ddba1d | supplemental-requirement | primary owner CODEX-0060-06-DATA-EVENTING-34e6c845e3 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0095 | CODEX-0670-06-DATA-EVENTING-c94e521373 | supplemental-requirement | primary owner CODEX-0061-06-DATA-EVENTING-f0442479c9 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0096 | CODEX-0671-06-DATA-EVENTING-57c3ab732a | supplemental-requirement | primary owner CODEX-0062-06-DATA-EVENTING-09d943908d | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0097 | CODEX-0672-06-DATA-EVENTING-8181b22939 | supplemental-requirement | primary owner CODEX-0063-06-DATA-EVENTING-f6f824261f | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0098 | CODEX-0673-06-DATA-EVENTING-7435c774dc | supplemental-requirement | primary owner CODEX-0615-06-DATA-EVENTING-58af1867fc | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0099 | CODEX-0674-06-DATA-EVENTING-6ce57411c9 | supplemental-requirement | primary owner CODEX-0064-06-DATA-EVENTING-2031b0ef61 | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |
| P0100 | CODEX-0675-06-DATA-EVENTING-fdf57fdbbe | supplemental-requirement | primary owner CODEX-0628-06-DATA-EVENTING-14e037cb2a | No implementation output; prompt already records merge target | Covered by owner tests outside B027 |

## Minimal Repair Slice

1. Add the current-safe CODEX-0661 module and target contract test.
2. Add the current-safe CODEX-0663 module and target contract test.
3. Register both contracts through crates/trpg-data-eventing/src/lib.rs.
4. Preserve supplemental prompt boundaries and existing documentation traceability outputs.
5. Rerun S03 cargo gates and environment checks; record SQLx/Docker/pnpm applicability honestly.

## Non-Goals

- Do not start BATCH-028.
- Do not use source-archive paths as current outputs.
- Do not rename or remove earlier-prompt files owned by non-B027 rows outside this failed B027 scope.
- Do not create migrations, NATS subjects, workflow/API handlers, direct provider calls, or player-visible private fixture outputs.
