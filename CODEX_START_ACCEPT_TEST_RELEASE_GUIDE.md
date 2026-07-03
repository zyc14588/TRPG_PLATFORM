# Codex 启动 / 验收 / 测试 / 发布指南 — v2.21

## 用途

本文件定义四个互不混用的 Codex 会话循环：阶段启动、阶段测试、严格验收、发布准备。它防止 Codex 在同一会话中混合实现、验收、修复和发布声明。

## 必读输入

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
9. `04_TEST_STRATEGY_AND_TEST_DATA.md`
10. `05_CI_CD_CONFIGURATION.md`
11. `codex-operator-guides/04_TESTING_PLAYBOOK.md`
12. `codex-operator-guides/10_STRICT_VALIDATION_COMMANDS.md`

## 可复制给 Codex 的中文流程提示词

```text
请按 v2.21 strict 施工包启动下一个阶段。先列出阶段 scope、目标文件、测试计划、测试数据、fixture、CI/CD 影响和 evidence 路径。实现完成后运行阶段测试，再运行阶段验收。若验收失败，只能使用该阶段 `REPAIR_PROMPT.md` 进行最小范围修复，并在修复后停止等待下一步指令。

执行时必须保持 Authority Contract 不可变、AI 只能经 Agent Gateway / Runtime / Provider Adapter、正式状态只经 Decision Commit Pipeline / State Service / Event Log、visibility 和 Fact Provenance 全链路可审计。任何未运行的测试不得声称 PASS。
```

## 执行步骤

1. 启动循环：输出 readiness evidence。
2. 实现循环：只修改阶段拥有的文件。
3. 测试循环：运行阶段测试和仓库原生 gate。
4. 验收循环：输出带证据的 PASS/FAIL 表。
5. 修复循环：只修 FAIL 项并复跑失败命令。
6. 发布循环：冻结 evidence 并验证 V1 matrix。

## 命令 / 检查

```powershell
New-Item -ItemType Directory -Force evidence/stages/SXX
git diff --name-only
cargo fmt --all -- --check
cargo test --workspace --all-features
pnpm typecheck
pnpm test:e2e
```

## 预期证据

每阶段 readiness、implementation、test、acceptance、repair 证据，以及 S13 后 release candidate 证据。

## 失败处理

启动失败表示前置材料缺失；测试失败阻断验收；验收失败阻断下一阶段；发布失败拒绝 release candidate。

## 退出标准

每个循环都有书面 evidence，并明确下一步操作。
