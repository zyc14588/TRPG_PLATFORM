# S09 — Platform Infrastructure：Docker Compose、Object Storage、Observability、Admin Health

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现一键部署、环境配置、对象存储、后台 worker、观测、性能预算、Admin health checks、dev/prod provider security boundary。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S09` |
| 相关分类 | `08-platform-infrastructure` |
| 相关 batch | BATCH-031-08-platform-infrastructure, BATCH-032-08-platform-infrastructure, BATCH-033-08-platform-infrastructure, BATCH-034-08-platform-infrastructure |
| Prompt 数 | 77 |
| Primary | 21 |
| Supplemental | 42 |
| Docs/Trace | 14 |
| 主要 crate | `trpg-platform` |

## 3. 启动条件

S03/S08 的服务可启动；S04/S07 安全与 provider 边界已有实现。

## 4. 主要输出

- `docker-compose.yml`
- `docker-compose.dev.yml`
- `infra/nginx/*`
- `crates/trpg-platform/src/*.rs`
- `apps/agent-worker/src/main.rs`
- `apps/admin-console/*`

## 5. 测试重点

- docker compose smoke
- 健康检查
- 对象存储上传/下载
- 日志/metrics/traces 脱敏
- provider base_url/key 生产安全检查
- 性能预算基准

## 6. 推荐命令

- `docker compose up -d --build`
- `cargo test -p trpg-platform --all-features`
- `curl -f http://localhost:8080/healthz`

## 7. 测试数据

- `test-data/provider_model_certification_cases.md`
- `test-data/seed_users_campaigns.md`

## 8. 阶段验收清单

- [ ] docker compose up -d 能启动 Web/API/Realtime/Agent/Postgres/pgvector/Redis/Object Storage/Reverse Proxy/Admin
- [ ] 生产环境拒绝占位 key 或暴露未鉴权本地模型
- [ ] 日志、metrics、traces 不泄露 keeper_only/private 内容
- [ ] Admin 初始化向导覆盖模型连接、规则包、数据库、WebSocket、RAG、骰子自检

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
