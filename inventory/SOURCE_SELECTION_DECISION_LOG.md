# Source Selection Decision Log

本文件说明 v2.21 包如何遍历、筛选、嵌入、隔离和清洗所有输入材料。

## 输入源统计

| 输入源 | 条目数 |
|---|---:|
| original-v6-codex-strict-governance | 1299 |
| previous-strict-acceptance-review | 1 |
| previous-v1-construction-plan | 132 |
| top-level-design | 1 |
| v2-stale-manifests | 1 |
| v2-strict-reacceptance-review | 1 |
| v2.21-normalization-maps | 1 |

## 筛选状态统计

| 状态 | 条目数 | 处理原则 |
|---|---:|---|
| `active-batch` | 52 | 作为 batch 编排材料嵌入 `batches/B###.md`。 |
| `active-domain-category` | 72 | 作为当前 Codex 工程施工的领域材料嵌入 `docs/codex/**`。 |
| `active-index` | 11 | 作为当前 Codex 持久上下文和边界入口嵌入 `docs/codex/00-index/**`。 |
| `active-prompt` | 1109 | 作为 per-file 施工 prompt 嵌入 `codex-prompts/<category>/P####.md`。 |
| `active-reference-overlaid` | 1 | 仅作为原始参考保留；当前根 AGENTS.md 已重写。 |
| `active-traceability` | 11 | 作为当前 traceability 材料嵌入，但不得覆盖 v2.21 根门禁。 |
| `appendix-reference` | 11 | 作为附录参考嵌入；只在对应阶段明确需要时读取，不得覆盖顶层设计。 |
| `current-authoritative-baseline` | 1 | 当前最高顶层设计基线。 |
| `current-normalization-authority` | 1 | 当前安全 module/output/prompt 规范化 overlay。 |
| `quarantined-provenance` | 25 | 转入 quarantine；不得作为当前需求、验收或施工入口。 |
| `repair-input-provenance` | 2 | 严格验收失败项与修复依据。 |
| `screened-provenance` | 7 | 转入 provenance；不得作为当前施工入口。 |
| `superseded-provenance` | 1 | 旧 manifest、旧报告、旧校验材料只保留为 provenance。 |
| `v1-inventory-superseded` | 5 | 旧 inventory/manifest 只作历史参考，当前以 v2.21 manifest 为准。 |
| `v1-plan-carried-forward` | 41 | 保留上次方案内容并纳入 v2.21 strict 包。 |
| `v1-plan-overwritten-by-v2` | 2 | 被 v2 系列根文档重写；当前以 v2.21 版本为准。 |
| `v1-stage-doc-overlaid-by-v2` | 84 | 阶段文档保留并插入 v2.21 前置说明。 |

## 状态示例

### active-batch

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `batches/B001.md` | `batches/B001.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B002.md` | `batches/B002.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B003.md` | `batches/B003.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B004.md` | `batches/B004.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B005.md` | `batches/B005.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B006.md` | `batches/B006.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B007.md` | `batches/B007.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| original-v6-codex-strict-governance | `batches/B008.md` | `batches/B008.md` | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |

### active-domain-category

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `docs/codex/01-foundation/AGENTS.md` | `docs/codex/01-foundation/AGENTS.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/01-foundation/README.md` | `docs/codex/01-foundation/README.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/01-foundation/codex-module-code-prompt.md` | `docs/codex/01-foundation/codex-module-code-prompt.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/01-foundation/codex-module-review-prompt.md` | `docs/codex/01-foundation/codex-module-review-prompt.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/01-foundation/codex-module-test-prompt.md` | `docs/codex/01-foundation/codex-module-test-prompt.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/01-foundation/per-file-prompt-manifest.md` | `docs/codex/01-foundation/per-file-prompt-manifest.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/02-domain-core/AGENTS.md` | `docs/codex/02-domain-core/AGENTS.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| original-v6-codex-strict-governance | `docs/codex/02-domain-core/README.md` | `docs/codex/02-domain-core/README.md` | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |

