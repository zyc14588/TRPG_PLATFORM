# P2 RAG Acceptance Test Matrix

## License gate

| Test | Required behavior |
|---|---|
| unknown_license_goes_pending_review | Missing/unclear license returns `PendingReview` |
| pending_review_is_not_chunked | Pending source creates job result but no chunks/embeddings |
| denied_license_is_not_indexed | Denied source creates no retrievable chunks |
| allowed_srd_can_index | Official SRD/open source can chunk/embed/index |
| commercial_adapter_text_denied | Commercial adapter carrying rule prose is denied |
| commercial_adapter_schema_allowed | Adapter with only mechanics/schema code can be allowed |

## Privacy / provider boundary

| Test | Required behavior |
|---|---|
| local_only_rejects_cloud_embedder | LocalOnly room cannot use cloud embedder |
| local_only_uses_deterministic_embedder | Local-only smoke test runs without network |
| provider_metadata_is_recorded | Ingest job records provider kind/version |

## Chunking and provenance

| Test | Required behavior |
|---|---|
| markdown_heading_path_preserved | Evidence includes section path |
| chunk_hash_stable | Same normalized text produces same content hash |
| chunk_hash_changes_on_content_change | Changed content changes hash |
| chunk_size_is_bounded | Chunker enforces max chars/tokens |
| citation_required | Evidence includes source/document/chunk/citation fields |

## Retrieval security

| Test | Required behavior |
|---|---|
| pl_cannot_retrieve_kp_only_module | PL query never returns KP-only chunks |
| observer_cannot_retrieve_character_private | Observer cannot see private character chunks |
| kp_can_retrieve_kp_only | KP/Owner/AssistantKp can retrieve KP-only if license allowed |
| system_internal_never_returns | `SystemInternal` never returned by normal query |
| public_rule_requires_allowed_license | Public rule still requires allowed license |
| retrieval_filters_before_scoring | Denied/invisible chunks are not scored/ranked |

## Repository / RLS

| Test | Required behavior |
|---|---|
| rls_blocks_pending_denied_chunks | DB role cannot select pending/denied chunks via normal policy |
| rls_blocks_cross_room_chunks | Member of room A cannot read room B chunks |
| review_path_lists_pending_for_kp | Pending review only via review policy/endpoint |
| pl_cannot_review_sources | PL cannot access pending review list |

## Idempotency and jobs

| Test | Required behavior |
|---|---|
| ingest_duplicate_replays | Same idempotency key/hash replays exact response |
| ingest_conflict_on_hash_mismatch | Same idempotency key/different payload returns conflict |
| failed_ingest_not_marked_completed | Failed write does not store completed response |
| job_state_rejects_invalid_transition | Terminal jobs cannot be reopened silently |
| retryable_job_can_resume | Non-terminal job can resume from safe checkpoint |
