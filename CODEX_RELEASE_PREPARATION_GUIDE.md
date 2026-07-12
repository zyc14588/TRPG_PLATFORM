# Codex 发布准备指南 — v2.21

## 用途

本文件用于 S13 之后准备 release candidate。它要求 Codex 冻结 evidence、填充 V1 验收矩阵、运行发布测试，并准备 rollback 证据；没有完整证据时不得创建 tag 或发布。

## 必读输入

1. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
2. `stages/s13-v1-release-hardening/**`
3. `04_TEST_STRATEGY_AND_TEST_DATA.md`
4. `05_CI_CD_CONFIGURATION.md`
5. `ci-cd/workflows-extractable/**`
6. `fixtures/**`
7. `codex-operator-guides/06_RELEASE_PREPARATION_PLAYBOOK.md`
8. 全部 stage acceptance evidence

## 可复制给 Codex 的中文发布准备提示词

```text
请准备 v2.21 release candidate，但不要创建 tag，也不要发布。先冻结当前 evidence，然后逐项填充 V1 acceptance matrix。必须运行或确认 CI、contracts、Golden Scenario、Visibility leakage、Docker Compose smoke、backup/restore、migration rollback、model certification 和 export privacy 检查。任何缺少证据的 V1 项必须标记 FAIL，而不是推测 PASS。请输出 release candidate report、rollback plan 和 open risks。
```

## 执行步骤

1. 确认 S00 至 S13 均有 acceptance evidence。
2. 填充 `docs/reports/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md`。
3. 运行 CI/CD、Golden Scenario、Docker Compose smoke、backup/restore 与 rollback 检查。
4. 汇总 release notes、migration notes、operator notes 和 known risks。
5. 只在全 PASS 时输出“可进入人工 release approval”。

## 命令 / 检查

```powershell
git status --short
npm test
cargo test --workspace --all-features
python scripts/ci/release_readiness.py --require-ready
```

当前仓库尚无产品前端或 E2E 运行时，因此不提供 `pnpm test:e2e` 空脚本；Release Readiness 在真实运行时出现前必须返回 `BLOCKED`。

## 预期证据

- `evidence/release/RELEASE_CANDIDATE_REPORT.md`
- `evidence/release/V1_ACCEPTANCE_MATRIX_FILLED.md`
- `evidence/release/ROLLBACK_PLAN.md`
- `evidence/release/KNOWN_RISKS.md`

## 失败处理

任何 V1 必须项缺证据都阻断 release candidate；修复对应阶段并重新跑发布准备。

## 退出标准

release candidate 只能在 V1 17 项均有命令或 artifact 证据时进入人工批准。
