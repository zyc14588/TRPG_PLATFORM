# S01 ACCEPTANCE_PROMPT — Rust Workspace 与 shared kernel 基座

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


你是 Codex 验收代理。请验收 `S01 — Rust Workspace 与 shared kernel 基座` 的实现。

## 验收输入

- 本阶段 README 与 START_PROMPT。
- Codex 变更 diff。
- 测试命令输出。
- 更新的 schema/migration/event/API/WS/NATS 文档。
- 关联 Prompt ID 与 batch 列表。

## 必查项

- [ ] 所有公开类型使用领域专名，不出现 ModuleService/ModuleCommand 模板残留
- [ ] CommandEnvelope 携带 idempotency_key、expected_version、actor、correlation_id、causation_id
- [ ] shared-kernel 不依赖 domain/runtime/agent/api
- [ ] serde_json::Value 只出现在 schema boundary 说明允许的位置

## 通用红线

- 不得让 AI、插件、provider、handler 绕过 Authority / Event Store / Visibility / Fact Provenance / Policy Gate。
- 不得直接调用裸 LLM。
- 不得用删除测试、弱化 policy、关闭 redaction 的方式通过测试。
- 不得让 supplemental prompt 创建 Rust src/test 输出。

## 输出格式

```text
阶段：S01
结论：PASS / FAIL
证据：
- 变更文件：...
- 测试命令：...
- Prompt 覆盖：...
Findings：
- P0：...
- P1：...
- P2：...
最小修复建议：...
```
