# BATCH-021 Handoff

## Completed

- Added current-safe `trpg-ruleset-coc7` crate to the workspace.
- Implemented governed COC7 ruleset modules for dice, SAN/madness, combat, chase, character derived stats, clue fail-forward, NPC visibility, rules metadata, rules engine routing, ruleset pack validation, and runtime governance.
- Added one or more contract tests for each primary prompt target.
- Added S05 module traceability document at `docs/codex/05-ruleset-coc7/m_05_ruleset_coc7.md`.
- Wrote batch plan, prompt traceability, test output, acceptance output, and acceptance report under `evidence/batches/BATCH-021/`.

## Tests

- `cargo check -p trpg-ruleset-coc7`
- `cargo test -p trpg-ruleset-coc7`
- `cargo fmt --all -- --check`
- `cargo test --workspace`

All commands passed.

## Unresolved Risk

- No unresolved blocker for BATCH-021 scope.
- Remaining deeper COC7 content tables or gameplay breadth should be handled only by a future mapped batch if one explicitly targets them.

## Next Batch Notes

- Treat `crates/trpg-ruleset-coc7` as the current-safe COC7 ruleset boundary.
- Reuse shared-kernel `AuthorityContract`, `CommandEnvelope`, `EventStore`, `Visibility`, and `FactProvenance` rather than adding parallel governance types.
- Keep supplemental prompts as constraints and test/traceability inputs unless a normalized map promotes them to primary outputs.
