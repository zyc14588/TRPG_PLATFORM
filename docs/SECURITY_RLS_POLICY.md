# Security and RLS Policy

## Authorization Model

The platform uses application-layer ABAC plus PostgreSQL RLS. Authorization decisions are deny-by-default. Every room, document, clue, memory, and Agent payload access must be scoped by actor, room role, visibility, and privacy mode.

## Visibility Rules

PL clients may receive only public rules, room rules, PL-visible clues, their own character-private data, and session views allowed by room policy. KP-only module text, KP secrets, system-internal payloads, and private memory must never be sent and hidden with CSS afterward.

## RLS Context

Repository code must set request context before querying scoped tables. RLS policy functions should read the current actor, room, and role context. Tables with room/document/agent data must enable and force RLS unless explicitly documented otherwise.

## Auth Private Tables

Production uses option A: a controlled `trpg_app_private` role for authentication-private tables. The application login role must not be the `postgres` superuser, must not have broad `BYPASSRLS`, and must be allowed to `SET ROLE trpg_app_private` only for these repository operations:

- `magic_link_challenges`
- `refresh_sessions`
- `idempotency_keys`
- `auth_identities`

`trpg_app_private` has `BYPASSRLS` but receives table privileges only on the authentication-private tables above. Repository code switches to that role only around those queries. Room, user, document, chunk, source, invite, audit, and game-state data still use the normal app role plus request RLS context.

`DATABASE_URL` must not use the `postgres` superuser in production. Migrations create `trpg_app_private`; production deployment must grant that role to the non-superuser application login role.

## RAG License RLS

Normal retrieval is DB-deny-by-default for unapproved licenses:

- `document_sources` ordinary `SELECT` requires `license_status = 'allowed'`.
- `documents` ordinary `SELECT` requires `license_status = 'allowed'`.
- `chunks` ordinary `SELECT` requires `license_status = 'allowed'`.

Pending review sources are visible only through the explicit KP/admin review path, marked in DB context with `app.rag_access_path = 'license_review'`. Denied content is not part of ordinary retrieval. Future admin views for denied rows must use a separate review/audit path, not the normal retrieval policy.

## Error Handling

Authorization failures must not reveal whether KP-only or denied resources exist. Use generic 403/404 behavior where necessary.

## Tests

Required coverage:

- PL cannot retrieve KP-only module chunks.
- Public screen cannot retrieve session/private data.
- `local_only` denies cloud model, embedding, rerank, and image providers.
- Missing room context cannot read scoped rows.
- Denied or pending-review documents are not indexed.
