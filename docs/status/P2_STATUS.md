# P2 Status

## Overall

- Status: IN_REPAIR
- Last updated: 2026-06-29
- Current batch: P2 B01 — RAG Domain and Schema Contracts
- Can proceed to next batch: conditional

## B01 scope evidence

- Allowed areas changed: `crates/rag_core/**`, `docs/status/P2_STATUS.md`, `docs/p2/**`, `prompts/codex/**`
- B01 allowed summary: RAG domain contracts, deterministic chunk/hash/embed/retrieval tests, B01 repair policy docs and prompts.
- Disallowed areas changed: none in current diff
- Storage changes in B01 diff: removed
- Deferred patch path: `E:\_trpg_codex_deferred\P2_B02_storage_worktree_deferred.patch`
- Deferred index patch path: `E:\_trpg_codex_deferred\P2_B02_storage_index_deferred.patch`

## Commands

| Command | Result | Notes |
| --- | --- | --- |
| `git status --short` | PASS | Initial status showed `crates/storage/src/lib.rs`; after defer/restore it no longer shows storage. |
| `git branch --show-current` | PASS | `main` |
| `git diff --stat` | PASS | Current tracked diff only shows `crates/rag_core/src/lib.rs`. |
| `git diff --name-status` | PASS | Current tracked diff only shows `M crates/rag_core/src/lib.rs`. |
| `cargo metadata --no-deps` | PASS | Workspace metadata resolved. |
| `cargo fmt --all --check` | PASS | No formatting changes required. |
| `cargo check -p rag_core` | PASS | Completed successfully. |
| `cargo test -p rag_core` | PASS | 20 passed. |
| `cargo check -p document_ingestor` | PASS | Completed successfully; crate exists but is not in current B01 diff. |
| `cargo test -p document_ingestor` | PASS | 2 passed; crate exists but is not in current B01 diff. |
| `cargo test --workspace` | BLOCKED | Not run: `TRPG_TEST_MIGRATOR_DATABASE_URL` and `TRPG_DATABASE_ADMIN_URL` are missing. This is a DB test environment blocker, not proof that full B01 workspace tests pass. |

## Deferred to B02

- Align `crates/storage` ingest job status mapping to canonical `rag_core::IngestJobStatus`.
- Verify DB CHECK constraint values match the canonical ingest job status set, including denied terminal semantics.
- Run DB-backed storage migration/RLS tests using a migrator/admin URL for bootstrap and ordinary app URL for runtime/RLS proof.

## Known blockers

- Full workspace DB-sensitive gate is blocked by missing `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL`.
