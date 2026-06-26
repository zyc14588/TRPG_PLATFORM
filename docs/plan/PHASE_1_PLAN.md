# Phase 1 Plan - Identity, Authorization, and Room Core

> Status: tightened implementation plan. Do not implement from this document until the owner confirms the batch.

## Goal

Deliver one real vertical flow:

1. User logs in with a mock/development auth provider.
2. User creates a room.
3. User invites a second user.
4. Second user joins the room.
5. Members read role-appropriate room projections.
6. Non-members cannot infer whether the room exists.
7. The whole flow writes audit records.

## Hard Scope

Must do:

- Identity and auth domain.
- Magic Link and OIDC ports.
- Mock/Development Auth Provider.
- Access token and rotating refresh session.
- `RoomRole`, `VisibilityScope`, `RoomPrivacyMode`.
- Room, member, invite domain.
- Repositories.
- Additive migrations.
- Application ABAC plus PostgreSQL RLS.
- Audit.
- Idempotency.
- OpenAPI.
- Thin frontend login and room flow.
- Integration, security, and E2E tests.
- Phase 1 report.

Must not do:

- Repeat Phase 0 setup.
- RAG.
- LLM.
- Agent behavior.
- WebSocket replay.
- Tactical map.
- Final visual system.
- TTS.
- Creator images.
- Commercial rule text.
- Real email delivery.
- Real OIDC provider.
- Paid or external API dependency in CI.

## Phase 0 Baseline

Already complete and not to be repeated:

- Cargo workspace and crate skeletons.
- Next.js TypeScript shell.
- Axum `/healthz`, `/readyz`, and `/metrics`.
- Mock LLM, embedding, and image provider boundaries.
- Initial SQLx migration.
- Compose infrastructure for PostgreSQL/pgvector, Redis, MinIO, Prometheus, and Grafana.
- CI and baseline build/test commands.

Reusable existing pieces:

| Area | Existing item |
|---|---|
| Auth | `RoomRole`, `VisibilityScope`, `RoomPrivacyMode`, `AuthContext` |
| Game core | `ExpectedVersion`, `IdempotencyKey`, deadlock retry helpers |
| Storage | SQLx migrator |
| Database | `users`, `rooms`, `room_members`, `sessions`, `audit_logs`, RLS-enabled core tables |
| Providers | Mock model and image provider boundaries |

## Acceptance Summary

P1 is accepted only when all checks below pass.

| Check | Acceptance standard |
|---|---|
| No Phase 0 repeat | No workspace, crate, Next.js, Compose, or initial migration re-initialization. |
| No scope creep | No RAG, Agent behavior, WebSocket replay, map, TTS, Creator images, or commercial rule content. |
| CI isolation | CI uses only local Postgres and mock/development auth; no SMTP, OIDC, LLM, image, or paid API calls. |
| Mock auth | `DevelopmentAuthProvider` is explicit, test-covered, and disabled outside dev/test configuration. |
| ABAC plus RLS | Cross-room denial is tested once through API/application ABAC and once through direct database RLS policy. |
| Safe frontend DTOs | Frontend consumes only `PlayerSafeRoomDto` / `MemberSafeRoomDto` style projections, never raw entities. |
| OpenAPI | `/openapi.json` includes all Phase 1 auth and room routes. |
| Migrations | New schema is additive; Phase 0 migration is not edited. |
| Tests | Unit, repository integration, RLS, API security, frontend, and E2E tests cover the vertical flow. |
| Reports | Phase 1 status report records scope, commands, dependencies, skipped items, and residual risk. |
| Dependencies | Every new dependency has a concrete reason; no real OIDC SDK or SMTP dependency in P1. |
| Stop points | Each batch has a runnable stopping point before the next batch begins. |

## Existing Gaps

| Area | Gap |
|---|---|
| Auth | No users/auth identities, magic link store, token store, refresh rotation, or provider port implementation. |
| Rooms | No invite flow, join flow, safe projection, or room repository. |
| Authorization | `AuthContext::can_view` is a useful start but not complete ABAC. |
| RLS | Tables have RLS enabled/forced, but policies and request context are missing. |
| API | Only health/ready/metrics exist. |
| Frontend | Phase 0 placeholder only. |
| OpenAPI | No generated API contract. |
| Reports | Phase 1 report must be produced or updated as part of P1. |

## Phase 1A - Backend Core

Purpose: make the vertical flow work without HTTP.

