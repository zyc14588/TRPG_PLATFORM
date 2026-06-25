# Codex Master Prompt — 初始化完整 TRPG 平台

你正在一个空仓库或仅含设计文档的仓库中工作。必须先阅读：

1. `DECISIONS.md`
2. `AGENTS.md`
3. `docs/PRODUCT_SYSTEM_DESIGN.md`
4. `docs/BACKEND_ARCHITECTURE.md`
5. `docs/UI_UX_SPEC.md`
6. `docs/IMPLEMENTATION_HANDBOOK.md`
7. `config/default.toml`

这些决策已经最终确认，不要再次提问。采用保守、安全、可编译的实现。

## 第一轮目标

创建可运行的 monorepo 骨架：

```text
apps/web
crates/server
crates/worker
crates/auth
crates/game_core
crates/trpg_rules
crates/rule_system_generic_percentile
crates/rule_system_dnd5e_srd
crates/rule_system_commercial_adapter
crates/dice_engine
crates/rag_core
crates/document_ingestor
crates/llm_client
crates/media_provider
crates/agent_core
crates/dice_agent
crates/character_agent
crates/kp_agent
crates/creator_agent
crates/memory_core
crates/export_core
crates/storage
crates/observability
migrations
infra/compose
infra/helm/trpg-platform
prompts
schemas
```

## 必须实现

- 根 Cargo workspace 和 pnpm workspace。
- Axum `/healthz`、`/readyz`、`/metrics`。
- 统一 `AppConfig`，读取环境变量和 `config/default.toml`。
- SQLx migrations：users、rooms、room_members、sessions、session_events、snapshots、characters、combat_states、documents、chunks、agent_runs、outbox、generated_media、audit_logs。
- AuthContext、RoomRole、VisibilityScope、RoomPrivacyMode。
- WebSocket envelope、`server_seq`、resume contract 的强类型结构。
- LlmProvider、EmbeddingProvider、ImageProvider 和 Mock 实现。
- RuleSystem、GridGeometry、VectorStore、KeywordIndex trait。
- Optimistic locking repository，要求 expected_version；死锁/serialization 分类器与有界重试策略。
- Docker Compose：api、worker、postgres+pgvector、redis、minio、prometheus、grafana、caddy。
- Next.js 基础 shell 与 mock 页面，不必首轮完成视觉特效。
- OpenAPI、JSON Schema 和测试骨架。

## 特别约束

- COC/商业系统只实现适配器和合法规则包入口，不捆绑任何未授权正文。
- 地图合同同时支持 scene board、square、hex flat、hex pointy。
- Creator 图片默认关闭，Mock ImageProvider 必须可测试；输出先进入 draft。
- 战斗、回合、角色卡写入使用 expected_version + idempotency_key。
- 捕获 PostgreSQL 40P01/40001/55P03；最多三次重试；重试耗尽返回 409。
- CRDT 只保留接口，不用于权威状态。
- PL 路由和 WS 投影不得包含 KP-only 字段。
- 不调用真实模型作为 CI 前提。

## 完成标准

运行并修复：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check
pnpm lint
pnpm typecheck
pnpm test
```

最后输出：修改文件、架构取舍、运行命令、测试结果、仍为 mock 的部分。不要承诺后台工作。
