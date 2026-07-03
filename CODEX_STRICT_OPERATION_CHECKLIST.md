# Codex 严格操作清单 — v2.21

## 用途

本文件是 Codex 输出后的硬门禁清单，用来捕获直接 LLM 调用、Authority Contract 可变、骰子不可信、状态未事件化、visibility 泄露、证据不足和发布闭环缺失等问题。

## 必读输入

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
9. `STRICT_LINK_AND_REFERENCE_VALIDATION.md`
10. `codex-operator-guides/10_STRICT_VALIDATION_COMMANDS.md`

## 可复制给 Codex 的中文验收提示词

```text
请执行 v2.21 strict operation checklist，并为以下硬门禁逐项返回 PASS/FAIL：Authority Contract 不可变、AI 能力只能经 Agent Gateway / Runtime / Provider Adapter、Tool Permission Gate、服务端骰子、Event Log、Visibility Label Propagation、Fact Provenance、COC7 规则闭环、Provider 安全、No Silent Cloud Fallback、CI/CD、Docker Compose、Golden Scenario、Export 隐私、V1 Acceptance closure。

每个 PASS 必须给出文件、命令和 evidence；每个 FAIL 必须给出 P0/P1/P2 等级、精确路径、失败原因和最小修复提示词。不得把未运行的检查写成 PASS。
```

## 执行步骤

1. 搜索 Agent Runtime / Provider Adapter 之外的直接 LLM 调用。
2. 验证 Authority Contract 只能 fork，不可原地修改。
3. 验证 AI KP 正式裁定通过 Tool Gate 与 Event Log。
4. 验证真人 KP 模式下 AI 输出为 draft-only。
5. 验证正式骰子由服务端生成并记录。
6. 验证 visibility 在 memory、RAG、summary、export、replay 中传播。
7. 验证 COC7 测试覆盖 V1 功能。
8. 验证发布和 evidence gate。

## 命令 / 检查

```powershell
rg -n "openai|ollama|llama|chat_completion|responses" .
rg -n "AuthorityContract|FORK_ONLY|authority_mode" .
rg -n "DecisionRecord|DiceRoll|GameEvent|visibility" .
rg -n "docker compose|compose.yml|compose.yaml" .
```

## 预期证据

`evidence/strict/OPERATION_CHECKLIST.md`，每个 hard gate 一行，并链接测试输出。

## 失败处理

任何 hard gate 失败都阻断阶段验收或发布；修复责任层后必须复跑 checklist。

## 退出标准

所有 hard gate 均有命令支撑的 PASS 证据。
