# Frontend RAG UI Spec

## Goal

Add a minimal, testable RAG management surface for room KP/Owner users. The UI is not a full document management system; it exists to prove secure ingest, license review, and citation-bearing retrieval.

## Routes

Recommended routes under `apps/web/src/app`:

```text
/rooms/[roomId]/documents
/rooms/[roomId]/documents/new
/rooms/[roomId]/rag
/rooms/[roomId]/document-sources/review
```

Adapt route structure to existing app conventions.

## Pages

### Documents list

- Shows allowed documents visible to the actor.
- KP/Owner sees link to pending review.
- PL does not see KP-only management actions.

### New document / paste ingest

Fields:

- title
- source kind
- license attestation
- visibility
- content type
- text area for Markdown/plain text
- privacy mode

Client behavior:

- Generate idempotency key per submission.
- Show pending review result distinctly from completed ingest.
- Do not show raw server error internals.

### Pending review

KP/Owner only:

- list pending sources
- approve/deny with reason
- refresh list after action

PL/observer:

- route should not render review data; API should deny even if UI route is reached.

### RAG query

- Input query and bounded top_k.
- Show evidence cards with citation, source title, heading path, content hash, and preview.
- Do not show generated final answer in P2.

## Client API requirements

Add typed functions in the existing frontend backend/client layer:

- `ingestRoomDocument(roomId, request)`
- `queryRoomRag(roomId, request)`
- `listPendingDocumentSources(roomId)`
- `reviewDocumentSource(roomId, sourceId, request)`

All DTOs should match `schemas/openapi.json`.

## Privacy tests

- Fake backend test: PL response does not include KP-only fields.
- Component/test route: pending review controls absent for PL.
- API-client test: idempotency key is stable for a single submission and regenerated for a new submission.
- Query result rendering includes citation and content hash.

## UX copy

Use phase-neutral copy such as “Rules & RAG” or “Document evidence”. Avoid stale labels like “Phase 1B” in production UI.
