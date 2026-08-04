# 03 — 工程文件目录规划

## 1. 目录树

```text

.
├── AGENTS.md
├── Cargo.toml
├── rust-toolchain.toml
├── deny.toml
├── docker-compose.yml
├── docker-compose.dev.yml
├── docker-compose.ci.yml
├── .github/
│   └── workflows/
│       ├── ci.yml
│       ├── contracts.yml
│       ├── golden-scenarios.yml
│       ├── docker-compose-smoke.yml
│       └── release.yml
├── apps/
│   ├── api-server/
│   │   └── src/main.rs
│   ├── realtime-server/
│   │   └── src/main.rs
│   ├── agent-worker/
│   │   └── src/main.rs
│   ├── admin-console/
│   └── web/
├── crates/
│   ├── trpg-shared-kernel/
│   ├── trpg-domain-core/
│   ├── trpg-data-eventing/
│   ├── trpg-security-governance/
│   ├── trpg-ruleset-coc7/
│   ├── trpg-runtime/
│   ├── trpg-agent-runtime/
│   ├── trpg-api/
│   ├── trpg-platform/
│   ├── trpg-ops/
│   ├── trpg-testing/
│   └── trpg-extension-sdk/
├── migrations/
├── schemas/
│   ├── events/
│   ├── api/
│   ├── websocket/
│   ├── agent/
│   ├── scenario/
│   └── export/
├── rulesets/
│   └── coc7/
│       ├── character_schema.json
│       ├── dice_rules.json
│       ├── skill_check_rules.json
│       ├── sanity_rules.json
│       ├── combat_rules.json
│       ├── chase_rules.json
│       ├── status_effects.json
│       ├── rag_index_config.json
│       ├── terminology.json
│       ├── ui_schema.json
│       ├── keeper_prompts/
│       └── character_creation_prompts/
├── agent-packs/
│   └── coc7/
├── fixtures/
│   ├── tutorial/
│   ├── golden/
│   ├── visibility/
│   ├── provider/
│   └── export/
├── policy/
│   ├── openfga/
│   └── opa/
├── infra/
│   ├── nginx/
│   ├── minio/
│   ├── postgres/
│   └── observability/
├── ops/
│   ├── runbooks/
│   ├── backup/
│   ├── restore/
│   ├── migration/
│   └── release/
├── scripts/
│   ├── validate_codex_prompt_inventory.py
│   ├── validate_markdown_links.py
│   ├── ci/
│   ├── backup_restore/
│   └── projection_rebuild/
├── docs/
│   ├── codex/
│   ├── architecture/
│   ├── adr/
│   ├── domain/
│   ├── api/
│   ├── security/
│   ├── testing/
│   ├── operations/
│   ├── ui/
│   └── reports/
└── tests/
    ├── contract/
    ├── integration/
    ├── e2e/
    ├── golden/
    └── leakage/
```

## 2. crate 职责边界

| crate | 阶段 | 职责 | 允许依赖 | 禁止事项 |
|---|---|---|---|---|
| `trpg-shared-kernel` | S01 | 基础类型、错误、配置、ID、时间、Actor、CommandEnvelope、Visibility 基础 | 无业务 crate | 不依赖 domain/runtime/api/agent。 |
| `trpg-domain-core` | S02 | Campaign、AuthorityContract、DecisionRecord、GameEvent、Fork、Character、MemoryFact、Visibility/Provenance | shared-kernel | 不直接访问数据库、不调用 LLM。 |
| `trpg-data-eventing` | S03 | SQLx、Event Store、Outbox、Projection、NATS、Redis、pgvector、RAG Snapshot | shared-kernel、domain-core | NATS/Redis/Projection 不得替代正史。 |
| `trpg-security-governance` | S04 | OpenFGA/OPA、Policy Gate、Audit、权限矩阵、隐私/版权/删除 | shared-kernel、domain-core、data-eventing | 平台管理权不得覆盖游戏裁定。 |
| `trpg-ruleset-coc7` | S05 | COC7 角色卡、骰子、SAN、战斗、追逐、Scenario Validator | shared-kernel、domain-core | COC 专有字段不污染 ruleset-agnostic core。 |
| `trpg-runtime` | S06 | Workflow、Session、PendingDecision、Decision Commit Pipeline、Saga、Scheduler | shared、domain、data、security、ruleset | 不绕过 Event Store/Policy Gate。 |
| `trpg-agent-runtime` | S07 | Agent Gateway/Runtime、Tool Gate、Provider、Memory/RAG、Model Certification | shared、domain、data、security、runtime | 业务层禁止直接 LLM；Agent 禁止直接写库。 |
| `trpg-api` | S08 | Axum REST、OpenAPI、WebSocket、idempotency、contract DTO | runtime、agent、security、domain | handler 不丢 actor/provenance/visibility。 |
| `trpg-platform` | S09 | 部署、object storage、observability、background workers、admin health | api、agent、data、security | prod 不接受占位 key 或未鉴权本地 provider 暴露。 |
| `trpg-ops` | S10 | backup/restore、upgrade/rollback、projection rebuild、incident runbook | data、platform | runbook 不得修改正史。 |
| `trpg-testing` | S11 | Golden/Tutorial、contract、leakage、model certification、bench | workspace | 不弱化测试来通过 CI。 |
| `trpg-extension-sdk` | S12 | ruleset/agent/tool/plugin SDK 与兼容矩阵 | shared、domain、agent | 插件不得绕过 Tool Grant/Policy/Event Store。 |

## 3. Apps 与服务映射

| app | 对应服务 | 依赖 crate | 验收重点 |
|---|---|---|---|
| `apps/api-server` | REST / Admin API | `trpg-api` | OpenAPI、idempotency、authz、health。 |
| `apps/realtime-server` | WebSocket 房间同步 | `trpg-api`, `trpg-runtime` | 断线重连、多 active_scene、visibility delta。 |
| `apps/agent-worker` | Agent Run / Memory / Export / Evaluation | `trpg-agent-runtime`, `trpg-runtime` | Tool Gate、provider、budget、audit。 |
| `apps/admin-console` | 管理与初始化 | `trpg-platform`, `trpg-api` | 模型配置、健康检查、备份恢复、日志。 |
| `apps/web` | Player / KP / Developer UI | API/WS contracts | 玩家低复杂度，KP/Developer 展示控制与审计。 |

## 4. 文件布局规则

- Rust 默认 flat module：`crates/<crate>/src/<module_tail>.rs`。
- 同一任务不得同时创建 flat 文件与目录式 `mod.rs`。
- 需要子模块时必须删除对应 flat 文件、更新 `lib.rs`、测试、文档追踪和 review 说明。
- concrete Rust 输出路径只由 primary prompt 产生；supplemental prompt 只生成 `docs/codex/90-traceability/supplemental-requirements/<Prompt ID>.md`。

## 5. Schema / Migration / Contract 目录

| 路径 | 内容 | 所属阶段 |
|---|---|---|
| `migrations/` | SQLx migrations；Event Store、outbox、projection、authz/audit tables。 | S03/S04 |
| `schemas/events/` | GameEvent、DecisionRecord、DiceRoll、CharacterSheetVersion 等 JSON Schema。 | S02/S03 |
| `schemas/api/` | OpenAPI DTO schema。 | S08 |
| `schemas/websocket/` | WS client/server messages 与 visibility delta schema。 | S08 |
| `schemas/agent/` | Agent output protocol、tool call schema、provider capability profile。 | S07 |
| `schemas/scenario/` | scenario.yaml/json schema。 | S05 |
| `fixtures/` | Tutorial/Golden/visibility/provider/export 测试数据。 | S05/S11 |
