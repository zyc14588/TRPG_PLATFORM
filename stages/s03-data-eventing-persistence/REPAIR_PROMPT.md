# S03 REPAIR_PROMPT — Data/Eventing：PostgreSQL、Event Store、Outbox、Projection、RAG Snapshot

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


你是 Codex 修复代理。当前阶段 `S03 — Data/Eventing：PostgreSQL、Event Store、Outbox、Projection、RAG Snapshot` 出现失败。

## 输入

- 失败命令与日志。
- 最近变更文件。
- 相关 Prompt ID。
- 阶段 README / TEST_PLAN / ACCEPTANCE_PROMPT。

## 修复规则

1. 先定位失败类别：编译、格式、clippy、unit、integration、migration、contract、leakage、golden、compose、provider、policy。
2. 只做最小修复。
3. 不删除测试、不弱化 policy、不关闭 visibility redaction、不绕过 Event Store、不让 AI 直接写库、不引入直接 LLM 调用。
4. 修复后重跑失败命令与上游相关命令。
5. 输出修复文件、测试结果、剩余风险。
