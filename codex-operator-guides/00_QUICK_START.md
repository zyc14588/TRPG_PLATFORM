# 快速启动 — v2.21

## 用途

首次启动 Codex。目标是建立上下文、检查仓库和 evidence 状态、识别下一阶段，不做任何修改。

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
你是 Codex，正在接手 COC AI TRPG v2.21 strict 施工包。请先读取权威文件、normalized maps 和 V1 验收矩阵。当前只允许只读检查，不得修改代码或文档。请输出 `evidence/operator/BOOTSTRAP_READINESS.md`，说明仓库状态、当前阶段、下一阶段、必读文件、测试范围、证据路径和阻塞项。
```

## 执行步骤

1. 确认包位置与仓库根目录。
2. 读取权威文件和 normalized maps。
3. 检查 `git status --short`。
4. 检查已有 `evidence/**`。
5. 识别下一个未验收阶段并停止。

## 命令 / 检查

```powershell
Get-ChildItem -Force
Get-ChildItem evidence -Recurse -ErrorAction SilentlyContinue
git status --short
```

## 预期证据

`evidence/operator/BOOTSTRAP_READINESS.md`

## 失败处理

若仓库不干净或权威文件缺失，停止并报告，不得进入编码。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
