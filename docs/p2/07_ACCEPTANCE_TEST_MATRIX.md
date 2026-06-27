# P2 Acceptance Test Matrix

Each row must map to at least one automated test, unless explicitly deferred in `docs/status/P2_STATUS.md`.

| Area | Test name | Required behavior | Layer |
|---|---|---|---|
| License | `unknown_license_goes_pending_review` | Missing/unclear license returns pending review | rag_core/document_ingestor/API |
| License | `pending_review_is_not_chunked` | Pending source creates no chunks/embeddings | document_ingestor/storage |
| License | `denied_license_is_not_indexed` | Denied source creates no retrievable chunks | document_ingestor/storage/API |
| License | `allowed_srd_can_index` | Allowed SRD/open text chunks, embeds, indexes | end-to-end |
| License | `commercial_adapter_text_denied` | Commercial prose in adapter is denied | rag_core/document_ingestor |
| License | `commercial_adapter_schema_allowed` | Mechanics/schema-only adapter can be allowed | rag_core |
| Privacy | `local_only_rejects_cloud_embedder` | LocalOnly room cannot use cloud embedder | rag_core/document_ingestor/API |
| Privacy | `local_only_uses_deterministic_embedder` | Local smoke test runs with no network | rag_core |
| Provider | `provider_metadata_is_recorded` | Ingest job records provider kind/version | storage/API |
| Chunking | `markdown_heading_path_preserved` | Evidence includes heading path | rag_core/API/frontend |
| Chunking | `chunk_hash_stable` | Same normalized text same hash | rag_core |
| Chunking | `chunk_hash_changes_on_content_change` | Changed content changes hash | rag_core |
| Chunking | `chunk_size_is_bounded` | Chunker enforces configured max | rag_core |
| Evidence | `citation_required` | Evidence includes source/document/chunk/citation fields | API/frontend |
| Retrieval | `pl_cannot_retrieve_kp_only_module` | PL never receives KP-only chunks | storage/API |
| Retrieval | `observer_cannot_retrieve_character_private` | Observer cannot see character-private chunks | storage/API |
| Retrieval | `kp_can_retrieve_kp_only` | KP/Owner/AssistantKp can retrieve KP-only if allowed | storage/API |
| Retrieval | `system_internal_never_returns` | SystemInternal never returned by normal query | storage/API |
| Retrieval | `public_rule_requires_allowed_license` | Public visibility still requires allowed license | storage/API |
| Retrieval | `retrieval_filters_before_scoring` | Denied/invisible chunks are not scored/ranked | rag_core/storage |
| RLS | `rls_blocks_pending_denied_chunks` | DB role cannot select pending/denied chunks via normal path | storage integration |
| RLS | `rls_blocks_cross_room_chunks` | Room A member cannot read room B chunks | storage integration |
| RLS | `review_path_lists_pending_for_kp` | Pending review only via review policy/endpoint | storage/API |
| RLS | `pl_cannot_review_sources` | PL cannot access pending review list | storage/API |
| Idempotency | `ingest_duplicate_replays` | Same key/hash replays exact response | storage/API |
| Idempotency | `ingest_conflict_on_hash_mismatch` | Same key/different payload conflicts | storage/API |
| Jobs | `failed_ingest_not_marked_completed` | Failed write does not store completed response | storage/document_ingestor |
| Jobs | `job_state_rejects_invalid_transition` | Terminal jobs not reopened silently | storage |
| Jobs | `retryable_job_can_resume` | Non-terminal job can resume safely | storage/document_ingestor |
| API | `openapi_matches_routes` | OpenAPI includes every P2 route and DTO | server/schema |
| API | `top_k_is_bounded` | Query rejects excessive top_k | server/rag_core |
| Frontend | `query_result_shows_citations` | UI renders source/title/heading/hash | frontend |
| Frontend | `pl_review_controls_absent` | PL cannot see review controls | frontend/API |
| Frontend | `kp_only_fields_absent_not_hidden` | Client DTO does not include hidden KP-only fields | frontend fake backend |