### File Plan

| File | Change |
|---|---|
| `crates/auth/src/lib.rs` | Extend domain types for auth identities, sessions, magic links, OIDC port, and ABAC decisions. |
| `crates/auth/src/dev_provider.rs` | Add `DevelopmentAuthProvider`; it returns a mock delivery token for tests/dev only. |
| `crates/game_core/src/lib.rs` | Export room core types without replacing existing Phase 0 types. |
| `crates/game_core/src/room.rs` | Add `Room`, `RoomInvite`, `RoomProjection`, member-safe DTO domain types. |
| `crates/storage/src/lib.rs` | Export storage modules. |
| `crates/storage/src/rls.rs` | Set per-request DB context such as `app.user_id` and `app.room_id`. |
| `crates/storage/src/auth_repo.rs` | Store users, identities, magic links, access tokens, and refresh sessions. |
| `crates/storage/src/room_repo.rs` | Create room, invite member, accept invite, and load safe projections. |
| `crates/storage/src/idempotency_repo.rs` | Store request hash and replay duplicate responses. |
| `crates/storage/src/audit_repo.rs` | Append audit records for auth and room actions. |
| `crates/storage/tests/p1_identity_room_flow.rs` | Integration test for login, create room, invite, join, projection, audit. |
| `crates/storage/tests/p1_rls_isolation.rs` | Direct RLS test for cross-room denial. |
| `migrations/*_phase_1_identity_room_core.sql` | Add new tables, indexes, RLS helpers, and policies. |

### Migration Plan

Add only a new migration. Do not edit `migrations/20260626000000_phase_0_initial.sql`.

New tables:

- `auth_identities`
- `auth_magic_links`
- `auth_access_tokens`
- `auth_refresh_sessions`
- `room_invites`
- `idempotency_keys`

New policy/support objects:

- RLS helper functions for current user and current room context.
- `rooms` policies: members can read; authenticated users can create owned rooms.
- `room_members` policies: members can read safe membership data for their room.
- `room_invites` policies: inviter and intended invitee can access required invite data.
- `audit_logs` policies: server inserts; member reads only allowed audit projection if exposed.

### Phase 1A Stop Points

1. Migration applies cleanly.
2. Domain code compiles.
3. Repository integration test runs the full flow without HTTP.
4. RLS test proves another room's member cannot read the target room.
5. Audit rows exist for login, room creation, invite creation, invite accept, and denied access.

## Phase 1B - REST, OpenAPI, and Thin Frontend

Purpose: expose the same flow through REST and a small frontend.

### File Plan

| File | Change |
|---|---|
| `crates/server/src/lib.rs` | Wire PgPool/storage repositories and register auth/room routes. |
| `crates/server/src/error.rs` | Add unified `ApiError`; non-member room access returns indistinguishable 404. |
| `crates/server/src/extractors.rs` | Add auth extractor from HttpOnly cookie/session into `AuthContext`. |
| `crates/server/src/auth_api.rs` | Add magic-link request/consume, refresh, logout, and me routes. |
| `crates/server/src/rooms_api.rs` | Add create/list/get/invite/accept room routes. |
| `crates/server/src/openapi.rs` | Expose `/openapi.json`. |
| `crates/server/tests/p1_api_flow.rs` | HTTP test for the two-user vertical flow. |
| `crates/server/tests/p1_security.rs` | API tests for non-member 404 and cross-room ABAC. |
| `apps/web/src/lib/contracts.ts` | Add safe frontend DTOs only. |
| `apps/web/src/lib/api.ts` | Add typed fetch client consuming only safe DTOs. |
| `apps/web/src/app/login/page.tsx` | Add development magic-link login page. |
| `apps/web/src/app/rooms/page.tsx` | Add create room and room list thin page. |
| `apps/web/src/app/rooms/[roomId]/page.tsx` | Add member-safe room projection page. |
| `apps/web/src/app/invites/[token]/page.tsx` | Add invite accept page. |
| `apps/web/src/test/e2e/phase1-flow.spec.ts` | Add thin E2E for login, room creation, invite, join, projection. |
| `docs/status/PHASE_1_REPORT.md` | Record final P1 status and verification. |
| `schemas/openapi.json` | Store generated or checked OpenAPI contract if the repo keeps schema snapshots. |

### API Contract

Auth:

