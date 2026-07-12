# 测试执行 — v2.21

## 用途

运行单元、集成、契约、fixture、Golden Scenario、Visibility、Docker Compose、release smoke 等测试。

## 必读输入

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`

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
cargo test --workspace --all-features
npm test
python scripts/ci/verify_test_inventory.py
python scripts/ci/release_readiness.py --require-ready
```

当前仓库没有产品前端或可执行 E2E 运行时，不提供 no-op `typecheck` / `test:e2e` 命令；最后一条命令预期在产品运行时落地前 Fail-Closed。

## 预期证据

`evidence/stages/SXX/TEST_RESULTS.md`

## 失败处理

测试失败必须阻断验收；不得删除或弱化测试来获得 PASS。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
