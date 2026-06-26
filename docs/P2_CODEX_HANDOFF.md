# TRPG_PLATFORM P2 Codex Handoff — Rules / RAG / Ingestion

本文档是 P1.5 修复完成后交给 Codex 的 P2 主任务说明。当前唯一有效的 P2 定义是：**规则系统、文档导入、license gate、切块、embedding provider trait、权限优先混合检索、citation-bearing evidence**。

Realtime/WebSocket/Redis/outbox replay 不属于本主线 P2；如需并行，命名为 P2B 或 Phase 3。

## 0. 先决条件

Codex 开始本任务前必须确认：

- P1.5 audit blockers 全部关闭。
- `docs/status/P1_AUDIT.md` 不再声称不存在的 RAG kernel 已完成。
- `prompts/03_RULES_RAG.md` 已改成 Phase 2。
- 生产启动、idempotency、refresh、RLS/license gate 已通过新增测试。
- 不存在 `.git` / `node_modules` / generated build artifacts 的交付包污染。

## 1. P2 目标

P2 的最小可交付是：

1. `rag_core` 提供可编译、可测试、provider-agnostic 的 RAG kernel。
2. `document_ingestor` 只做 ingest orchestration，不重复定义 license/status 语义。
3. PostgreSQL schema/RLS 与 Rust repository 合同一致。
4. 本地 deterministic embedder + in-memory store 可跑完整 ingest/query smoke test。
5. 所有 retrieval 返回 evidence/citation，不直接生成最终回答。
6. pending/denied license 永远不进入普通 indexing/retrieval。
7. KP-only/module/private visibility 永远不泄露给 PL/observer/public screen。

## 2. 不允许事项

- 不引入任何未授权商业规则正文。
- 不把 LLM 输出直接写入游戏状态。
- 不在 LocalOnly room 中调用 cloud embedder / cloud LLM。
- 不让 pending_review / denied 文档进入 vector index 或 retrieval result。
- 不绕过 ABAC/RLS，仅用应用层注释保证安全。
- 不把 PDF/OCR 作为 P2 必做；PDF/OCR 可以保留 adapter boundary。
- 不先做 WebSocket/Redis/agent UI，除非明确完成 P2 core。

## 3. crate 边界

### `crates/rag_core`

必须包含并测试：

- domain types：
  - `DocumentSource`
  - `SourceKind`
  - `LicenseStatus`
  - `LicenseDecision`
  - `DocumentMetadata`
  - `Chunk`
  - `ChunkId` / `DocumentId` / `SourceId` wrapper 或 UUID aliases
  - `ChunkingOptions`
  - `RetrievalQuery`
  - `RetrievalFilter`
  - `RetrievalResult`
  - `Evidence`
  - `Citation`
- traits：
  - `Chunker`
  - `Embedder`
  - `VectorStore`
  - `KeywordIndex`
  - `HybridRetriever`
- local implementations：
  - `MarkdownChunker`
  - `DeterministicLocalEmbedder`
  - `InMemoryVectorStore`
  - Optional simple keyword scoring for tests

### `crates/document_ingestor`

职责：

- 接收 source metadata 和 raw text。
- 调用 `rag_core::check_license`。
- 如果 license 不是 allowed，生成 pending/denied job result，不 chunk、不 embed、不 index。
- 如果 allowed，调用 chunker/embedder/store。
- 返回 ingest summary：source id、document id、chunk count、license status、job status、citations。

不得：

- 自己定义第二套 `LicenseStatus`。
- 调 cloud provider 时忽略 `PrivacyMode::LocalOnly`。

### `crates/storage`

职责：

- 提供 `RagRepository` / `RagRepositoryTransaction`。
- 持久化 `document_sources`、`documents`、`chunks`、`ingest_jobs`。
- 所有写操作事务化。
- 所有 retrieval 查询显式过滤 `license_status='allowed'`。

### `crates/server`

P2 可以只做最小 API：

- `POST /api/rooms/{room_id}/documents/ingest`
- `GET /api/rooms/{room_id}/documents/{document_id}`
- `POST /api/rooms/{room_id}/rag/query`
- `GET /api/rooms/{room_id}/document-sources/pending-review`，仅 KP/Owner/AssistantKp
- `POST /api/rooms/{room_id}/document-sources/{source_id}/review`，仅 KP/Owner

