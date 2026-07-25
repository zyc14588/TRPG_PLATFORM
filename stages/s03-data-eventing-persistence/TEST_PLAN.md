# S03 TEST_PLAN — Data/Eventing：PostgreSQL、Event Store、Outbox、Projection、RAG Snapshot

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 测试目标

- SQLx migration 分类验证：forward apply、重复 no-op、旧库 upgrade、备份/恢复与应用回滚
- Event Store append expected_version 冲突
- idempotency key 重放
- outbox exactly-once-ish 发布
- projection replay hash
- RAG snapshot chunk hash 与 visibility 继承

## 推荐命令

- `sqlx migrate info --source migrations`
- `sqlx migrate run --source migrations`（连续执行两次，第二次必须为 no-op）
- `cargo test -p trpg-data-eventing --all-features`
- `cargo test --test event_store_contract`
- `cargo test --test projection_replay`

## Migration 回滚口径

- Event Store、Authority/Identity、审计、Projection checkpoint 与 RAG 来源约束属于
  forward-only 正史/安全迁移，不得用 down migration 删除历史或约束。
- 这类迁移的“反向”验收是：保留已升级 schema，回滚应用二进制或停用 worker，随后用
  备份恢复、forward re-apply 与 replay 验证可恢复性。
- 只有被单独标记为 `reversible`、且不包含正史或安全边界的迁移，才允许在专用空测试库执行
  `run -> revert -> run`。历史 `20260705000100` 的可逆测试不能替代后续 forward-only 迁移验收。

## 必须补齐的测试类型

- Unit：领域纯逻辑、错误码、状态机。
- Integration：数据库、事务、policy、provider、缓存、消息。
- Contract：API/Event/WS/NATS/schema。
- Negative：权限拒绝、version 冲突、幂等冲突、visibility 泄露、prompt injection。
- Golden/Replay：可重放一致性、导出 snapshot、projection hash。

## 阶段测试数据

- `test-data/event_store_stream_cases.md`
- `test-data/rag_snapshot_cases.md`
- `test-data/api_ws_contract_samples.md`

## 失败处理

用同目录 `REPAIR_PROMPT.md`。不得删除测试、关闭 policy gate、绕过 Event Store 或弱化 visibility redaction。
