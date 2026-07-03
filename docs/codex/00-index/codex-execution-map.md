> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/00-index/codex-execution-map.md`
> 筛选状态：`active-index`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Codex Execution Map — 施工执行图

## 推荐阶段

1. `01-foundation`：workspace、shared kernel、错误模型、配置模型。
2. `06-data-eventing`：Event Store、SQLx migrations、outbox、projection worker。
3. `02-domain-core`：authority、command、visibility、fact provenance、domain entities。
4. `03-runtime-orchestration`：workflow、pending decision、saga、scheduler、session runtime。
5. `07-api-realtime-contracts`：Axum/OpenAPI/WebSocket/NATS contract。
6. `04-ai-agent-system`：agent runtime、tool protocol、model provider、RAG snapshot。
7. `05-ruleset-coc7`：CoC7 dice、sanity、combat、chase、investigation。
8. `09-security-governance`：OpenFGA、OPA、audit、retention、visibility enforcement。
9. `10-testing-quality`：contract/golden/replay/model certification/benchmark。
10. `08-platform-infrastructure` 与 `11-ops-migration`：部署、观测、备份、发布、事故响应。
11. `12-extension-sdk`：plugin/ruleset/agent/tool provider SDK。
12. `00-index`、`90-traceability`、`99-appendix`：文档、映射、模板与追踪持续更新。

## 模块执行入口

| 目录 | 范围 | 默认 crate | Per-file prompts | 编码入口 | 测试入口 |
| --- | --- | --- | --- | --- | --- |
| 00-index | 项目索引与施工总控 | trpg-docs-governance | 48 | docs/codex/00-index/codex-module-code-prompt.md | docs/codex/00-index/codex-module-test-prompt.md |
| 01-foundation | Rust workspace / shared kernel / configuration | trpg-shared-kernel | 98 | docs/codex/01-foundation/codex-module-code-prompt.md | docs/codex/01-foundation/codex-module-test-prompt.md |
| 02-domain-core | Domain core / command / authority / visibility | trpg-domain-core | 106 | docs/codex/02-domain-core/codex-module-code-prompt.md | docs/codex/02-domain-core/codex-module-test-prompt.md |
| 03-runtime-orchestration | Runtime orchestration / workflow / saga / scheduler | trpg-runtime | 115 | docs/codex/03-runtime-orchestration/codex-module-code-prompt.md | docs/codex/03-runtime-orchestration/codex-module-test-prompt.md |
| 04-ai-agent-system | AI agent runtime / model provider / tool protocol / RAG | trpg-agent-runtime | 95 | docs/codex/04-ai-agent-system/codex-module-code-prompt.md | docs/codex/04-ai-agent-system/codex-module-test-prompt.md |
| 05-ruleset-coc7 | CoC7 ruleset / dice / combat / sanity / chase | trpg-ruleset-coc7 | 65 | docs/codex/05-ruleset-coc7/codex-module-code-prompt.md | docs/codex/05-ruleset-coc7/codex-module-test-prompt.md |
| 06-data-eventing | Event Store / SQLx / Outbox / NATS / Projection | trpg-data-eventing | 107 | docs/codex/06-data-eventing/codex-module-code-prompt.md | docs/codex/06-data-eventing/codex-module-test-prompt.md |
| 07-api-realtime-contracts | Axum API / OpenAPI / WebSocket / realtime contracts | trpg-api | 48 | docs/codex/07-api-realtime-contracts/codex-module-code-prompt.md | docs/codex/07-api-realtime-contracts/codex-module-test-prompt.md |
| 08-platform-infrastructure | Deployment / object storage / observability / workers | trpg-platform | 77 | docs/codex/08-platform-infrastructure/codex-module-code-prompt.md | docs/codex/08-platform-infrastructure/codex-module-test-prompt.md |
| 09-security-governance | OpenFGA / OPA / audit / privacy / visibility enforcement | trpg-security-governance | 54 | docs/codex/09-security-governance/codex-module-code-prompt.md | docs/codex/09-security-governance/codex-module-test-prompt.md |
| 10-testing-quality | Golden tests / contract tests / replay / model certification | trpg-testing | 78 | docs/codex/10-testing-quality/codex-module-code-prompt.md | docs/codex/10-testing-quality/codex-module-test-prompt.md |
| 11-ops-migration | Backup / restore / migration / incident response / release | trpg-ops | 43 | docs/codex/11-ops-migration/codex-module-code-prompt.md | docs/codex/11-ops-migration/codex-module-test-prompt.md |
| 12-extension-sdk | Plugin SDK / ruleset pack / agent pack / tool provider | trpg-extension-sdk | 32 | docs/codex/12-extension-sdk/codex-module-code-prompt.md | docs/codex/12-extension-sdk/codex-module-test-prompt.md |
| 90-traceability | Traceability / source audit / requirement-to-test mapping | trpg-docs-governance | 110 | docs/codex/90-traceability/codex-module-code-prompt.md | docs/codex/90-traceability/codex-module-test-prompt.md |
| 99-appendix | Appendix / templates / glossary / research notes | trpg-docs-governance | 33 | docs/codex/99-appendix/codex-module-code-prompt.md | docs/codex/99-appendix/codex-module-test-prompt.md |
