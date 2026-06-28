# B06 Spec — Frontend RAG UI

## Goal

Provide a minimal, testable Next.js UI for document ingest, pending review and evidence query. UI improves usability but is never the security boundary.

## Recommended routes

Adapt names to existing app router conventions:

```text
/rooms/[roomId]/documents
/rooms/[roomId]/documents/new
/rooms/[roomId]/rag
/rooms/[roomId]/document-sources/review
```

## Typed client functions

In `apps/web/src/lib/backend.ts` or equivalent:

- `listRoomDocuments(roomId)`
- `ingestRoomDocument(roomId, request)`
- `queryRoomRag(roomId, request)`
- `listPendingDocumentSources(roomId)`
- `reviewDocumentSource(roomId, sourceId, request)`

Rules:

- use DTOs matching OpenAPI/server contract;
- do not depend on raw DB rows;
- preserve auth/CSRF conventions;
- do not embed provider API keys or server secrets.

## Documents list

- Show actor-visible allowed documents.
- Show source kind, title, created time, safe visibility metadata.
- KP/Owner may see pending review navigation if safe role info exists.
- PL/observer must not see review controls.

## New document ingest

Fields:

- title
- source kind
- license attestation/metadata
- visibility
- content type
- Markdown/plain text content
- privacy mode

Behavior:

- generate stable idempotency key per submission attempt;
- new submission gets new key;
- show completed/pending_review/denied/failed states clearly;
- show conflict state if idempotency mismatch occurs;
- do not display raw server internals.

## Pending review UI

- Only render for KP/Owner/AssistantKP role data returned by safe backend DTO.
- Approve/deny with reason.
- Refresh state after action.
- PL/observer tests must prove controls are absent, not merely CSS-hidden.

## RAG evidence query UI

Inputs:

- query text
- bounded `top_k`
- filters if API supports them
- privacy mode

Output:

- evidence cards
- source title
- heading path / location
- citation
- content hash
- preview
- safe provider metadata

P2 UI must not show a generated final answer as the main product output.

## Accessibility / testability

- Use accessible labels for forms/buttons.
- Stable test selectors are allowed if project uses them.
- Loading/empty/error states are covered.
- Avoid stale phase copy such as `Phase 1B`; use neutral copy like `Rules & RAG`, `Document evidence`, `Room knowledge`.

## Required tests

- `ingest_form_sends_contract_shape`
- `ingest_submission_idempotency_key_stable`
- `new_ingest_submission_uses_new_key`
- `pending_denied_completed_states_render`
- `query_result_shows_citations`
- `query_result_shows_content_hash`
- `pl_review_controls_absent`
- `kp_only_fields_absent_not_hidden`
- `csrf_sent_for_cookie_mutation`
- `no_provider_secret_in_frontend_bundle_or_env`

## Batch boundary

B06 should not change backend semantics except tiny contract alignment fixes. If a backend security bug is discovered, stop and open a backend repair batch.
