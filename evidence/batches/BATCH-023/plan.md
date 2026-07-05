# BATCH-023-05-ruleset-coc7 Strict Governance Final Plan

Baseline date: 2026-07-05

## Scope Guard

- Batch file: `batches/B023.md`
- Stage: `stages/s05-ruleset-coc7-engine`
- Declared prompt rows: 15
- Current batch primary implementation rows: 0
- Current-safe target crate: `trpg-ruleset-coc7`
- `source-archive/**` is provenance only. Historical V3/V4/V5/V6 names, hashes, and source paths must not become current modules, migrations, events, metrics, tests, workflows, or outputs.
- Documentation-or-traceability rows may create only Markdown traceability records.
- Supplemental-requirement rows may create only supplemental requirement Markdown and merge instructions for their primary prompt. They do not own Rust src/test output.
- No implementation, migration, API handler, NATS subject, workflow, event schema, metric, provider, database path, or Authority Contract change is authorized in this batch.

## Prompt Mapping

| Prompt | Role | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|---|
| CODEX-0570 / P0047 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_rules_coc7.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0571 / P0051 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_05_ruleset_coc7_sanity_madness_state_machine.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0572 / P0049 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_rule_runtime_coc7.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0573 / P0052 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_character_combat_san_chase.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0574 / P0045 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_90_traceability_source_breakdown_domain_investigation_clue_npc_time.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0575 / P0056 | traceability | `docs/codex/05-ruleset-coc7/source_processing_record_docs_implementation_12_extension_sdk_ruleset_pack_sdk.md` | Markdown provenance record only | Prompt traceability check |
| CODEX-0576 / P0057 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0576-05-RULESET-COC7-c4db17f4ae.md` | Merge instructions for CODEX-0049 only | Supplemental boundary check |
| CODEX-0577 / P0058 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0577-05-RULESET-COC7-beeb6daa0d.md` | Merge instructions for CODEX-0050 only | Supplemental boundary check |
| CODEX-0578 / P0059 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0578-05-RULESET-COC7-67fad89a16.md` | Merge instructions for CODEX-0051 only | Supplemental boundary check |
| CODEX-0579 / P0060 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0579-05-RULESET-COC7-ad43ae3bfd.md` | Merge instructions for CODEX-0052 only | Supplemental boundary check |
| CODEX-0580 / P0061 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0580-05-RULESET-COC7-60e84aa01c.md` | Merge instructions for CODEX-0053 only | Supplemental boundary check |
| CODEX-0581 / P0062 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0581-05-RULESET-COC7-4272e1db6a.md` | Merge instructions for CODEX-0554 only | Supplemental boundary check |
| CODEX-0582 / P0063 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0582-05-RULESET-COC7-f9d12478d9.md` | Merge instructions for CODEX-0054 only | Supplemental boundary check |
| CODEX-0583 / P0064 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0583-05-RULESET-COC7-a3d270fccb.md` | Merge instructions for CODEX-0055 only | Supplemental boundary check |
| CODEX-0584 / P0065 | supplemental | `docs/codex/90-traceability/supplemental-requirements/CODEX-0584-05-RULESET-COC7-14b91df550.md` | Merge instructions for CODEX-0557 only | Supplemental boundary check |

## Test Plan

Minimal related checks:

- `rg "CODEX-057[0-9]|CODEX-058[0-4]" docs/codex/05-ruleset-coc7 docs/codex/90-traceability/supplemental-requirements evidence/batches/BATCH-023`
- `rg "crates/trpg-ruleset-coc7/(src|tests)|src/[A-Za-z0-9_/-]+\\.rs|tests/[A-Za-z0-9_/-]+\\.rs|migrations/[A-Za-z0-9_/-]+\\.sql" docs/codex/90-traceability/supplemental-requirements/CODEX-0576-05-RULESET-COC7-c4db17f4ae.md docs/codex/90-traceability/supplemental-requirements/CODEX-0577-05-RULESET-COC7-beeb6daa0d.md docs/codex/90-traceability/supplemental-requirements/CODEX-0578-05-RULESET-COC7-67fad89a16.md docs/codex/90-traceability/supplemental-requirements/CODEX-0579-05-RULESET-COC7-ad43ae3bfd.md docs/codex/90-traceability/supplemental-requirements/CODEX-0580-05-RULESET-COC7-60e84aa01c.md docs/codex/90-traceability/supplemental-requirements/CODEX-0581-05-RULESET-COC7-4272e1db6a.md docs/codex/90-traceability/supplemental-requirements/CODEX-0582-05-RULESET-COC7-f9d12478d9.md docs/codex/90-traceability/supplemental-requirements/CODEX-0583-05-RULESET-COC7-a3d270fccb.md docs/codex/90-traceability/supplemental-requirements/CODEX-0584-05-RULESET-COC7-14b91df550.md`
- `cargo check -p trpg-ruleset-coc7`

Stage checks:

- `cargo test -p trpg-ruleset-coc7 --all-features`
- `cargo test -p trpg-ruleset-coc7 dice`
- `cargo test -p trpg-ruleset-coc7 sanity`
- `cargo fmt --all -- --check`

## Execution Notes

- No new Rust tests are added because B023 contains no primary implementation prompt.
- Existing S05 tests remain the verification surface for primary-owned behavior.
- Supplemental test assertions are recorded for later primary-owned merge only.
