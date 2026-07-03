# S04 — Security Governance：OpenFGA/OPA、权限、隐私、版权与审计

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现角色权限矩阵、OpenFGA 关系授权、OPA/Rego 上下文策略、Tool Grant/Policy Gate、Audit Log、数据保留删除、版权边界与平台治理。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S04` |
| 相关分类 | `09-security-governance` |
| 相关 batch | BATCH-035-09-security-governance, BATCH-036-09-security-governance, BATCH-037-09-security-governance |
| Prompt 数 | 54 |
| Primary | 13 |
| Supplemental | 24 |
| Docs/Trace | 17 |
| 主要 crate | `trpg-security-governance` |

## 3. 启动条件

S02/S03 已提供 actor、authority、visibility、event/audit 基础。

## 4. 主要输出

- `crates/trpg-security-governance/src/*.rs`
- `policy/openfga/*.fga`
- `policy/opa/*.rego`
- `crates/trpg-security-governance/tests/*.rs`

## 5. 测试重点

- 关系授权允许/拒绝
- OPA 上下文策略拒绝
- 平台管理员不能覆盖游戏裁定
- 私聊与 keeper_only 泄露负例
- API Key 加密保存与占位 key 生产拒绝

## 6. 推荐命令

- `cargo test -p trpg-security-governance --all-features`
- `cargo test --test visibility_leakage`
- `opa test policy/opa`

## 7. 测试数据

- `test-data/visibility_leakage_cases.md`
- `test-data/permission_matrix_cases.md`
- `test-data/provider_model_certification_cases.md`

## 8. 阶段验收清单

- [ ] Policy Gate 不能被 Agent、插件、handler、provider 绕过
- [ ] 安全暂停不直接改变游戏结果
- [ ] 平台管理权与游戏裁定权分离
- [ ] 版权策略不内置未授权商业规则书/模组全文

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
