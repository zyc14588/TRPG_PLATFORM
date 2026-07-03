# CI/CD 落库 — v2.21

## 用途

从 Markdown-only workflow source 提取 `.github/workflows/*.yml`，并验证 GitHub Actions YAML。

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
请将 `ci-cd/workflows-extractable/target-*.yml.md` 中第一个 fenced `yaml` 代码块提取到 `.github/workflows/*.yml`。只使用 `workflows-extractable` 作为 canonical source，不得使用 provenance 旧 workflow。提取后验证 YAML、service container command、artifact 路径和 evidence。
```

## 执行步骤

1. 创建 `.github/workflows`。
2. 逐个提取 target workflow YAML。
3. 验证 YAML 解析。
4. 检查 NATS/MinIO 使用 `services.command`。
5. 写 CI extraction evidence。

## 命令 / 检查

```powershell
New-Item -ItemType Directory -Force .github/workflows
Get-ChildItem ci-cd/workflows-extractable -Filter "target-*.yml.md"
python scripts/ci/extract_workflows.py
Get-ChildItem .github/workflows
```

## 预期证据

`evidence/ci/WORKFLOW_EXTRACTION.md`

## 失败处理

如果 YAML 解析失败或出现旧 `services.options` 承载 CMD，必须 FAIL 并修正 source Markdown。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
