# 证据与审计 — v2.21

## 用途

记录命令、AI 裁定、事件、骰子、visibility、export、model-route 和审计证据。

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
请为当前阶段创建或更新 evidence。每个 PASS claim 必须链接命令、文件、fixture、GameEvent、DecisionRecord、DiceRoll、visibility audit 或 export diff。任何私密信息进入 public evidence 都必须标记 P0 FAIL。
```

## 执行步骤

1. 创建 evidence 目录。
2. 记录命令和 exit code。
3. 记录 AI Decision / model route / tool calls。
4. 记录规则、骰子、事件与状态变化。
5. 记录 export redaction diff。

## 命令 / 检查

```powershell
New-Item -ItemType Directory -Force evidence/audit
Get-ChildItem evidence -Recurse
rg -n "DecisionRecord|DiceRoll|GameEvent|visibility|ModelRouteSnapshot|AuditLog" .
```

## 预期证据

`evidence/audit/VISIBILITY_AUDIT.md 与 evidence/stages/SXX/*`

## 失败处理

缺 evidence 不得验收；隐私泄露必须立即阻断。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
