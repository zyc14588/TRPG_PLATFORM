# S03 — Data/Eventing：PostgreSQL、Event Store、Outbox、Projection、RAG Snapshot

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

落地 SQLx migrations、Event Store append/replay、idempotency guard、outbox、projection rebuild、NATS JetStream adapter、Redis presence/cache、pgvector/RAG snapshot。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S03` |
| 相关分类 | `06-data-eventing` |
| 相关 batch | BATCH-024-06-data-eventing, BATCH-025-06-data-eventing, BATCH-026-06-data-eventing, BATCH-027-06-data-eventing, BATCH-028-06-data-eventing |
| Prompt 数 | 107 |
| Primary | 38 |
| Supplemental | 46 |
| Docs/Trace | 23 |
| 主要 crate | `trpg-data-eventing` |

## 3. 启动条件

S01/S02 通过；domain events 与 command envelopes 已稳定。

## 4. 主要输出

- `crates/trpg-data-eventing/src/*.rs`
- `migrations/*.sql`
- `schemas/events/*.json`
- `crates/trpg-data-eventing/tests/*.rs`

## 5. 测试重点

- SQLx migration 正反向
- Event Store append expected_version 冲突
- idempotency key 重放
- outbox exactly-once-ish 发布
- projection replay hash
- RAG snapshot chunk hash 与 visibility 继承

## 6. 推荐命令

- `sqlx migrate run`
- `cargo test -p trpg-data-eventing --all-features`
- `cargo test --test event_store_contract`
- `cargo test --test projection_replay`

## 7. 测试数据

- `test-data/event_store_stream_cases.md`
- `test-data/rag_snapshot_cases.md`
- `test-data/api_ws_contract_samples.md`

## 8. 阶段验收清单

- [ ] Event Store 是唯一正史；Projection/Cache/RAG 均可重建
- [ ] 所有正式写入在 transaction 内完成 event append 与 outbox
- [ ] NATS 只发布 Event Store 派生事件，不替代正史
- [ ] RAG chunk 携带 source_type、visibility、version、owner、allowed_use

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
