# 故障排查与回滚 — v2.21

## 用途

处理构建、测试、迁移、Docker、provider、visibility、release rollback 与 restore 失败。

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
请排查当前失败。先分类失败类型，定位 owner stage，保存日志，然后提出一个最小修复方案。不得弱化测试或绕过治理。若涉及 migration、backup、restore 或 release，必须说明 rollback / restore 步骤和证据。
```

## 执行步骤

1. 分类失败。
2. 定位 owner stage 和 repair prompt。
3. 保存日志。
4. 做最小补丁。
5. 复跑失败命令和阶段 gate。
6. 记录 rollback/restore 状态。

## 命令 / 检查

```powershell
git diff
git status --short
docker compose logs --tail=200
cargo test --workspace --all-features
pnpm test
```

## 预期证据

`evidence/failures/FAILURE_ANALYSIS.md`

## 失败处理

如果修复失败，不得叠加无关补丁；必须回滚或升级为 blocker。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
