# B03 Spec — Document Ingestor and Worker

## Goal

Implement deterministic, license-first ingestion orchestration that transforms allowed text into persisted documents/chunks/evidence-ready records. Pending or denied content must not become searchable.

## Ingest pipeline

```text
validate request
  ↓
compute request hash / idempotency lookup
  ↓
evaluate license policy
  ├─ denied         → persist denied source/job summary; no chunks/embeddings
  ├─ pending_review → persist pending source/job summary; no ordinary chunks/embeddings
  └─ allowed        → normalize → chunk → hash/citation → embed → persist transactionally
```

## License-first requirement

License evaluation must happen before chunking, embedding, indexing, provider calls and Rig workflows. Unknown or ambiguous license should become `PendingReview` unless project policy says stricter denial.

## Visibility behavior

- Source visibility default is copied to documents/chunks unless explicitly overridden by policy.
- KP-only/SystemInternal text must not be embedded into a provider that the room privacy policy forbids.
- Ingest may persist hidden allowed chunks, but ordinary PL retrieval must not score them.

## Provider metadata

Persist safe metadata only:

- provider kind
- model/vector dimension/version
- local/cloud classification
- provider crate/integration name if useful
- input hash/request id

Never persist:

- API keys
- raw provider request with hidden text unless explicitly safe and access controlled
- authorization headers
- DB URLs

## Job state machine

Required states or equivalents:

```text
queued/claimed → parsing → embedding → indexed/completed
queued/claimed → pending_review
queued/claimed → denied
queued/claimed/parsing/embedding → failed
```

Terminal states should not be silently reopened unless a documented retry/resume policy exists.

## Transaction boundaries

- Source/document/chunk/job summary writes should be atomic for successful allowed ingest.
- Failed ingest must not leave completed job response.
- Partial chunks should not be ordinary-searchable after failure.
- Idempotency response must be saved only when state is semantically final or replayable.

## Worker behavior

If `crates/worker` exists or is added:

- Provide a testable single-step runner.
- Avoid busy loops.
- Support graceful shutdown or cancellation.
- Job claiming must avoid duplicate concurrent processing.
- Tests must not sleep for long wall-clock durations.

## Required tests

- `unknown_license_goes_pending_review`
- `pending_review_is_not_chunked`
- `denied_license_is_not_indexed`
- `allowed_srd_can_index`
- `commercial_adapter_text_denied`
- `commercial_adapter_schema_allowed`
- `local_only_rejects_cloud_embedder`
- `provider_metadata_is_recorded`
- `failed_ingest_not_marked_completed`
- `ingest_duplicate_replays`
- `ingest_conflict_on_hash_mismatch`
- `retryable_job_can_resume` if retry is implemented

## Batch boundary

B03 must not expose HTTP routes, frontend UI or final LLM answer generation. It may use `storage` and `rag_core`, but not bypass their invariants.
