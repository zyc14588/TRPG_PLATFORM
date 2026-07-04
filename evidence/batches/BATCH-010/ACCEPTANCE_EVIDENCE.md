# BATCH-010 Acceptance Evidence

Batch: BATCH-010-02-domain-core - Strict Governance Final  
Date: 2026-07-04  

## Inputs Read

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B010.md`
- S02 stage START, TEST_PLAN, TEST_DATA, ACCEPTANCE, and REPAIR prompts
- All 25 B010 per-file prompts

## Scope Decision

- The user-supplied primary count of 0 conflicted with B010 and the normalized/current-safe maps.
- Repository authority identified 5 primary implementation prompts and 20 documentation/supplemental prompts.
- Implementation was limited to B010 current-safe outputs plus required `trpg-domain-core` module exports.
- `source-archive/**` was not used as an executable prompt source.

## Files Changed

### Rust modules

- `crates/trpg-domain-core/src/lib.rs`
- `crates/trpg-domain-core/src/adr_0003_authority_contract.rs`
- `crates/trpg-domain-core/src/character_combat_san_chase.rs`
- `crates/trpg-domain-core/src/event_sourcing_projection.rs`
- `crates/trpg-domain-core/src/investigation_clue_npc_time.rs`
- `crates/trpg-domain-core/src/rule_runtime_coc7.rs`

### Rust tests

- `crates/trpg-domain-core/tests/adr_0003_authority_contract_contract_tests.rs`
- `crates/trpg-domain-core/tests/character_combat_san_chase_contract_tests.rs`
- `crates/trpg-domain-core/tests/event_sourcing_projection_contract_tests.rs`
- `crates/trpg-domain-core/tests/investigation_clue_npc_time_contract_tests.rs`
- `crates/trpg-domain-core/tests/rule_runtime_coc7_contract_tests.rs`

### Traceability and evidence

- `docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_command_cqrs.md`
- `docs/codex/02-domain-core/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_authority_contract.md`
- `docs/codex/02-domain-core/source_processing_record_docs_implementation_09_security_governance_visibility_enforcement_points.md`
- `docs/codex/02-domain-core/source_processing_record_docs_implementation_10_testing_quality_visibility_leakage_tests.md`
- `evidence/batches/BATCH-010/BATCH_WORK_PLAN.md`
- `evidence/batches/BATCH-010/TEST_RESULTS.md`
- `evidence/batches/BATCH-010/ACCEPTANCE_EVIDENCE.md`

## Acceptance Matrix

| Requirement | Evidence |
|---|---|
| Batch work plan maps prompt ID, target, allowed range, and tests | `evidence/batches/BATCH-010/BATCH_WORK_PLAN.md` |
| Current-safe primary output names are present | New flat modules and tests use B010 declared target names |
| Supplemental prompts do not create Rust outputs | Supplemental rows recorded as requirement-only in work plan |
| Authority Contract remains immutable and fork-only | `adr_0003_authority_contract_contract_tests` |
| HUMAN_KP / AI_KP authority violations append no events | New authority, character/combat/SAN/chase, investigation, rule runtime, and projection tests |
| Direct agent formal write appends no event | `adr_0003_authority_contract_blocks_direct_agent_write_without_event` |
| Formal writes use command/event-store path | New modules call existing `submit_domain_command` or canon event append helpers |
| Visibility and Fact Provenance survive event replay | New replay tests across all 5 primary outputs |
| Stage checks run after minimal checks | `evidence/batches/BATCH-010/TEST_RESULTS.md` |

## New Public API

- `domain_core::adr_0003_authority_contract`
- `domain_core::character_combat_san_chase`
- `domain_core::event_sourcing_projection`
- `domain_core::investigation_clue_npc_time`
- `domain_core::rule_runtime_coc7`

No migrations, API handlers, NATS subjects, metric labels, or LLM/provider integrations were added.

## Residual Risks

- Default parallel `cargo test -p trpg-domain-core --all-features` hit Windows/MSVC `LNK1104` output-exe open errors. The same suite passed with `--jobs 1`, so this is recorded as an environment/linker concurrency risk rather than a code failure.
- Existing older `_impl` and historical-named modules remain in the crate for compatibility; B010 added current-safe modules and did not delete prior batch artifacts.

## Next Batch Handoff

- Use the current-safe module names added here for future domain-core references.
- If a later cleanup batch is assigned, it can decide whether old compatibility module names should become re-export shims or be removed with migration notes.
- Keep serial linking (`--jobs 1`) available for Windows full-suite verification when many test binaries are built at once.