- `POST /api/auth/magic-link/request`
- `POST /api/auth/magic-link/consume`
- `POST /api/auth/refresh`
- `POST /api/auth/logout`
- `GET /api/auth/me`

Rooms:

- `POST /api/rooms`
- `GET /api/rooms`
- `GET /api/rooms/{room_id}`
- `POST /api/rooms/{room_id}/invites`
- `POST /api/invites/{token}/accept`

OpenAPI:

- `GET /openapi.json`

### Frontend DTO Rule

Frontend may consume only safe DTOs, for example:

- `CurrentUserDto`
- `PlayerSafeRoomDto`
- `MemberSafeRoomDto`
- `RoomInviteAcceptDto`

Frontend must not consume:

- raw database rows,
- repository structs,
- KP-only fields,
- internal audit payloads,
- token hashes,
- RLS context data.

### Phase 1B Stop Points

1. Auth REST tests pass with `DevelopmentAuthProvider`.
2. Room REST tests pass with safe DTOs.
3. Non-member API access cannot distinguish missing room from forbidden room.
4. `/openapi.json` includes all P1 routes.
5. Thin frontend E2E passes without real email or OIDC.
6. Phase 1 report exists and lists skipped out-of-scope systems.

## Test Matrix

| Layer | Tests |
|---|---|
| Unit | ABAC decisions, visibility projection, token hash helpers, idempotency request hash. |
| Storage integration | Full identity-room flow, duplicate idempotency keys, audit rows. |
| RLS | Same-room read allowed; cross-room read denied; non-member cannot infer target room. |
| API | Auth cookie/session flow, refresh rotation, logout revoke, create/list/get room, invite accept. |
| Security | Uniform 404 for non-member rooms, expired invite, duplicate invite accept, wrong invite user. |
| Frontend | API client DTO shape, login page, room list, room projection, invite accept. |
| E2E | Two development users complete the P1 vertical flow. |

## Verification Commands

Run all at the final P1 stop point:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check --workspace
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
```

CI must pass without:

- SMTP credentials,
- OIDC credentials,
- OpenAI API key,
- Ollama server,
- image provider,
- paid API.

## Dependency Plan

Allowed if needed:

| Dependency | Reason |
|---|---|
| `rand` | Generate unpredictable development magic-link/session tokens. |
| `sha2` | Store only token hashes. |
| `base64` | URL-safe token encoding. |
| `time` plus SQLx `time` feature | Session and invite expiry. |
| `utoipa` | OpenAPI generation with Rust DTOs. |
| `@playwright/test` | Real browser E2E for the thin P1 flow. |

Not allowed in P1:

- SMTP client.
- Real OIDC SDK.
- LLM SDK.
- WebSocket client/server implementation beyond existing Phase 0 surface.
- UI component library.

## Rollback Strategy

- Revert the P1 migration by adding a rollback/cleanup migration if needed; do not edit the Phase 0 migration.
- Remove route registration to disable P1 API while keeping health endpoints.
- Revert frontend P1 routes without touching the Phase 0 app shell history.
- Keep provider stubs and Agent/RAG crates unchanged.

## Files Explicitly Out of Scope

Do not modify unless a separate owner-approved task says so:

- `DECISIONS.md`
- `docs/status/PHASE_0_REPORT.md`
- `migrations/20260626000000_phase_0_initial.sql`
- RAG crates beyond imports required by existing build.
- Agent crates.
- WebSocket replay code.
- Map/audio/Creator image modules.
- Commercial rule content packages.

## Planned Stub Replacement

Replace only these Phase 0 placeholders:

- `server` ready DB status: replace `not_checked_phase_0` with a real DB check.
- Web placeholder page: replace with thin login/rooms flow.
- `backendPlaceholder` test: replace with safe API client tests.

Keep these stubs:

- Agent `phase0_not_implemented` markers.
- Mock LLM/embedding/image providers.
- Rule adapter text-free placeholders.

## Batch Order

| Batch | Deliverable | Stop point |
|---|---|---|
| A1 | Domain plus migration | Migration runs and domain tests pass. |
| A2 | Repositories plus RLS context | Storage integration flow passes. |
| A3 | ABAC/RLS/audit hardening | Cross-room tests and audit assertions pass. |
| B1 | REST API plus OpenAPI | API flow/security tests pass. |
| B2 | Thin frontend | Frontend tests and E2E pass. |
| B3 | Final report and full verification | All acceptance commands pass and Phase 1 report is complete. |

