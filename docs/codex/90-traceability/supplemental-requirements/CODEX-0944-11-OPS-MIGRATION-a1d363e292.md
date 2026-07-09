# Supplemental Requirement: Upgrade Rollback Implementation

Prompt ID: `CODEX-0944-11-OPS-MIGRATION-a1d363e292`
Primary Prompt: `CODEX-0929-11-OPS-MIGRATION-02f99d0dd9`
Shared module: `ops_migration::upgrade_rollback_impl`
Prompt file: `codex-prompts/11-ops-migration/P0042.md`

## Merge Instructions

- Merge into the BATCH-043 primary `upgrade_rollback_impl` module.
- Preserve migration ledger, rollback plan, event store hash, and projection replay evidence.
- This supplemental prompt owns no separate Rust src/test output in BATCH-043.
