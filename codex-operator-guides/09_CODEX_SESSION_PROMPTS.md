# Codex 会话提示词参考 — v2.21

## 用途

给人类程序员复制使用的中文会话提示词库。每次只选择一种会话类型。

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
请使用本提示词库。每次只选择一种会话类型执行。执行前说明所选会话类型，执行后写入 evidence，再给出结论。不得把未运行的命令或未生成的 evidence 声称为 PASS。
```

## 执行步骤

1. bootstrap 后再实现。
2. 阶段实现后再验收。
3. 验收 FAIL 后再修复。
4. CI 落库单独会话。
5. 发布准备仅在 S13 后执行。

## 命令 / 检查

```powershell
git status --short
git diff --name-only
Get-ChildItem evidence -Recurse -ErrorAction SilentlyContinue
```

## 预期证据

`evidence/operator/sessions/`

## 失败处理

混用会话类型时必须停止并重新选择。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。


## 会话提示词库

### Bootstrap

```text
请读取 v2.21 权威文件并检查仓库状态。不要修改文件。输出 Stage Readiness Report，列出下一阶段、必读文件、测试范围、evidence 路径和阻塞项。
```

### 阶段实现

```text
请只实现 SXX 阶段。读取该阶段六件套，套用 normalized maps，按阶段 scope 修改文件，运行测试，并写入 `evidence/stages/SXX/IMPLEMENTATION_SUMMARY.md`。
```

### 阶段验收

```text
请对 SXX 执行严格验收。每个 PASS 必须有文件、命令和 evidence；每个 FAIL 必须给出修复范围、失败等级和可复制的修复提示词。
```

### 失败修复

```text
请只修复 SXX 验收中的 FAIL 项。先复现失败命令，再做最小补丁，然后复跑失败命令和完整阶段测试。更新 `REPAIR_LOG.md`。
```

### CI 提取

```text
请将 `ci-cd/workflows-extractable/*.md` 中第一个 fenced yaml 代码块提取为 `.github/workflows/*.yml`，验证 YAML，并写入 `evidence/ci/WORKFLOW_EXTRACTION.md`。
```

### 发布准备

```text
请准备 release candidate。填充 V1 matrix，运行 CI、Docker Compose、Golden Scenario、backup/restore 与 rollback 检查。没有全 PASS 前不得创建 tag。
```

### 严格校验

```text
请执行 strict validation：检查 manifest、batch prompt refs、cleanup audit、guide quality、manual batch prompts、中文 manual prompt 覆盖、fixture 充分性、CI/CD、active stale information 和 actionable legacy hits。必须如实报告 PASS/FAIL。
```

## batch 级手动提示词参考

52 个 batch 级手动启动提示词和 52 个 batch 级手动验收提示词位于 `batch-prompts/`。执行任何 batch 前，先选择对应的 `B###.md`；执行完成后再使用对应的 `B###.md`。
