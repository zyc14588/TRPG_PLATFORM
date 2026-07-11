> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/90-traceability/readme.md`
> 筛选状态：`active-traceability`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 90-traceability — Codex 模块施工目录

范围：Traceability / source audit / requirement-to-test mapping
默认 crate：`trpg-docs-governance`
默认 module prefix：`traceability`
Per-file prompts：110

## 模块关键要求

追踪文档必须能证明每个源文件、需求、测试和 prompt 的闭环。

## 入口文件

- `AGENTS.md`
- `codex-module-code-prompt.md`
- `codex-module-test-prompt.md`
- `codex-module-review-prompt.md`
- per-file prompts：`codex-prompts/90-traceability/`

<!-- BATCH-046-START -->
## BATCH-046 current-safe readme trace

This marked section satisfies the current-safe docs/codex/90-traceability/readme.md output for B046.

- CODEX-0979-90-TRACEABILITY-c43359535b from codex-prompts/90-traceability/P0012.md maps to traceability::readme and is docs-only.
<!-- BATCH-046-END -->

<!-- BATCH-047-START -->
## BATCH-047 current-safe readme trace

This marked section satisfies the current-safe `docs/codex/90-traceability/readme.md` output for B047.

- CODEX-1014-90-TRACEABILITY-3e2e7e413e from `codex-prompts/90-traceability/P0047.md` maps to `traceability::readme` and is docs-only.
- Provenance source file: `docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-90-traceability-readme-dc8438b8d7.v5-code-ready.md`.
- Provenance source SHA256: `8993b23f1477f440ae03c573784ba12c68003d80b86ed8842a1f62ecd76181d8`.
- No Rust, migration, API, event, NATS, metric, workflow, provider, or formal state-write output is owned by this prompt.
<!-- BATCH-047-END -->

<!-- BATCH-049-START -->
## BATCH-049 current-safe readme trace

The five B049 prompts below share this exact canonical Markdown output. They
are merged here without replacing earlier batch traces or creating
prompt-specific aliases.

| Prompt ID | Prompt file | Current crate | Current module | Source file | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1053-90-TRACEABILITY-91cc9c1979` | `codex-prompts/90-traceability/P0086.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-99-appendix-readme-b62fd66b20.v5-code-ready.md` | `62c70a0dffc838684aaee315e59e31fbee945a9779cccdf2c8196e095d99404b` |
| `CODEX-1054-90-TRACEABILITY-74f2cf5e3b` | `codex-prompts/90-traceability/P0087.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/docs-implementation-readme-4906731413.v5-code-ready.md` | `cc0df06102546c24cc442d12727665807920a6973de2791f9697a2684cdc7157` |
| `CODEX-1056-90-TRACEABILITY-cc337fd4d2` | `codex-prompts/90-traceability/P0089.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/readme-8ebbb20182.v5-code-ready.md` | `e6a29631f029a10e177a521164e75f8f3b5e61294e3d930a62c679a70ea4c496` |
| `CODEX-1058-90-TRACEABILITY-d3a03a9d63` | `codex-prompts/90-traceability/P0091.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-00-index-readme-8ce4f7c2ad.v5-code-ready.md` | `4aad1df5b3e03162ce1933d0e1bb59b2033bc00d90db1f80e7d8e64ee143b423` |
| `CODEX-1064-90-TRACEABILITY-20708447f6` | `codex-prompts/90-traceability/P0097.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-90-traceability-readme-e7268538a3.v5-code-ready.md` | `990e5435b418fd4ea381cc2d43ed7a5bbb372cec580ba9b4834053b3a06b91e9` |

- All five rows are implemented as additive docs-only traceability.
- Historical path, version, and hash tokens are provenance only.
- No row owns Rust, migration, API, event, NATS, metric, workflow, provider,
  product-test, or formal state-write output.
- Test evidence is recorded in `evidence/batches/BATCH-049/test-output.txt`.
<!-- BATCH-049-END -->

<!-- BATCH-050-START -->
## BATCH-050 current-safe readme trace

The three B050 prompts below share this exact canonical Markdown output. They
are merged here without replacing earlier batch traces or creating
prompt-specific aliases.

| Prompt ID | Prompt file | Current crate | Current module | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|
| `CODEX-1068-90-TRACEABILITY-c50cc50eaf` | `codex-prompts/90-traceability/P0101.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-99-appendix-readme-8ddeaed7a2.v5-code-ready.md` | `6588e7b3b50b275f58aa272d354d053a721176ce47225b635c47c6cea03886b6` |
| `CODEX-1069-90-TRACEABILITY-e8e261e885` | `codex-prompts/90-traceability/P0102.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-docs-implementation-readme-5b65f2638b.v5-code-ready.md` | `848f5fc34f2d93dcc16e2641dd167e97ca25bd35f9ef85dd2f7dab4e8dfa28c6` |
| `CODEX-1071-90-TRACEABILITY-289dbea468` | `codex-prompts/90-traceability/P0104.md` | `trpg-docs-governance` | `traceability::readme` | `docs/implementation/90-traceability/per-file-code-ready/90-traceability/sources-v3-baseline-document-group-readme-9483616cdb.v5-code-ready.md` | `e85c37962f570ba4ab7cd6e7d5b33490501488a2f546cc45d55818faf3834c77` |

- All three rows are implemented as additive docs-only traceability.
- Historical path, version, and hash tokens are provenance only.
- No row owns Rust, migration, API, event, NATS, metric, workflow, provider,
  product-test, or formal state-write output.
- B050 checks must verify all three Prompt IDs, prompt and provenance paths,
  source SHA256 values, current-safe map agreement, Markdown table shape, and
  the docs-only boundary.
<!-- BATCH-050-END -->
