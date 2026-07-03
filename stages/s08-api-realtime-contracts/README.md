# S08 — API / Realtime：REST、OpenAPI、WebSocket、NATS Contract、服务二进制

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 Axum API、utoipa OpenAPI、WebSocket 房间同步/断线重连、多 active_scene、私聊/暗骰/分组调查、provider/admin endpoints、request idempotency 与 contract tests。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S08` |
| 相关分类 | `07-api-realtime-contracts` |
| 相关 batch | BATCH-029-07-api-realtime-contracts, BATCH-030-07-api-realtime-contracts |
| Prompt 数 | 48 |
| Primary | 16 |
| Supplemental | 21 |
| Docs/Trace | 11 |
| 主要 crate | `trpg-api` |

## 3. 启动条件

S02/S03/S04/S06/S07 提供 domain/event/policy/runtime/agent 服务接口。

## 4. 主要输出

- `crates/trpg-api/src/*.rs`
- `apps/api-server/src/main.rs`
- `apps/realtime-server/src/main.rs`
- `openapi/*.yaml`
- `schemas/websocket/*.json`

## 5. 测试重点

- OpenAPI contract
- WebSocket delta visibility
- request idempotency
- 角色/房间权限
- provider connection test
- 多人分组调查同步
- NATS subject contract

## 6. 推荐命令

- `cargo test -p trpg-api --all-features`
- `cargo test --test openapi_contract`
- `cargo test --test websocket_contract`
- `cargo test --test nats_subject_contract`

## 7. 测试数据

- `test-data/api_ws_contract_samples.md`
- `test-data/seed_users_campaigns.md`
- `test-data/visibility_leakage_cases.md`

## 8. 阶段验收清单

- [ ] 前端不能直接调用模型服务
- [ ] handler 必须传递 actor、visibility、provenance、correlation_id
- [ ] WebSocket 对不同 principal 下发不同可见 delta
- [ ] OpenAPI 与 DTO/schema/test 同步

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
