# S09 ACCEPTANCE_PROMPT — Platform Infrastructure：Docker Compose、Object Storage、Observability、Admin Health

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


你是 Codex 验收代理。请验收 `S09 — Platform Infrastructure：Docker Compose、Object Storage、Observability、Admin Health` 的实现。

## 验收输入

- 本阶段 README 与 START_PROMPT。
- Codex 变更 diff。
- 测试命令输出。
- 更新的 schema/migration/event/API/WS/NATS 文档。
- 关联 Prompt ID 与 batch 列表。

## 必查项

- [ ] docker compose up -d 能启动 Web/API/Realtime/Agent/Postgres/pgvector/Redis/Object Storage/Reverse Proxy/Admin
- [ ] 生产环境拒绝占位 key 或暴露未鉴权本地模型
- [ ] 日志、metrics、traces 不泄露 keeper_only/private 内容
- [ ] Admin 初始化向导覆盖模型连接、规则包、数据库、WebSocket、RAG、骰子自检

## 通用红线

- 不得让 AI、插件、provider、handler 绕过 Authority / Event Store / Visibility / Fact Provenance / Policy Gate。
- 不得直接调用裸 LLM。
- 不得用删除测试、弱化 policy、关闭 redaction 的方式通过测试。
- 不得让 supplemental prompt 创建 Rust src/test 输出。

## 输出格式

```text
阶段：S09
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
