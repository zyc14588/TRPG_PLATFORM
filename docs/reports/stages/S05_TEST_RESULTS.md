# S05 Test Results - BATCH-021

Stage: S05 - Ruleset COC7
Batch: BATCH-021-05-ruleset-coc7
Evidence date: 2026-07-05
Repair scope: tests and evidence only. No product implementation code was changed in this repair.

## Command Results

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Formatting check passed after rustfmt was applied to the new test file. |
| `cargo check -p trpg-ruleset-coc7` | PASS | COC7 ruleset crate compiled successfully. |
| `cargo test -p trpg-ruleset-coc7 --test s05_fixture_acceptance_contract_tests --all-features` | PASS | 4/4 S05 fixture acceptance tests passed. |
| `cargo test -p trpg-ruleset-coc7 --all-features` | PASS | COC7 package tests passed, including 30 integration tests. |
| `cargo test -p trpg-ruleset-coc7 dice` | PASS | 4 matching dice-related tests passed, including the S05 dice fixture assertion. |
| `cargo test -p trpg-ruleset-coc7 sanity` | PASS | 2 matching sanity tests passed. |
| `cargo test --workspace` | PASS | Workspace tests passed, including the new S05 fixture gate. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | Workspace clippy passed with warnings denied. |
| `rg -n -i "openai|ollama|llama|chat\.completions|responses\.create|direct llm|provider call" crates\trpg-ruleset-coc7` | PASS | No direct LLM/provider call matches were found in the COC7 ruleset crate. |
| `rg --files -g package.json -g pnpm-lock.yaml -g pnpm-workspace.yaml` | N/A | No pnpm/package manifest files are present. |
| `rg --files -g Dockerfile -g docker-compose.yml -g docker-compose.yaml` | N/A | No Docker manifest files are present. |

Observed non-failing cargo note: the workspace emitted `warn: could not canonicalize path C:\Users\zyc14588`. It did not fail any command.

## S05 Fixture Test Names

The current S05 fixture acceptance test contains and passed these tests:

- `s05_fixture_files_are_bound_to_current_acceptance_gate`
- `s05_character_and_dice_fixtures_map_to_ruleset_assertions`
- `s05_san_combat_chase_and_clue_fixtures_map_to_evented_assertions`
- `s05_visibility_provenance_and_private_leakage_assertions_are_event_bound`

## Covered Fixture Files

- `fixtures/stages/S05_stage_acceptance_fixture.v1.json.md`
- `fixtures/stages/detailed/S05_coc7_roll_san_combat_chase_expected.current.json.md`
- `fixtures/rules/coc7_character_creation_review.v1.json.md`
- `fixtures/rules/coc7_dice_matrix.v1.json.md`
- `fixtures/rules/coc7_san_combat_chase_flow.v1.json.md`
- `test-data/dice_san_combat_chase_cases.md`

Fixture repair notes:

- `coc7_character_creation_review.v1.json.md` now expects `Move: 7`, matching existing COC7 derivation for STR 45 / DEX 50 / SIZ 60.
- `S05_coc7_roll_san_combat_chase_expected.current.json.md` now names the current package in `automation_target`: `trpg-ruleset-coc7`.

## Conclusion

Current S05/BATCH-021 cargo fmt/check/test/clippy and fixture evidence is PASS, with pnpm and docker explicitly marked N/A for this repository.
