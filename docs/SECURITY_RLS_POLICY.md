# Security and RLS Policy

## Authorization Model

The platform uses application-layer ABAC plus PostgreSQL RLS. Authorization decisions are deny-by-default. Every room, document, clue, memory, and Agent payload access must be scoped by actor, room role, visibility, and privacy mode.

## Visibility Rules

PL clients may receive only public rules, room rules, PL-visible clues, their own character-private data, and session views allowed by room policy. KP-only module text, KP secrets, system-internal payloads, and private memory must never be sent and hidden with CSS afterward.

## RLS Context

Repository code must set request context before querying scoped tables. RLS policy functions should read the current actor, room, and role context. Tables with room/document/agent data must enable and force RLS unless explicitly documented otherwise.

## Error Handling

Authorization failures must not reveal whether KP-only or denied resources exist. Use generic 403/404 behavior where necessary.

## Tests

Required coverage:

- PL cannot retrieve KP-only module chunks.
- Public screen cannot retrieve session/private data.
- `local_only` denies cloud model, embedding, rerank, and image providers.
- Missing room context cannot read scoped rows.
- Denied or pending-review documents are not indexed.
