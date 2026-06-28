# B05 Spec — Server API and OpenAPI

## Goal

Expose minimal P2 API for document ingestion, document metadata, RAG/agent evidence query, pending review and review decisions. All routes must use existing auth/room membership patterns and rely on storage/RLS for data safety.

## Endpoint set

Path naming may follow existing project style. Required semantics:

```text
POST /rooms/{room_id}/documents:ingest
GET  /rooms/{room_id}/documents
GET  /rooms/{room_id}/documents/{document_id}
POST /rooms/{room_id}/rag/query
GET  /rooms/{room_id}/document-sources/pending
POST /rooms/{room_id}/document-sources/{source_id}/review
```

If Rig agent endpoint is separate:

```text
POST /rooms/{room_id}/agent/evidence-query
```

It must still return evidence-first output, not final answer, in P2.

## Ingest request

Fields:

- `title`
- `source_kind`
- `license_attestation` or license metadata
- `visibility`
- `content_type`
- `content`
- `privacy_mode`
- `idempotency_key` via header or request field following existing project convention

Limits:

- raw text size hard cap
- title length cap
- metadata size cap
- content type allowlist

## Ingest response

Fields:

- `source_id`
- `document_id` nullable when pending/denied
- `ingest_job_id`
- `status`
- `chunk_count`
- `provider_metadata`
- `replayed`

## Query request

Fields:

- `query`
- `top_k` with hard maximum
- `filters`
- `privacy_mode`
- optional `agent_mode` if B04 exposed evidence agent

## Query response

Must contain:

- `evidence[]`
- `applied_filters`
- `provider_metadata`
- `trace_id`

Each evidence item:

- `source_id`
- `document_id`
- `chunk_id`
- `score`
- `content_hash`
- `preview`
- `citation`
- safe source/visibility metadata

Must not contain final generated answer by default in P2.

## Review endpoints

`GET pending`:

- KP/Owner/AssistantKP only.
- Uses review access path.
- Lists pending source metadata only, not full hidden content unless explicitly safe.

`POST review`:

- decision: approve/deny
- reason
- optional visibility correction if policy allows
- creates auditable record or source status update

## Auth / CSRF / membership

- All P2 routes require auth.
- Room membership verified server-side.
- Client-supplied role ignored.
- Cookie-authenticated mutations require CSRF.
- Bearer-only mutations must not authenticate via refresh cookie fallback.
- Hidden/denied existence should not leak through error detail.

## Status codes

- `200` success or idempotent replay.
- `201` newly created document/job if project style uses create status.
- `202` pending review accepted.
- `400` invalid input/top_k/content type.
- `401` unauthenticated.
- `403` forbidden or use generalized `404` if project hides existence.
- `409` idempotency conflict.
- `413` upload too large.
- `422` semantic validation/license denial if project uses it.
- `500` unexpected internal error only.

## OpenAPI requirements

- All endpoints present.
- Request/response schemas match Rust DTO.
- Security schemes match route behavior.
- Examples contain no secrets or hidden content.
- No schema promises generated final answer in P2.

## Required tests

- route contract test per endpoint
- `openapi_matches_routes`
- `top_k_is_bounded`
- `missing_auth_rejected`
- `csrf_required_for_cookie_mutation`
- `bearer_route_rejects_cookie_only`
- `pl_cannot_access_review_endpoints`
- `kp_can_list_pending_review`
- `idempotent_ingest_replay_conflict`
- `query_result_has_citation_and_hash`
- `hidden_denied_pending_not_leaked_in_errors`
- `local_only_rejects_cloud_provider_through_api`

## Batch boundary

B05 must not implement frontend pages or generated-answer chat UX.
