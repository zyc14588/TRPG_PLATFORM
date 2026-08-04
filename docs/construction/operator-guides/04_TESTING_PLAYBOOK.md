# 测试执行 — v2.21

## 用途

运行单元、集成、契约、fixture、Golden Scenario、Visibility、Docker Compose、release smoke 等测试。

## 必读输入

1. `AGENTS.md`
2. `docs/construction/CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `docs/construction/SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `docs/acceptance/V1_ACCEPTANCE_EVIDENCE_MATRIX.md`

## 可复制给 Codex 的中文提示词

```text
请执行当前阶段测试计划。先读取 `TEST_PLAN.md`、`TEST_DATA.md` 和 `fixtures/**`，再运行 cargo、pnpm、contract、Golden Scenario、visibility、Docker Compose 或 release smoke 中适用的命令。每条命令都要记录 exit code、摘要和 evidence 路径。
```

## 执行步骤

1. 识别阶段测试范围。
2. 解析相关 fixture。
3. 运行最小相关测试。
4. 运行必要仓库级 gate。
5. 记录命令、输出和失败项。

## 命令 / 检查

```powershell
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
pnpm --filter ./apps/web... test
python3 scripts/ci/verify_test_inventory.py
python3 scripts/ci/release_readiness.py --require-ready
```

仓库已有 Web 与浏览器测试入口。Release Readiness 还需要仓库外的完整 evidence、
security evidence 和 V1 candidate matrix；缺少这些参数时必须 fail closed。

## 预期证据

`evidence/stages/SXX/TEST_RESULTS.md`

## 失败处理

测试失败必须阻断验收；不得删除或弱化测试来获得 PASS。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
