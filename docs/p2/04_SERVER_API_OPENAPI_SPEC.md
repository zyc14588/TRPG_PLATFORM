# Server API and OpenAPI Spec

## Route set

Minimum P2 API:

```text
POST /api/rooms/{room_id}/documents/ingest
GET  /api/rooms/{room_id}/documents/{document_id}
POST /api/rooms/{room_id}/rag/query
GET  /api/rooms/{room_id}/document-sources/pending-review
POST /api/rooms/{room_id}/document-sources/{source_id}/review
```

Route prefixes may follow existing API conventions. Update `schemas/openapi.json` to match the implementation.

## DTO rules

- API returns DTOs only; never raw DB entities.
- Evidence DTO must include provenance.
- Hidden fields must be absent, not present with null/empty values.
- Error DTO must not leak whether hidden/denied content exists.

## Ingest request

```json
{
  "idempotency_key": "uuid-or-client-key",
  "title": "House rules",
  "source_kind": "UserProvidedText",
  "license_attestation": "user_has_rights",
  "visibility": "RoomRule",
  "privacy_mode": "LocalOnly",
  "content_type": "text/markdown",
  "text": "# Markdown text..."
}
```

Bounds:

- Reject empty text.
- Reject text larger than configured limit.
- Reject unsupported source kind.
- Unknown/ambiguous license returns pending review and does not chunk/embed/index.

## Ingest response

```json
{
  "job_id": "uuid",
  "source_id": "uuid",
  "document_id": "uuid-or-null",
  "status": "completed|pending_review|denied|failed",
  "license_status": "allowed|pending_review|denied",
  "chunk_count": 12,
  "provider_metadata": {
    "embedder": "deterministic-local",
    "version": "test"
  },
  "replayed": false
}
```

## Query request

```json
{
  "query": "grapple rules",
  "top_k": 5,
  "filters": {
    "source_kind": ["OfficialSrd", "UserProvidedText"],
    "visibility": ["PublicRule", "RoomRule"]
  },
  "privacy_mode": "LocalOnly"
}
```

## Query response

```json
{
  "evidence": [
    {
      "source_id": "uuid",
      "document_id": "uuid",
      "chunk_id": "uuid",
      "score": 0.82,
      "content_hash": "sha256:...",
      "preview": "bounded excerpt",
      "citation": {
        "title": "SRD excerpt",
        "heading_path": ["Combat", "Grappling"],
        "location": "section heading"
      }
    }
  ],
  "applied_filters": {},
  "provider_metadata": {}
}
```

## Status codes

- `200`: success or idempotent replay.
- `201`: newly completed ingest if project convention uses create status.
- `202`: pending review accepted but not indexed.
- `400`: invalid input, unsupported content type, top_k too large.
- `401`: missing/invalid auth.
- `403` or generic `404`: visibility/license denial without existence leak.
- `409`: idempotency key reused with different payload.
- `413`: upload too large.
- `422`: license denied or source metadata invalid if project convention prefers semantic validation.
- `500`: unexpected server error only.

## CSRF/auth contract

Follow P1.5 CSRF decision:

- Bearer-token mutation routes may omit CSRF only if they never authenticate by cookies.
- Cookie-authenticated mutation routes require CSRF.
- Tests must cover missing CSRF on cookie-auth paths.

## Tests

- Route contract tests for every endpoint.
- OpenAPI validity test.
- PL/observer negative retrieval tests.
- KP review positive/negative tests.
- Idempotent ingest replay/conflict tests.
- LocalOnly rejects cloud provider test.
