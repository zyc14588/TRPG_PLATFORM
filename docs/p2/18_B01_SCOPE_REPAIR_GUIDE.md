# P2 B01 Scope Repair Guide

## Purpose

This document exists because B01 acceptance found that the B01 workspace touched `crates/storage/src/lib.rs`, while B01 is supposed to be limited to the RAG domain model and necessary `document_ingestor` compatibility work.

B01 must not fix storage, migrations, RLS, SQLx migration harnesses, or repository mappings. Those belong to B02 or a dedicated DB/test-harness repair branch.

## Current blocker classes

### B01 hard blockers

1. `crates/storage/**` modified in a B01 branch.
2. `docs/status/P2_STATUS.md` missing.
3. B01 acceptance evidence missing command results, deferred rationale, or status-model mapping notes.

### Environment / harness blockers

1. `cargo test --workspace` may fail if `migration_fresh_install_and_rerun_idempotence` requires a migrator/admin URL.
2. The correct fix is not to set runtime `DATABASE_URL` to `postgres`.
3. Migration/bootstrap tests must use `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL`; runtime/repository/RLS proof must continue to use ordinary app `DATABASE_URL`.

### B02 deferred blockers

1. `crates/rag_core` and `crates/storage` must not keep independent ingest job status semantics.
2. B02 must make `rag_core::IngestJobStatus` the single semantic source, with storage doing explicit DB string conversion or a type alias/newtype adapter.
3. DB CHECK constraints must match the canonical status set and include terminal denied semantics where the domain model requires it.

## B01 allowed scope

B01 repair may modify:

```text
crates/rag_core/**
crates/document_ingestor/**            # only compatibility with rag_core domain types
docs/status/P2_STATUS.md
docs/p2/**                            # only status/guide/acceptance documentation
prompts/codex/**                      # only Codex control prompts, if this repair pack is being installed
```

B01 repair must not modify:

```text
crates/storage/**
migrations/**
crates/server/**
apps/web/**
schemas/openapi.json
.env.example
Cargo.lock, unless a B01 dependency change is explicitly required and justified
```

## Required B01 repair outcome

Before B01 can be accepted:

1. `git diff --name-status` must not show `crates/storage/**` as modified in the B01 change set.
2. `docs/status/P2_STATUS.md` must exist.
3. `docs/status/P2_STATUS.md` must record:
   - current batch: P2 B01 Domain;
   - B01 evidence commands;
   - whether `cargo test --workspace` ran;
   - if workspace test did not run or failed only because no migrator/admin URL exists, the exact environment blocker;
   - deferred B02 item: align storage ingest job status with `rag_core::IngestJobStatus`.
4. `cargo test -p rag_core` must pass.
5. `cargo test -p document_ingestor` should pass if that crate exists and is part of B01 compatibility work.
6. `cargo test --workspace` should run only when the required DB/migrator environment is available. If unavailable, do not claim full workspace PASS.

## Safe handling of accidental storage diff

If `crates/storage/src/lib.rs` has only uncommitted B01-local changes:

```powershell
$deferredDir = Join-Path (Split-Path (Get-Location) -Parent) "_trpg_codex_deferred"
New-Item -ItemType Directory -Force $deferredDir | Out-Null

git diff -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $deferredDir "P2_B02_storage_worktree_deferred.patch") -Encoding utf8
git diff --cached -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $deferredDir "P2_B02_storage_index_deferred.patch") -Encoding utf8

git restore --staged -- crates/storage/src/lib.rs
git restore -- crates/storage/src/lib.rs
```

Do not add those patch files to the B01 commit. The final B01 report should mention the outside-repo patch path so the owner can reuse it in B02 if desired.

If storage changes are already committed in the B01 branch, do not silently rewrite history. Create a clean B01 branch from the correct base or perform an explicit owner-approved reverse patch.

## B01 status file skeleton

If `docs/status/P2_STATUS.md` does not exist, create it from this skeleton:

```markdown
# P2 Status

## Overall

- Current batch: P2 B01 — RAG Domain and Schema Contracts
- Current result: IN_REPAIR
- Last updated by: Codex

## B01 scope evidence

- Allowed areas changed:
- Disallowed areas changed:
- Storage changes in B01 diff: none / removed / blocked

## Commands

| Command | Result | Notes |
| --- | --- | --- |
| cargo fmt --all --check |  |  |
| cargo check -p rag_core |  |  |
| cargo test -p rag_core |  |  |
| cargo test -p document_ingestor |  |  |
| cargo test --workspace |  | Requires `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL` if storage DB tests run. |

## Deferred to B02

- Align `crates/storage` ingest job status mapping to canonical `rag_core::IngestJobStatus`.
- Verify DB CHECK constraint values match the canonical ingest job status set.
- Run DB-backed storage migration/RLS tests using a migrator/admin URL for bootstrap and ordinary app URL for runtime/RLS proof.

## Known blockers

- None / list exact blocker.
```
