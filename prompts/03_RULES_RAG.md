# Codex Phase 2 — Rules and RAG

P1.5 gate 通过后才开始本阶段。当前唯一有效的 P2 主线是 Rules/RAG/Document Ingestion：规则适配器边界、合法文档导入、license gate、结构化切块、embedding provider trait、权限优先混合检索和 citation-bearing evidence。

## Boundaries

- 先读 `docs/P2_CODEX_HANDOFF.md`、`docs/P2_RAG_ACCEPTANCE_TESTS.md`、`docs/P2_RAG_IMPLEMENTATION_SPEC.md`、`docs/RAG_DESIGN.md`。
- 所有检索类型、trait、过滤和 evidence 模型放在 `crates/rag_core`。
- `document_ingestor` 只负责许可检查、解析、切块和调用 `rag_core` 边界。
- 规则系统代码适配器与规则正文内容包必须解耦。
- P2 可以只做本地 deterministic embedding/local store，云 provider 只保留边界。

## Stop Points

- P2-A: `rag_core` domain types、license status、chunk metadata、embedder/vector/search traits 和 local kernel 编译通过。
- P2-B: document ingestion 通过 license gate，unknown 进入 `pending_review`，denied 不入索引。
- P2-C: retrieval 先做 license/visibility/room role 过滤，再返回 citation-bearing evidence。
- P2-D: 最小 ingest/query API 或 repository contract 可测试；未实现的 adapter 明确留 TODO，不假装完成。

## Prohibited

- 不实现 WebSocket、Redis、Realtime/Concurrency、Outbox Replay 或 agent UI。
- 不加入、抓取、fixture 化任何未授权商业规则正文。
- 不让 PL 查询或接收 KP-only 数据；不得先发送再用前端隐藏。
- 不让骰子随机数、数学判定或权威状态写入依赖 LLM。
- 不在 `local_only` 房间调用云端 embedding、rerank、chat 或 image provider。

## Acceptance Criteria

- `rg "Codex Phase 2" docs prompts` 只把当前主线指向 Rules/RAG/Document Ingestion。
- Unknown license 文档为 `pending_review`，不生成可检索 chunk。
- Commercial adapter fixture 使用自造或明确许可文本。
- Retrieval evidence 带 source/license/page/section metadata。
- PL-visible filter 不能命中 KP-only chunk。
- P2 状态报告列出实际运行命令和结果；P1.5 gate 未通过时不得启动本阶段。
