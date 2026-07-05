# S05 Traceability - BATCH-021

Stage: S05 - Ruleset COC7
Batch: BATCH-021-05-ruleset-coc7
Evidence date: 2026-07-05
Repair scope: tests and evidence only. No product implementation code was changed in this repair.

## Inputs Reconciled

- `AGENTS.md`
- `batches/B021.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `stages/s05-ruleset-coc7-engine/START_PROMPT.md`
- `stages/s05-ruleset-coc7-engine/TEST_PLAN.md`
- `stages/s05-ruleset-coc7-engine/TEST_DATA.md`
- `stages/s05-ruleset-coc7-engine/ACCEPTANCE_PROMPT.md`
- `evidence/batches/BATCH-021/prompt-traceability.md`
- `evidence/batches/BATCH-021/test-output.txt`
- `evidence/batches/BATCH-021/acceptance-test-output.txt`
- `evidence/batches/BATCH-021/acceptance-report.md`
- `evidence/stages/S05/coc7-rules-tests.txt`
- `evidence/stages/S05/dice-audit-tests.txt`

## Prompt Row Traceability

The authoritative per-row S05/B021 acceptance table is `evidence/batches/BATCH-021/prompt-traceability.md`.

- Prompt row result: 25/25 PASS.
- Primary implementation prompts: 13/13 PASS.
- Supplemental requirement prompts: 11/11 PASS.
- Docs-governance prompts: 1/1 PASS.
- Each primary prompt has implementation evidence and at least one targeted contract test.
- Supplemental prompts are merged into their primary current-safe targets and do not create standalone implementation scope.

## Current-safe Output Traceability

S05 evidence uses current-safe COC7 module/output names:

- `character_combat_san_chase`
- `chase_state_machine`
- `combat_state_machine`
- `dice_roll_contract`
- `investigation_clue_npc_time`
- `rules_coc7`
- `sanity_madness_state_machine`
- `coc7_rules_engine`
- `san`
- `npc`
- `rule_runtime_coc7_ruleset_pack`
- `rule_runtime_coc7`
- `coc7_rule_runtime`
- `docs/codex/05-ruleset-coc7/m_05_ruleset_coc7.md`

No source archive path, source hash fragment, or historical V3/V4/V5/V6 token is promoted as a current implementation, event, subject, workflow, metric, or test name.

## S05 Fixture Traceability

| Fixture category | Covered values | Evidence |
| --- | --- | --- |
| Stage acceptance fixture | S05 stage id and required stage report file names | `s05_fixture_files_are_bound_to_current_acceptance_gate`. |
| Detailed expected events | `DiceRolled`, `SkillCheckResolved`, `SanityLossApplied`, `CombatStateUpdated`, `ChaseSegmentResolved` | S05 fixture gate plus COC7 package contract tests. |
| Detailed expected errors | `CLIENT_FORMAL_DICE_FORBIDDEN`, `AI_DICE_FABRICATION_FORBIDDEN`, `STATE_CHANGE_WITHOUT_EVENT` | Direct-agent write denial and server-dice assertions. |
| Character creation | Derived HP, MP, SAN, Luck, damage bonus, build, and Move 7 from the same fixture attributes | `s05_character_and_dice_fixtures_map_to_ruleset_assertions`. |
| Dice matrix | Success level, bonus die, penalty die, event recording | `s05_character_and_dice_fixtures_map_to_ruleset_assertions`. |
| SAN/combat/chase flow | SAN loss, combat HP transition, chase lead/caught transition | `s05_san_combat_chase_and_clue_fixtures_map_to_evented_assertions`. |
| Core clue fail-forward | Failed core clue resolves as revealed-with-cost | `s05_san_combat_chase_and_clue_fixtures_map_to_evented_assertions`. |
| Visibility/provenance | `ruleset_id`, `visibility_label`, `provenance_kind`, public replay filtering | `s05_visibility_provenance_and_private_leakage_assertions_are_event_bound`. |
| Keeper-only leakage | Keeper-only NPC secret is not public-visible | `s05_visibility_provenance_and_private_leakage_assertions_are_event_bound`. |

## Governance Boundary Traceability

- Authority Contract immutability: PASS. Tests use existing governed contracts and do not mutate Authority Contract.
- Agent Gateway-only AI access: PASS. No direct LLM/provider path is introduced by S05 repair.
- Tool Permission Gate: PASS. Direct-agent formal state write is rejected.
- Formal state boundary: PASS. Formal COC7 results are recorded through Event Store append helpers.
- Visibility Label Propagation: PASS. Event payload and replay visibility are asserted.
- Fact Provenance: PASS. `ProvenanceKind::RulesEngineDecision` is asserted.
- Event Log boundary: PASS. SAN, dice, combat, chase, and clue outcomes are recorded as events.
- V1 Acceptance boundary: PASS. The repair adds fixture tests and evidence only, with no product feature expansion.

## Test Result Traceability

Current results are recorded in `docs/reports/stages/S05_TEST_RESULTS.md`:

- `cargo fmt --all -- --check`: PASS.
- `cargo check -p trpg-ruleset-coc7`: PASS.
- `cargo test -p trpg-ruleset-coc7 --test s05_fixture_acceptance_contract_tests --all-features`: PASS.
- `cargo test -p trpg-ruleset-coc7 --all-features`: PASS.
- `cargo test -p trpg-ruleset-coc7 dice`: PASS.
- `cargo test -p trpg-ruleset-coc7 sanity`: PASS.
- `cargo test --workspace`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.

The detailed S05 fixture automation target now uses the current package name `trpg-ruleset-coc7`.

## pnpm and Docker Traceability

- `pnpm`: N/A because no `package.json`, `pnpm-lock.yaml`, or `pnpm-workspace.yaml` files are present.
- `docker`: N/A because no `Dockerfile`, `docker-compose.yml`, or `docker-compose.yaml` files are present.

## Conclusion

S05 traceability is complete for BATCH-021: 25/25 prompt rows accounted, required fixtures loaded and mapped to concrete assertions, governance boundaries preserved, and cargo verification results are recorded.
