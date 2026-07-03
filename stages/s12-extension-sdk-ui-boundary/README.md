# S12 — Extension SDK 与分层 UI 边界

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 ruleset pack、agent pack、tool provider、plugin SDK 的受限接口；同时定义 Player/KP/Admin/Developer UI 与 API/Realtime 的分层边界，完成最小可玩 Web UI 施工提示词。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S12` |
| 相关分类 | `12-extension-sdk` |
| 相关 batch | BATCH-044-12-extension-sdk, BATCH-045-12-extension-sdk |
| Prompt 数 | 32 |
| Primary | 8 |
| Supplemental | 15 |
| Docs/Trace | 9 |
| 主要 crate | `trpg-extension-sdk` |

## 3. 启动条件

S05/S07/S08 接口稳定；扩展点不得破坏 Authority/Event/Visibility/Policy。

## 4. 主要输出

- `crates/trpg-extension-sdk/src/*.rs`
- `sdk/*.md`
- `apps/web/*`
- `apps/admin-console/*`
- `docs/ui/*.md`

## 5. 测试重点

- SDK capability deny-by-default
- 插件不能写 Event Store
- Agent Pack schema compatibility
- UI 不暴露内部 Tool Gate 概念给玩家
- UI visibility snapshot

## 6. 推荐命令

- `cargo test -p trpg-extension-sdk --all-features`
- `cargo test --test extension_compatibility_matrix`
- `pnpm test --if-present`
- `pnpm build --if-present`

## 7. 测试数据

- `test-data/api_ws_contract_samples.md`
- `test-data/visibility_leakage_cases.md`

## 8. 阶段验收清单

- [ ] 插件/扩展不能绕过 Tool Grant、Policy Gate、Event Store
- [ ] V1 UI 覆盖 Player/KP/Admin/Developer 最小界面
- [ ] 玩家 UI 保持低复杂度，开发者 UI 才展示 Agent debug/audit
- [ ] SDK 向后兼容矩阵存在

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
