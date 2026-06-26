# Phase 1A Report

Status: implemented; waiting for human review.

Scope completed:

- Identity/Auth domain with Magic Link and OIDC ports.
- MockAuthProvider and DevelopmentAuthProvider with no real network calls.
- AccessToken and RefreshSession domain models.
- Refresh session rotation, revocation, expiration, and reuse detection logic.
- RoomRole, VisibilityScope, RoomPrivacyMode, Room, RoomMember, and RoomInvite domain models.
- Repository traits with transaction boundary support.
- PostgreSQL repository implementation for users, rooms, members, invites, refresh sessions, audit logs, and idempotency keys.
- SQLx migration for Phase 1A auth/session/invite/idempotency tables and RLS policies.
- Application-layer ABAC and visibility projection helpers.
- PostgreSQL RLS context helpers and deny-by-default policies.
- AuditLog success/failure persistence.
- IdempotencyKey storage and duplicate/conflict checks.
- Database integration tests.

Required source reread:

- Read: `AGENTS.md`, `DECISIONS.md`, `MANIFEST.md`, `docs/PRODUCT_SYSTEM_DESIGN.md`, `docs/BACKEND_ARCHITECTURE.md`, `docs/UI_UX_SPEC.md`, `docs/IMPLEMENTATION_HANDBOOK.md`, `docs/status/PHASE_0_REPORT.md`, `prompts/01_FOUNDATION.md`, `docs/adr/0001-deployment-and-privacy.md`, `docs/adr/0005-concurrency-and-crdt.md`.
- Missing: `docs/plans/PHASE_1_IDENTITY_ROOM_CORE.md` is not present in the repository.

Modified files:

- `TODO.md`
- `Cargo.lock`
- `crates/auth/Cargo.toml`
- `crates/auth/src/lib.rs`
- `crates/storage/Cargo.toml`
- `crates/storage/src/lib.rs`
- `migrations/20260626010000_phase_1a_identity_room.sql`
- `migrations/20260626011000_phase_1a_rls_membership_fix.sql`
- `docs/status/PHASE_1A_REPORT.md`

Migrations:

- `20260626000000_phase_0_initial.sql`
- `20260626010000_phase_1a_identity_room.sql`
- `20260626011000_phase_1a_rls_membership_fix.sql`

New dependencies:

- No new third-party crates were added to the workspace.
- Existing workspace dependencies were wired into `auth` and `storage`: `async-trait`, `serde_json`, `thiserror`, `uuid`, `tokio` for tests, and local crate dependencies on `auth`/`game_core`.

Tests added:

- RoomRole permission matrix.
- VisibilityScope projection matrix.
- Refresh session rotation.
- Refresh token reuse/revocation.
- Persisted refresh token reuse/revocation after repository lookup by old token hash.
- Magic Link mock provider verification.
- OIDC mock provider exchange.
- Invitation lifecycle.
- Duplicate idempotency key.
- Cross-room repository isolation.
- PostgreSQL RLS non-member rejection.
- Owner/KP/PL/spectator permission differences.
- Audit record success and failure paths.
- Migration fresh-install and re-run/idempotence check.
- Pure backend vertical flow without HTTP.

Stop-point review:

- A1 domain + migration compile/run: passed by `cargo check --workspace --all-targets` and `cargo sqlx migrate run` on an isolated fresh PostgreSQL database.
- A2 repository integration + RLS no false allow: passed by `cargo test --workspace` with `DATABASE_URL`; repository isolation and non-member RLS rejection tests executed against PostgreSQL.
- A3 pure backend vertical flow, no HTTP: passed by `pure_backend_vertical_flow_without_http`, covering mock Magic Link, user/session/room/invite/member/idempotency/audit persistence, and member RLS visibility without adding handlers.

Security check:

- No API keys, OIDC secrets, or mail credentials were added.
- Magic Link and OIDC providers are ports with mock/development implementations only.
- `local_only` denies cloud-provider authorization at ABAC level.
- KP-only visibility is denied for PL, observer, and public screen roles in application ABAC and RLS.
- RLS membership policies require a real `room_members` row matching `app.user_id`, `app.room_id`, and `app.room_role`; role GUC spoofing alone is insufficient.
- Idempotency keys store request hashes and response payloads, not raw secrets.
- Refresh sessions store token hashes, not raw refresh tokens.
- Room invites require a caller-provided token hash; repositories no longer derive predictable hashes from invite ids.
- Repository database errors are sanitized before crossing the repository boundary.
- PostgreSQL retryable SQLSTATE classification remains centralized in `game_core`.

Remaining stubs:

- No formal HTTP handlers were added in Phase 1A.
- Real email delivery and real OIDC exchange remain ports only.
- JWT signing/verification middleware is deferred to Phase 1B.
- OpenAPI updates for formal auth routes are deferred because this batch does not add handlers.

Verification:

- `git diff --check`: passed; Windows line-ending warnings only.
- `cargo fmt --all --check`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
- `docker compose up -d postgres`: passed after temporary local placeholder env vars for Compose-required MinIO/Grafana settings.
- `cargo sqlx migrate run`: passed against an isolated temporary pgvector PostgreSQL container to prove fresh-install behavior without relying on persistent compose volume state.
- `cargo test --workspace` with `DATABASE_URL` pointed at the isolated pgvector container: passed; database integration tests executed.
- `cargo sqlx prepare --check --workspace`: passed against the isolated pgvector container.

Stopped before Phase 1B.
