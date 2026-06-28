# TRPG_PLATFORM P2 DB — SourceKind / DB CHECK Alignment Repair

## 0. Purpose

This document is a persistent Codex-readable repair spec for the P2 B02 database build gate.

The current blocker is a storage/schema alignment bug:

- `rag_core::SourceKind` includes `KpPrivateModule` and `SystemInternal`.
- storage string conversion currently does not cover those variants.
- storage string parsing currently does not parse `kp_private_module` or `system_internal`.
- PostgreSQL CHECK constraints for `document_sources.source_kind` and `documents.source_kind` do not include those canonical values.
- As a result, `cargo sqlx prepare --check --workspace`, `cargo test -p storage`, and `cargo test --workspace` fail before DB-backed RLS proof can run.

This is a real P2 B02 storage/database contract failure, not an environment blocker.

## 1. Scope

Allowed in this repair session:

- `crates/storage/**`
- `migrations/**`
- storage tests and migration tests
- `docs/status/P2_DATABASE_STATUS.md` or `docs/status/P2_STATUS.md`
- this document and Codex prompts, if installing the repair docs

Not allowed in this repair session:

- changing `crates/rag_core::SourceKind` variant names merely to satisfy storage
- deleting `KpPrivateModule` or `SystemInternal`
- editing old migrations in place, unless the migration has not been committed and the repository policy explicitly allows it
- disabling SQLx checks
- weakening RLS policies
- changing app runtime `DATABASE_URL` to a superuser URL
- adding server API routes
- adding frontend UI
- implementing Rig / agent engine work

## 2. Canonical SourceKind string contract

`rag_core::SourceKind` is the canonical domain model. Storage and DB must conform to it.

Canonical write strings:

```text
SourceKind::OfficialSrd                 -> official_srd
SourceKind::OpenText                    -> open_text
SourceKind::UserProvidedText            -> user_provided_text
SourceKind::CampaignNotes               -> campaign_notes
SourceKind::CharacterSheet              -> character_sheet
SourceKind::ModulePrivateNotes          -> module_private_notes
SourceKind::KpPrivateModule             -> kp_private_module
SourceKind::CommercialAdapterMetadata   -> commercial_adapter_metadata
SourceKind::SystemInternal              -> system_internal
SourceKind::Unknown                     -> unknown
```

Legacy read aliases, if existing DB rows or older migrations used them:

```text
open_license       -> OpenText
user_upload        -> UserProvidedText
commercial_adapter -> CommercialAdapterMetadata
```

New writes should use canonical strings only. Legacy aliases may remain accepted by DB CHECK constraints to avoid breaking existing local databases and migration replay.

## 3. Storage code requirements

Codex must find the current equivalent of:

```text
source_kind_as_str
source_kind_from_str
source_kind row conversion
DocumentSource / Document row mappers
```

Then implement the following behavior:

1. `source_kind_as_str` must be exhaustive for every `rag_core::SourceKind` variant.
2. It must include:
   - `SourceKind::KpPrivateModule => "kp_private_module"`
   - `SourceKind::SystemInternal => "system_internal"`
3. Do not use a catch-all `_ => ...` in `source_kind_as_str`. A future new enum variant should trigger a compile failure, not silently serialize incorrectly.
4. `source_kind_from_str` must parse every canonical string.
5. `source_kind_from_str` should parse known legacy aliases if they already exist in migrations or seeded data.
6. Unknown DB strings should return the project’s typed storage/repository data error. Do not silently map arbitrary unknown DB text to `SourceKind::Unknown`; only the literal `"unknown"` maps to `SourceKind::Unknown`.
7. Add or update tests proving round-trip behavior.

Suggested unit tests or equivalent names:

```text
source_kind_as_str_covers_all_canonical_variants
source_kind_from_str_accepts_all_canonical_values
source_kind_from_str_accepts_legacy_aliases
source_kind_from_str_rejects_invalid_database_value
source_kind_round_trip_canonical_values
```

## 4. Additive migration requirements

Create a new additive migration after the latest migration. Do not rewrite already applied migrations.

The migration must drop and recreate these CHECK constraints:

