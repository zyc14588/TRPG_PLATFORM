> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/00-index/codex-batch-plan.md`
> 筛选状态：`active-index`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Codex Batch Plan（Strict Governance Final）

> Batch 数量：52
> Prompt 覆盖：1109 / 1109
> 批次执行时必须遵守 semantic ownership：primary 创建 Rust 输出，supplemental 只归并需求，documentation 只维护追踪文档。

| Batch ID | 目录 | Prompt 数量 | Batch 文件 |
|---|---|---:|---|
| BATCH-001-00-index | 00-index | 25 | batches/B001.md |
| BATCH-002-00-index | 00-index | 23 | batches/B002.md |
| BATCH-003-01-foundation | 01-foundation | 25 | batches/B003.md |
| BATCH-004-01-foundation | 01-foundation | 25 | batches/B004.md |
| BATCH-005-01-foundation | 01-foundation | 25 | batches/B005.md |
| BATCH-006-01-foundation | 01-foundation | 23 | batches/B006.md |
| BATCH-007-02-domain-core | 02-domain-core | 25 | batches/B007.md |
| BATCH-008-02-domain-core | 02-domain-core | 25 | batches/B008.md |
| BATCH-009-02-domain-core | 02-domain-core | 25 | batches/B009.md |
| BATCH-010-02-domain-core | 02-domain-core | 25 | batches/B010.md |
| BATCH-011-02-domain-core | 02-domain-core | 6 | batches/B011.md |
| BATCH-012-03-runtime-orchestration | 03-runtime-orchestration | 25 | batches/B012.md |
| BATCH-013-03-runtime-orchestration | 03-runtime-orchestration | 25 | batches/B013.md |
| BATCH-014-03-runtime-orchestration | 03-runtime-orchestration | 25 | batches/B014.md |
| BATCH-015-03-runtime-orchestration | 03-runtime-orchestration | 25 | batches/B015.md |
| BATCH-016-03-runtime-orchestration | 03-runtime-orchestration | 15 | batches/B016.md |
| BATCH-017-04-ai-agent-system | 04-ai-agent-system | 25 | batches/B017.md |
| BATCH-018-04-ai-agent-system | 04-ai-agent-system | 25 | batches/B018.md |
| BATCH-019-04-ai-agent-system | 04-ai-agent-system | 25 | batches/B019.md |
| BATCH-020-04-ai-agent-system | 04-ai-agent-system | 20 | batches/B020.md |
| BATCH-021-05-ruleset-coc7 | 05-ruleset-coc7 | 25 | batches/B021.md |
| BATCH-022-05-ruleset-coc7 | 05-ruleset-coc7 | 25 | batches/B022.md |
| BATCH-023-05-ruleset-coc7 | 05-ruleset-coc7 | 15 | batches/B023.md |
| BATCH-024-06-data-eventing | 06-data-eventing | 25 | batches/B024.md |
| BATCH-025-06-data-eventing | 06-data-eventing | 25 | batches/B025.md |
| BATCH-026-06-data-eventing | 06-data-eventing | 25 | batches/B026.md |
| BATCH-027-06-data-eventing | 06-data-eventing | 25 | batches/B027.md |
| BATCH-028-06-data-eventing | 06-data-eventing | 7 | batches/B028.md |
| BATCH-029-07-api-realtime-contracts | 07-api-realtime-contracts | 25 | batches/B029.md |
| BATCH-030-07-api-realtime-contracts | 07-api-realtime-contracts | 23 | batches/B030.md |
| BATCH-031-08-platform-infrastructure | 08-platform-infrastructure | 25 | batches/B031.md |
| BATCH-032-08-platform-infrastructure | 08-platform-infrastructure | 25 | batches/B032.md |
| BATCH-033-08-platform-infrastructure | 08-platform-infrastructure | 25 | batches/B033.md |
| BATCH-034-08-platform-infrastructure | 08-platform-infrastructure | 2 | batches/B034.md |
| BATCH-035-09-security-governance | 09-security-governance | 25 | batches/B035.md |
| BATCH-036-09-security-governance | 09-security-governance | 25 | batches/B036.md |
| BATCH-037-09-security-governance | 09-security-governance | 4 | batches/B037.md |
| BATCH-038-10-testing-quality | 10-testing-quality | 25 | batches/B038.md |
| BATCH-039-10-testing-quality | 10-testing-quality | 25 | batches/B039.md |
| BATCH-040-10-testing-quality | 10-testing-quality | 25 | batches/B040.md |
| BATCH-041-10-testing-quality | 10-testing-quality | 3 | batches/B041.md |
| BATCH-042-11-ops-migration | 11-ops-migration | 25 | batches/B042.md |
| BATCH-043-11-ops-migration | 11-ops-migration | 18 | batches/B043.md |
| BATCH-044-12-extension-sdk | 12-extension-sdk | 25 | batches/B044.md |
| BATCH-045-12-extension-sdk | 12-extension-sdk | 7 | batches/B045.md |
| BATCH-046-90-traceability | 90-traceability | 25 | batches/B046.md |
| BATCH-047-90-traceability | 90-traceability | 25 | batches/B047.md |
| BATCH-048-90-traceability | 90-traceability | 25 | batches/B048.md |
| BATCH-049-90-traceability | 90-traceability | 25 | batches/B049.md |
| BATCH-050-90-traceability | 90-traceability | 10 | batches/B050.md |
| BATCH-051-99-appendix | 99-appendix | 25 | batches/B051.md |
| BATCH-052-99-appendix | 99-appendix | 8 | batches/B052.md |
