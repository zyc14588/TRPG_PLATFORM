# P2 Acceptance Matrix

Every row must be mapped to an automated test, SQLx compile proof, OpenAPI check, or a documented deferred rationale in `docs/status/P2_STATUS.md`.

## B01 Domain

| Requirement | Evidence |
|---|---|
| shared license status allowed/pending_review/denied | unit test + type definition |
| visibility model supports PL/KP/SystemInternal boundaries | unit test + type definition |
| privacy mode supports LocalOnly vs AllowConfiguredCloud | unit test |
| chunking deterministic | `chunk_hash_stable` |
| content hash changes on content change | `chunk_hash_changes_on_content_change` |
| heading path preserved | `markdown_heading_path_preserved` |
| top_k bounded | `top_k_is_bounded` |
| citations required for evidence | `citation_required_for_evidence` |
| local provider deterministic/no-network | `local_embedder_is_deterministic` |

## B02 Storage/RLS/DB

| Requirement | Evidence |
|---|---|
| migrations additive and runnable | `cargo sqlx migrate run` |
| Rust enum values match DB CHECK constraints | schema test |
| denied job status persistable | `denied_ingest_job_status_is_persistable` |
| ordinary role blocked from pending/denied | `rls_blocks_pending_denied_chunks` |
| cross-room blocked | `rls_blocks_cross_room_chunks` |
| PL cannot see KP-only | `pl_cannot_retrieve_kp_only_module` |
| observer cannot see character-private | `observer_cannot_retrieve_character_private` |
| KP can retrieve KP-only if allowed | `kp_can_retrieve_kp_only` |
| SystemInternal never ordinary retrieval | `system_internal_never_returns` |
| review path KP-only | `review_path_lists_pending_for_kp`, `pl_cannot_review_sources` |
| prefilter before scoring | `retrieval_filters_before_scoring` |
| idempotency replay/conflict | `ingest_duplicate_replays`, `ingest_conflict_on_hash_mismatch` |

## B03 Ingest/Worker

| Requirement | Evidence |
|---|---|
| unknown license pending review | `unknown_license_goes_pending_review` |
| pending not chunked | `pending_review_is_not_chunked` |
| denied not indexed | `denied_license_is_not_indexed` |
| allowed open text indexed | `allowed_srd_can_index` |
| commercial prose denied | `commercial_adapter_text_denied` |
| schema-only adapter allowed | `commercial_adapter_schema_allowed` |
| LocalOnly rejects cloud embedder | `local_only_rejects_cloud_embedder` |
| provider metadata recorded safely | `provider_metadata_is_recorded` |
| failed ingest not completed | `failed_ingest_not_marked_completed` |
| retry/resume safe if supported | `retryable_job_can_resume` |

## B04 Rig Agent Engine

| Requirement | Evidence |
|---|---|
| Rig provider integration behind project trait | unit test + crate boundary |
| LocalOnly rejects cloud completion/embedding | `rig_local_only_rejects_cloud_completion`, `rig_local_only_rejects_cloud_embedding` |
| tools use policy-guarded repository | `rig_retrieval_tool_uses_policy_guarded_repository` |
| evidence bundle output, no final answer | `rig_agent_returns_evidence_bundle_not_final_answer` |
| hidden/denied/pending absent from prompt context | `rig_hidden_denied_pending_not_in_prompt_context` |
| provider metadata has no secret | `rig_provider_metadata_has_no_secret` |
| fake provider deterministic | `rig_fake_provider_is_deterministic` |

## B05 Server API/OpenAPI

| Requirement | Evidence |
|---|---|
| routes exist and require auth | route tests |
| CSRF required for cookie mutation | `csrf_required_for_cookie_mutation` |
| bearer route rejects cookie-only | `bearer_route_rejects_cookie_only` |
| PL cannot access review endpoints | `pl_cannot_access_review_endpoints` |
| KP can list pending review | `kp_can_list_pending_review` |
| ingest idempotency replay/conflict via API | `idempotent_ingest_replay_conflict` |
| query result has citation/hash | `query_result_has_citation_and_hash` |
| hidden/denied/pending not leaked in errors | `hidden_denied_pending_not_leaked_in_errors` |
| OpenAPI matches routes | `openapi_matches_routes` |

## B06 Frontend

| Requirement | Evidence |
|---|---|
| typed client functions exist | frontend tests/typecheck |
| ingest form sends contract shape | `ingest_form_sends_contract_shape` |
| idempotency key stable per submission | `ingest_submission_idempotency_key_stable` |
| evidence card shows citations/hash | `query_result_shows_citations`, `query_result_shows_content_hash` |
| PL review controls absent | `pl_review_controls_absent` |
| KP-only fields absent, not hidden | `kp_only_fields_absent_not_hidden` |
| no frontend secret | bundle/static search |

## B07 Final Gate

| Requirement | Evidence |
|---|---|
| Rust full gate | fmt/check/clippy/test |
| SQLx/migration gate | migrate/prepare |
| frontend full gate | install/lint/typecheck/test/build |
| generated artifact hygiene | `git ls-files` pattern check |
| status report complete | `docs/status/P2_STATUS.md` |
| no stale docs/OpenAPI | docs/schema review |
| no secret leakage | static search + tests |