API 输出必须是 DTO，不返回 raw DB entity。

## 4. License policy

### 状态

```rust
pub enum LicenseStatus {
    Allowed,
    PendingReview,
    Denied,
}
```

### 规则

- Official SRD / recognized open license / explicit user-provided rights => `Allowed`。
- Missing or ambiguous license => `PendingReview`。
- Known no-redistribution / incompatible / commercial rule text => `Denied`。
- Commercial adapter crate 只允许 mechanics/schema code，不允许携带商业正文。

### 强制点

- ingestion 前检查。
- chunking 前检查。
- embedding 前检查。
- index upsert 前检查。
- retrieval query 前和 DB/RLS 中检查。

## 5. Visibility policy

Retrieval 必须先过滤权限，再做 scoring。

- `PublicRule`：任何用户可读，但必须 license allowed。
- `RoomRule` / `PlVisibleClue`：房间成员可读。
- `CharacterPrivate`：本人、Owner、KP、AssistantKp 可读。
- `KpOnlyModule` / `KpSecret` / `MemoryPrivate`：Owner、KP、AssistantKp 可读。
- `SystemInternal`：不可由普通 retrieval 返回。

## 6. Stop points

### Stop point P2-A：rag_core domain + local kernel

交付：

- `rag_core` 编译。
- local chunking/embed/search 完整单测。
- `document_ingestor` 复用 `rag_core::LicenseStatus`。

测试：

```bash
cargo test -p rag_core -p document_ingestor
```

### Stop point P2-B：storage schema/repository

交付：

- Additive migration 修正/补充 RAG tables/policies。
- SQLx repository + transaction trait。
- license/status/visibility RLS 测试。

测试：

```bash
cargo test -p storage --features postgres
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

### Stop point P2-C：server API

交付：

- ingest/query/review endpoints。
- OpenAPI 更新。
- route-contract tests。
- ABAC/RLS API tests。

测试：

```bash
cargo test -p server
```

### Stop point P2-D：frontend minimal admin flow

交付：

- 文档上传/粘贴 ingest 表单。
- pending review 列表。
- query evidence/citation 展示。
- 客户端 DTO 不接受 KP-only 泄露字段。

测试：

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
```

## 7. Acceptance tests

必须新增并通过：

- unknown_license_goes_pending_review_and_is_not_indexed
- denied_commercial_text_is_not_chunked_or_embedded
- allowed_srd_text_can_ingest_and_query
- local_only_room_rejects_cloud_embedder
- chunk_hash_is_stable_for_same_normalized_text
- changed_text_changes_chunk_hash
- pl_query_cannot_return_kp_only_module
- observer_query_cannot_return_character_private
- kp_query_can_return_kp_only_with_citation
- public_rule_requires_allowed_license
- retrieval_result_contains_source_id_document_id_chunk_id_content_hash_citation
- query_top_k_is_bounded
- ingest_idempotency_replays_same_response
- ingest_idempotency_conflicts_on_different_payload
- pending_review_can_be_listed_only_by_kp_or_owner

## 8. Suggested implementation order

1. Normalize P2 docs/prompt names.
2. Implement `rag_core` domain types and license decision.
3. Move `document_ingestor::LicenseStatus` into `rag_core`.
4. Implement `MarkdownChunker` and hash normalization.
5. Implement deterministic local embedder and in-memory vector store.
6. Add visibility/license-first retrieval tests.
7. Add repository traits and SQLx storage.
8. Add server endpoints and OpenAPI.
9. Add minimal frontend flow.
10. Run full verification and update status report.

## 9. Final P2 completion definition

P2 is complete only when:

- `cargo fmt/check/clippy/test` pass across workspace.
- SQLx migration and prepare pass against fresh DB.
- pnpm lint/typecheck/test/e2e/build pass.
- A PL user cannot retrieve KP-only or denied/pending license chunks through API or direct DB role.
- Evidence includes citation and provenance.
- No cloud provider is used in LocalOnly mode.
- P2 status report lists exact commands and results.
