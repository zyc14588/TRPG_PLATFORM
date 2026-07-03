# S01 — Rust Workspace 与 shared kernel 基座

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

创建 Cargo workspace、crate ownership、错误模型、配置模型、时间/ID/Actor/CommandEnvelope、Visibility/Provenance 基础类型与依赖方向。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S01` |
| 相关分类 | `01-foundation` |
| 相关 batch | BATCH-003-01-foundation, BATCH-004-01-foundation, BATCH-005-01-foundation, BATCH-006-01-foundation |
| Prompt 数 | 98 |
| Primary | 22 |
| Supplemental | 51 |
| Docs/Trace | 25 |
| 主要 crate | `trpg-shared-kernel` |

## 3. 启动条件

S00 通过；Codex 已能读取 foundation 模块提示词与 per-file prompts。

## 4. 主要输出

- `Cargo.toml`
- `crates/trpg-shared-kernel/src/*.rs`
- `crates/trpg-shared-kernel/tests/*_contract_tests.rs`
- `docs/adr/adr-0001-rust-first.md`

## 5. 测试重点

- workspace 编译
- 共享类型序列化兼容
- 错误码稳定性
- 依赖方向检测
- 配置加载测试

## 6. 推荐命令

- `cargo fmt --all -- --check`
- `cargo clippy -p trpg-shared-kernel --all-targets --all-features -- -D warnings`
- `cargo test -p trpg-shared-kernel --all-features`

## 7. 测试数据

- `test-data/seed_users_campaigns.md`
- `test-data/authority_contract_cases.md`

## 8. 阶段验收清单

- [ ] 所有公开类型使用领域专名，不出现 ModuleService/ModuleCommand 模板残留
- [ ] CommandEnvelope 携带 idempotency_key、expected_version、actor、correlation_id、causation_id
- [ ] shared-kernel 不依赖 domain/runtime/agent/api
- [ ] serde_json::Value 只出现在 schema boundary 说明允许的位置

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