```text
document_sources_source_kind_check
documents_source_kind_check
```

The new constraints must include all canonical values:

```text
official_srd
open_text
user_provided_text
campaign_notes
character_sheet
module_private_notes
kp_private_module
commercial_adapter_metadata
system_internal
unknown
```

To preserve migration compatibility, they may also include legacy aliases:

```text
open_license
user_upload
commercial_adapter
```

`documents.source_kind` may remain nullable if current schema allows null. If it is nullable, the CHECK must allow `source_kind IS NULL OR source_kind IN (...)`.

Recommended SQL shape:

```sql
ALTER TABLE document_sources DROP CONSTRAINT IF EXISTS document_sources_source_kind_check;

ALTER TABLE document_sources
  ADD CONSTRAINT document_sources_source_kind_check
  CHECK (source_kind IN (
    'official_srd',
    'open_license',
    'open_text',
    'user_upload',
    'user_provided_text',
    'campaign_notes',
    'character_sheet',
    'module_private_notes',
    'kp_private_module',
    'commercial_adapter',
    'commercial_adapter_metadata',
    'system_internal',
    'unknown'
  ));

ALTER TABLE documents DROP CONSTRAINT IF EXISTS documents_source_kind_check;

ALTER TABLE documents
  ADD CONSTRAINT documents_source_kind_check
  CHECK (
    source_kind IS NULL
    OR source_kind IN (
      'official_srd',
      'open_license',
      'open_text',
      'user_upload',
      'user_provided_text',
      'campaign_notes',
      'character_sheet',
      'module_private_notes',
      'kp_private_module',
      'commercial_adapter',
      'commercial_adapter_metadata',
      'system_internal',
      'unknown'
    )
  );
```

If current schema names differ, Codex must adapt without changing the semantic contract.

## 5. DB-backed tests

When PostgreSQL/pgvector is available, prove the migration does what it claims.

Minimum DB-backed proof:

```text
source_kind_check_allows_kp_private_module
source_kind_check_allows_system_internal
source_kind_check_rejects_invalid_value
```

If the project’s storage tests already insert `DocumentSource` or `Document` through repository APIs, prefer repository-level tests. If direct SQL is simpler, keep it scoped to migration/schema proof.

Do not run DB proof with `DATABASE_URL=postgres://postgres:...` for ordinary runtime/RLS checks. Use admin/migrator URL only for migration/bootstrap, and ordinary app URL for runtime/RLS proof.

## 6. Required command sequence

PowerShell-friendly command sequence:

```powershell
git status --short
git diff --stat
git diff --name-status

cargo fmt --all --check
cargo check --workspace
cargo test -p rag_core
cargo test -p storage

# If local DB environment is installed:
. .\scripts\dev\db\env.ps1

cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1

cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

If DB is unavailable, do not call the result PASS. Report it as environment-blocked for DB-backed proof, while still reporting Rust compile/test status honestly.

## 7. Acceptance criteria

PASS requires all of the following:

- `cargo check --workspace` no longer fails because `SourceKind::KpPrivateModule` or `SourceKind::SystemInternal` are unmapped.
- `source_kind_as_str` writes `kp_private_module` and `system_internal`.
- `source_kind_from_str` reads `kp_private_module` and `system_internal`.
- tests cover all canonical variants.
- an additive migration updates both `document_sources_source_kind_check` and `documents_source_kind_check`.
- DB CHECK constraints include both `kp_private_module` and `system_internal`.
- SQLx prepare succeeds when DB env is available.
- storage tests pass, or any remaining failure is a new explicit issue unrelated to SourceKind alignment.
- no API/UI/Rig work is introduced.

FAIL if:

- storage still has incomplete enum matches.
- DB constraints still reject canonical SourceKind values.
- Codex edits old committed migrations instead of adding a migration.
- runtime `DATABASE_URL` is changed to a superuser URL.
- RLS is disabled or weakened to get tests green.
- API/frontend/agent work is mixed into this repair.

CONDITIONAL PASS is allowed only if static/compile/tests pass and the only missing evidence is an unavailable local DB target. It must list the exact DB commands not run and why.
