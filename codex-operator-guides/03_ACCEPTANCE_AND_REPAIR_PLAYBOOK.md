# 验收与修复 — v2.21

## 用途

对阶段或 batch 做严格 PASS/FAIL 验收；失败后只允许最小范围修复。

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
请对 SXX 或 BATCH-NNN 执行严格验收。不得新增功能。每个 PASS 必须有文件、命令和 evidence；每个 FAIL 必须给出等级、路径、原因和可复制修复提示词。若用户要求修复，只修 FAIL 项并复跑失败命令。
```

## 执行步骤

1. 读取对应 `ACCEPTANCE_PROMPT.md` 或 batch acceptance prompt。
2. 复查变更文件与 scope。
3. 复跑相关测试。
4. 输出 PASS/FAIL 表。
5. FAIL 时生成最小修复提示词。

## 命令 / 检查

```powershell
git diff --name-only
Get-ChildItem evidence -Recurse -ErrorAction SilentlyContinue
cargo test --workspace --all-features
pnpm test
```

## 预期证据

`evidence/stages/SXX/ACCEPTANCE_REPORT.md 或 evidence/batches/BATCH-NNN/acceptance-report.md`

## 失败处理

不得把未运行测试写成 PASS；不得在验收会话直接扩功能。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
