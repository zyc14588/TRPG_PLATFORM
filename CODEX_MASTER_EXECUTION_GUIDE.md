# Codex 总控施工指南 — v2.21

## 用途

本文件是人类程序员启动 Codex 施工前的总控入口。它用于让 Codex 建立项目上下文、识别下一阶段、确认测试与 evidence 责任，并防止直接跳入编码。

## 必读输入

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
9. `02_STAGE_CONFIRMATION_MATRIX.md`
10. `codex-operator-guides/00_QUICK_START.md`
11. `codex-operator-guides/09_CODEX_SESSION_PROMPTS.md`

## 可复制给 Codex 的中文启动提示词

```text
你是 Codex，正在接手 COC AI TRPG v2.21 strict 施工包。当前会话只允许进行只读准备，不得编码、重构、删除文件或修改配置。

请按顺序读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`02_STAGE_CONFIRMATION_MATRIX.md` 与 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`。

请输出 `Stage Readiness Report`，必须包含：仓库当前状态、下一个未验收阶段、该阶段六件套路径、关联 execution batch、关联 per-file prompt 范围、必须运行的测试、需要使用的 fixture、CI/CD 影响、预期 evidence 路径、阻塞项、风险和是否可以进入编码。任何缺失材料都必须标记为 FAIL 或 BLOCKED，不得推测 PASS。
```

## 执行步骤

1. 检查 `git status --short` 和 `git diff --name-only`。
2. 按 `02_STAGE_CONFIRMATION_MATRIX.md` 识别下一阶段。
3. 读取该阶段 `README.md`、`START_PROMPT.md`、`TEST_PLAN.md`、`TEST_DATA.md`、`ACCEPTANCE_PROMPT.md` 与 `REPAIR_PROMPT.md`。
4. 找到关联 batch 和 per-file prompts。
5. 写出 readiness 报告，并停止等待人工确认。

## 命令 / 检查

```powershell
git status --short
git diff --name-only
Get-ChildItem stages -Directory
Get-ChildItem evidence -Recurse -ErrorAction SilentlyContinue
```

## 预期证据

`evidence/operator/STAGE_READINESS_REPORT.md`

## 失败处理

若缺少权威文件、normalized maps、阶段六件套、batch prompt 或 evidence 目录规范，必须停止施工并输出阻塞项。

## 退出标准

只有 readiness 报告明确下一阶段、scope、测试范围、证据路径和阻塞状态后，才允许进入阶段启动。
