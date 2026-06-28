# Ingest Job Status Single Source Policy

## Decision

`rag_core::IngestJobStatus` is the canonical semantic source for P2 ingest job status.

Storage must not define an independent semantic enum with overlapping meanings. Storage may define one of the following only if needed:

1. A type alias for backward compatibility:

```rust
pub type RagIngestJobStatus = rag_core::IngestJobStatus;
```

2. A narrow SQL adapter that converts canonical values to and from database strings:

```rust
fn ingest_status_to_db(status: rag_core::IngestJobStatus) -> &'static str;
fn ingest_status_from_db(value: &str) -> Result<rag_core::IngestJobStatus, StorageError>;
```

The adapter must be explicit, tested, and incapable of silently mapping unknown database values to a safe-looking status.

## Required status alignment

B02 must inspect the current canonical domain enum and the current database CHECK constraints, then make them match.

The canonical set must at minimum support the P2 lifecycle semantics required by the acceptance matrix:

- claimed or queued/claimed equivalent;
- parsing or processing equivalent;
- embedding or indexing equivalent;
- indexed/completed equivalent;
- pending review;
- denied;
- failed.

If the actual project uses different public names, B02 must document the mapping in `docs/status/P2_STATUS.md` and keep database strings, repository conversion, and tests aligned.

## Denied semantics

`denied` is not merely an error string. It is a terminal review/license outcome. A denied source/job must not create ordinary searchable chunks or embeddings. Ordinary retrieval must not see denied data.

## Database policy

Migrations must be additive. Do not edit already-applied migration files to fix a CHECK constraint. Add a new migration that:

1. drops the old named CHECK constraint if needed;
2. creates a new named CHECK constraint that matches canonical status strings;
3. preserves existing rows or explicitly transforms them before applying the constraint;
4. is safe on a fresh database and on an already-migrated database.

## Required B02 tests

B02 should add or update tests equivalent to:

```text
ingest_status_domain_db_values_are_aligned
denied_ingest_job_status_is_persistable
denied_license_is_not_indexed
migration_fresh_install_and_rerun_idempotence
ordinary_app_role_is_not_superuser_or_bypassrls
```

If DB-backed tests run migrations, they must use a migrator/admin URL such as `TRPG_TEST_MIGRATOR_DATABASE_URL` or `TRPG_DATABASE_ADMIN_URL`. They must not set runtime `DATABASE_URL` to `postgres`.

## Acceptance rule

B01 may record this as a deferred B02 item. B02 cannot pass while `crates/storage` and `crates/rag_core` maintain independent ingest job status semantics.
