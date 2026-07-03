# 发布准备 — v2.21

## 用途

准备 release candidate、证据冻结、rollback、restore 和 V1 acceptance closure。

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
请准备 release candidate，但不要创建 tag。先冻结 evidence，填充 V1 acceptance matrix，运行 CI、Docker Compose smoke、Golden Scenario、backup/restore、rollback 和 export privacy 检查。缺证据的 V1 项必须标记 FAIL。
```

## 执行步骤

1. 确认 S00-S13 验收证据。
2. 冻结 evidence 快照。
3. 填充 V1 矩阵。
4. 运行 release gate。
5. 输出 rollback / restore 计划。

## 命令 / 检查

```powershell
git status --short
cargo test --workspace --all-features
pnpm test:e2e
docker compose config
docker compose up -d
```

## 预期证据

`evidence/release/RELEASE_CANDIDATE_REPORT.md`

## 失败处理

任何 V1 必须项缺证据均阻断 release candidate。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
