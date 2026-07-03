# 严格校验命令 — v2.21

## 用途

提供外部只读校验命令，覆盖 ZIP、manifest、audit、batch、中文 prompt、fixture、CI/CD 和旧信息。

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
请从全新解压目录对 v2.21 strict 包执行外部严格校验。必须重新计算 ZIP 完整性、manifest 行数与 SHA256、cleanup audit 覆盖、batch prompt 引用、52 个 batch 手动启动提示词、52 个 batch 手动验收提示词、中文手动 prompt 覆盖、14 个 detailed fixture 字段充分性、CI/CD YAML 解析、Markdown fence、BOM、CRLF、行尾空格、guide 可读性、active stale information 和 actionable legacy hits。每一项都必须给出 PASS/FAIL；未实际运行的检查不得写成 PASS。
```

## 执行步骤

1. 全新解压 ZIP。
2. 重算 manifest 与 audit。
3. 校验 batch prompt refs。
4. 校验中文 prompt 覆盖。
5. 解析 JSON/YAML。
6. 执行 strict validation 代码块。

## 命令 / 检查

```powershell
$PACKAGE_ZIP = "coc_ai_trpg_codex_strict_self_contained_construction_pack_v2_21_strict.zip"
unzip -t $PACKAGE_ZIP
Get-ChildItem -Recurse -File | Where-Object { $_.Extension -eq ".md" } | Measure-Object
rg -n "PENDING.*ARCHIVE|FAIL" . -g "*.md"
```

## 预期证据

`evidence/strict/EXTERNAL_VALIDATION_REPORT.md`

## 失败处理

任何失败都要求修源文件、重建 manifest、重建 ZIP 并重跑。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