### active-index

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `docs/codex/00-index/AGENTS.md` | `docs/codex/00-index/AGENTS.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/README.md` | `docs/codex/00-index/readme.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-batch-plan.md` | `docs/codex/00-index/codex-batch-plan.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-execution-map.md` | `docs/codex/00-index/codex-execution-map.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-module-code-prompt.md` | `docs/codex/00-index/codex-module-code-prompt.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-module-review-prompt.md` | `docs/codex/00-index/codex-module-review-prompt.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-module-test-prompt.md` | `docs/codex/00-index/codex-module-test-prompt.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| original-v6-codex-strict-governance | `docs/codex/00-index/codex-persistent-context.md` | `docs/codex/00-index/codex-persistent-context.md` | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |

### active-prompt

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0001.md` | `codex-prompts/00-index/P0001.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0002.md` | `codex-prompts/00-index/P0002.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0003.md` | `codex-prompts/00-index/P0003.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0004.md` | `codex-prompts/00-index/P0004.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0005.md` | `codex-prompts/00-index/P0005.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0007.md` | `codex-prompts/00-index/P0007.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0006.md` | `codex-prompts/00-index/P0006.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| original-v6-codex-strict-governance | `codex-prompts/00-index/P0008.md` | `codex-prompts/00-index/P0008.md` | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |

### active-reference-overlaid

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `AGENTS.md` | `source-archive/v6-root/S0001.md` | 原 AGENTS 作为参考；v2 根 AGENTS.md 已重写并继承其治理约束。 |

### active-traceability

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `docs/codex/90-traceability/AGENTS.md` | `docs/codex/90-traceability/AGENTS.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/README.md` | `docs/codex/90-traceability/README.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/codex-module-code-prompt.md` | `docs/codex/90-traceability/codex-module-code-prompt.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/codex-module-review-prompt.md` | `docs/codex/90-traceability/codex-module-review-prompt.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/codex-module-test-prompt.md` | `docs/codex/90-traceability/codex-module-test-prompt.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/previous-disposition-matrix-codex.md` | `docs/codex/90-traceability/previous-disposition-matrix-codex.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/output-path-ownership-matrix.md` | `docs/codex/90-traceability/output-path-ownership-matrix.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/per-file-prompt-index.md` | `docs/codex/90-traceability/per-file-prompt-index.md` | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |

### appendix-reference

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `docs/codex/99-appendix/AGENTS.md` | `docs/codex/99-appendix/AGENTS.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/README.md` | `docs/codex/99-appendix/README.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-module-code-prompt.md` | `docs/codex/99-appendix/codex-module-code-prompt.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-module-review-prompt.md` | `docs/codex/99-appendix/codex-module-review-prompt.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-module-test-prompt.md` | `docs/codex/99-appendix/codex-module-test-prompt.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-official-reference-notes.md` | `docs/codex/99-appendix/codex-official-reference-notes.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-prompt-template.md` | `docs/codex/99-appendix/codex-prompt-template.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| original-v6-codex-strict-governance | `docs/codex/99-appendix/codex-test-command-catalog.md` | `docs/codex/99-appendix/codex-test-command-catalog.md` | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |

### current-authoritative-baseline

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| top-level-design | `coc_ai_trpg_top_level_design.md` | `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md` | 当前顶层设计基线；v2 所有施工、验收和清洗均以此为最高业务/架构依据。 |

### quarantined-provenance

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-code-strict-fix-report.md` | `source-archive/quarantined/S0001.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-full-module-layout-fix-audit-v6.md` | `source-archive/quarantined/S0002.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-full-strict-fix-audit-v6.md` | `source-archive/quarantined/S0003.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-initial-codex-audit.md` | `source-archive/quarantined/S0004.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-initial-codex-delivery-report.md` | `source-archive/quarantined/S0005.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-module-layout-audit.md` | `source-archive/quarantined/S0006.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-module-layout-fix-report.md` | `source-archive/quarantined/S0007.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| original-v6-codex-strict-governance | `docs/codex/90-traceability/fix-history/archived-module-layout-validation.md` | `source-archive/quarantined/S0008.md` | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |

### repair-input-provenance

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| previous-strict-acceptance-review | `coc_ai_trpg_codex_construction_plan_strict_acceptance_review.md` | `source-archive/superseded/S0009.md` | 本次修复依据；已转入 source-archive/provided-review-provenance/ 作为严格验收失败项与修复闭环证据。 |

### screened-provenance

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| original-v6-codex-strict-governance | `CODEX_DELIVERY_REPORT.md` | `source-archive/v6-root/S0002.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `CODEX_STRICT_GOVERNANCE_FIX_REPORT.md` | `source-archive/v6-root/S0003.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `MANIFEST.md` | `source-archive/v6-root/S0004.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `README.md` | `source-archive/v6-root/S0005.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `STRICT_CODEX_SEMANTIC_VALIDATION.md` | `source-archive/v6-root/S0006.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `VALIDATION.md` | `source-archive/v6-root/S0007.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |
| original-v6-codex-strict-governance | `strict_codex_governance_audit.json.md` | `source-archive/v6-root/S0008.md` | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |

