# P2 Scope and Architecture

## Product scope

P2 为每个 TRPG room 提供可审计的 rules/document evidence foundation：

- 导入 text/Markdown source。
- 根据 license policy 和 visibility policy 决定是否可进入 retrieval。
- 确定性 normalize、chunk、hash、citation。
- 使用 provider abstraction 生成 embeddings 或 deterministic local vectors。
- 将 source/document/chunk/job/provenance 持久化到 PostgreSQL。
- 通过 RLS 和 repository 查询强制 room/role/license/visibility 边界。
- 通过 server API 返回 evidence/citations/provenance。
- 通过 frontend 展示 ingest/review/query evidence。
- 通过 Rig agent engine 建立 provider/tool-call/workflow adapter，但 P2 默认不生成最终叙事答案。

## Non-goals

- 完整 PDF/OCR pipeline。
- 大型 agentic GM 自动剧情生成。
- 未经授权商业规则正文 ingestion。
- UI-only security。
- Live cloud provider integration tests。
- WebSocket/Redis/outbox replay 作为 P2 必需项。

## System architecture

```text
User / KP / Player
   │
   ▼
apps/web ── typed client ──► crates/server ── auth, CSRF, room membership, OpenAPI DTO
                                  │
                                  ├─► crates/document_ingestor ── license-first ingest
                                  │          │
                                  │          ├─► crates/rag_core ── domain types / traits
                                  │          └─► crates/storage ── SQLx / PostgreSQL / RLS
                                  │
                                  ├─► crates/agent_engine ── Rig provider/tool orchestration
                                  │          │
                                  │          └─► policy-guarded retrieval tools only
                                  │
                                  └─► crates/storage ── repository / idempotency / candidate retrieval
```

## Data flow: ingestion

1. Authenticated actor submits source metadata, content, license attestation, visibility, privacy mode, idempotency key.
2. Server validates auth, CSRF/mutation semantics, room membership, payload size.
3. Ingestor evaluates license before chunking/embedding.
4. If `denied`, persist denied job/source summary; do not create searchable chunks/embeddings.
5. If `pending_review`, persist reviewable source/job; do not create ordinary retrieval chunks/embeddings.
6. If `allowed`, normalize text, chunk deterministically, compute hashes/citations, call allowed provider, persist transactionally.
7. Repository stores idempotency response for replay/conflict.

## Data flow: retrieval / agent query

1. Actor submits query, bounded `top_k`, filters, privacy mode.
2. Server validates auth and room membership.
3. `agent_engine` may plan retrieval or call a retrieval tool, but only through policy-guarded service/repository.
4. Storage/RLS filters by room, license, visibility, role before scoring/ranking.
5. Result returns evidence with citations/provenance; no final generated answer by default in P2.

## Trust boundaries

| Boundary | Trusted for security? | Notes |
|---|---:|---|
| Frontend UI | No | May hide controls for UX only. Backend/RLS must enforce. |
| Server handlers | Partially | Must enforce auth/CSRF/membership, but cannot be sole filter for hidden data. |
| Repository | Yes | Must apply query predicates, idempotency, DTO mapping. |
| PostgreSQL RLS | Yes | Final room/license/visibility guard for DB reads/writes. |
| Rig agent | No | Uses tools/providers; cannot bypass policy. |
| Provider | No | Never receives hidden or denied content unless policy permits. |

## Persistent design rule

If implementation discovers that the current codebase uses different names, Codex should adapt names while preserving semantics, and record the mapping in `docs/status/P2_STATUS.md`.