### v1-inventory-superseded

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| previous-v1-construction-plan | `inventory/CODEX_PROMPT_INVENTORY.md` | `inventory/CODEX_PROMPT_INVENTORY.md` | 上次索引/manifest 已保留或被 v2 新索引补充，当前以 v2 strict manifest 为准。 |
| previous-v1-construction-plan | `inventory/EXECUTION_BATCH_MAPPING.md` | `inventory/EXECUTION_BATCH_MAPPING.md` | 上次索引/manifest 已保留或被 v2 新索引补充，当前以 v2 strict manifest 为准。 |
| previous-v1-construction-plan | `inventory/INPUT_FILE_INVENTORY.md` | `inventory/INPUT_FILE_INVENTORY.md` | 上次索引/manifest 已保留或被 v2 新索引补充，当前以 v2 strict manifest 为准。 |
| previous-v1-construction-plan | `source-archive/superseded/S0014.md` | `source-archive/superseded/S0014.md` | 上次索引/manifest 已保留或被 v2 新索引补充，当前以 v2 strict manifest 为准。 |
| previous-v1-construction-plan | `manifests/README.md` | `manifests/README.md` | 上次索引/manifest 已保留或被 v2 新索引补充，当前以 v2 strict manifest 为准。 |

### v1-plan-carried-forward

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| previous-v1-construction-plan | `01_OVERALL_CONSTRUCTION_PLAN.md` | `01_OVERALL_CONSTRUCTION_PLAN.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `02_STAGE_CONFIRMATION_MATRIX.md` | `02_STAGE_CONFIRMATION_MATRIX.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `03_ENGINEERING_DIRECTORY_PLAN.md` | `03_ENGINEERING_DIRECTORY_PLAN.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `04_TEST_STRATEGY_AND_TEST_DATA.md` | `04_TEST_STRATEGY_AND_TEST_DATA.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `05_CI_CD_CONFIGURATION.md` | `05_CI_CD_CONFIGURATION.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `source-archive/provenance/S0001.md` | `source-archive/provenance/S0001.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `source-archive/provenance/S0002.md` | `source-archive/provenance/S0002.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |
| previous-v1-construction-plan | `source-archive/provenance/S0003.md` | `source-archive/provenance/S0003.md` | 上次施工方案基础文件；v2 已保留并叠加自包含修复前置、源材料集成、fixture 与严格验收矩阵。 |

### v1-plan-overwritten-by-v2

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| previous-v1-construction-plan | `00_INPUT_ANALYSIS_AND_TRACEABILITY.md` | `00_INPUT_ANALYSIS_AND_TRACEABILITY.md` | 上次方案文件已被 v2 自包含修复版重写，以当前 strict acceptance 为准。 |
| previous-v1-construction-plan | `README.md` | `README.md` | 上次方案文件已被 v2 自包含修复版重写，以当前 strict acceptance 为准。 |

### v1-stage-doc-overlaid-by-v2

| 输入源 | 原路径 | v2.21 包内路径 | 原因 |
|---|---|---|---|
| previous-v1-construction-plan | `stages/s00-governance-onboarding/ACCEPTANCE_PROMPT.md` | `stages/s00-governance-onboarding/ACCEPTANCE_PROMPT.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s00-governance-onboarding/README.md` | `stages/s00-governance-onboarding/README.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s00-governance-onboarding/REPAIR_PROMPT.md` | `stages/s00-governance-onboarding/REPAIR_PROMPT.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s00-governance-onboarding/START_PROMPT.md` | `stages/s00-governance-onboarding/START_PROMPT.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s00-governance-onboarding/TEST_DATA.md` | `stages/s00-governance-onboarding/TEST_DATA.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s00-governance-onboarding/TEST_PLAN.md` | `stages/s00-governance-onboarding/TEST_PLAN.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s01-foundation-shared-kernel/ACCEPTANCE_PROMPT.md` | `stages/s01-foundation-shared-kernel/ACCEPTANCE_PROMPT.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |
| previous-v1-construction-plan | `stages/s01-foundation-shared-kernel/README.md` | `stages/s01-foundation-shared-kernel/README.md` | 上次阶段文档已保留并插入 v2 自包含前置说明和 fixture 入口。 |


## v2.21 追加筛选记录

| 输入源 | 原路径 | 筛选状态 | v2.21 包内路径 | 筛选原因 |
|---|---|---|---|---|
| v2-strict-reacceptance-review | `coc_ai_trpg_codex_v2_strict_reacceptance_review.md` | `repair-input-provenance` | `source-archive/superseded/S0011.md` | 本轮严格验收失败依据；用于修复，不是当前通过结论。 |
| v2-stale-manifests | `source-archive/superseded/S0014.md` 等 4 个文件 | `superseded-provenance` | `source-archive/superseded/S0014.md` | v2 旧 manifest 已归档，不得作为当前 manifest 或 PASS/FAIL 入口。 |
| v2.21-normalization-maps | `CURRENT_*` 新增文档 | `current-normalization-authority` | `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md` | 当前 Codex 执行旧版本 token 清理、path 解析、module/output 规范化的强制入口。 |
