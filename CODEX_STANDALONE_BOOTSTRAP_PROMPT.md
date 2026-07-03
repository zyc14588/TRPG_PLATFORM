
# Codex Standalone Bootstrap Prompt — v2.21 自包含施工启动提示词

你是 Codex。你正在接手一个 COC AI TRPG 工程实现任务。本包已经内置顶层设计、V6 strict governance 源材料、阶段施工计划、测试数据、CI/CD 提取说明、验收矩阵和 v2.21 normalized prompt 执行映射。不要依赖外部 zip、聊天上下文或未落库文件。

## 0. 首次读取顺序

```text
AGENTS.md
SOURCE_BUNDLE_INTEGRATION_GUIDE.md
docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md
docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
00_INPUT_ANALYSIS_AND_TRACEABILITY.md
01_OVERALL_CONSTRUCTION_PLAN.md
02_STAGE_CONFIRMATION_MATRIX.md
03_ENGINEERING_DIRECTORY_PLAN.md
04_TEST_STRATEGY_AND_TEST_DATA.md
05_CI_CD_CONFIGURATION.md
V1_ACCEPTANCE_EVIDENCE_MATRIX.md
PER_STAGE_FIXTURE_EXPANSION_PLAN.md
docs/codex/00-index/codex-persistent-context.md
docs/codex/00-index/codex-prompt-boundary.md
docs/codex/00-index/codex-batch-plan.md
```

然后从 `stages/s00-governance-onboarding/START_PROMPT.md` 开始。

## 1. 施工模型

```text
stage START_PROMPT
读取阶段 README / TEST_PLAN / TEST_DATA
读取 v2.21 normalized maps
读取关联 docs/codex 分类 README / AGENTS / module prompts
读取关联 execution batch
读取 batch 中引用的 per-file prompts
按 primary/supplemental/documentation 边界施工
运行阶段测试
写阶段证据
stage ACCEPTANCE_PROMPT
失败时运行 REPAIR_PROMPT
```

## 2. 当前产品目标

V1 必须完成 COC 7 在线跑团闭环：部署、管理员、模型配置、AI/HUMAN KP Campaign、Authority Contract 锁定、COC 车卡、原创 Tutorial、调查、检定、线索、SAN、NPC、基础战斗、基础追逐、多人同步、分组调查、导出、fork、Golden Scenario、隐私隔离、本地模型认证、无静默云 fallback、AI 裁定解释和审计记录。

## 3. 输出约束

- Rust-first Cargo workspace。
- 后端基线按 Codex 源材料执行：Axum、SQLx、PostgreSQL/pgvector、Redis、NATS JetStream、OpenFGA + OPA、OpenTelemetry、Docker Compose。
- 所有写命令必须携带 idempotency_key、expected_version、actor、correlation_id、causation_id。
- 正式状态只通过 Decision Commit Pipeline 写入 Event Store。
- projection、summary、RAG、export 不得成为正史来源。
- 任何业务层直连 LLM 都是 P0 阻断缺陷。
- 任何旧 V3/V4/V5/V6 token、源 SHA 或旧路径进入当前 module/output/migration/event/NATS/metric/test 名称，均为 P1 阻断缺陷。

## 4. 阶段输出证据

```text
docs/reports/stages/SXX_ACCEPTANCE_EVIDENCE.md
docs/reports/stages/SXX_TEST_RESULTS.md
docs/reports/stages/SXX_TRACEABILITY.md
```

S13 必须生成：

```text
docs/reports/V1_ACCEPTANCE_REPORT.md
docs/reports/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md
artifacts/test-reports/golden-scenarios/**
artifacts/test-reports/visibility-leakage/**
artifacts/test-reports/model-certification/**
artifacts/test-reports/docker-compose-smoke/**
```


## v2.21 strict repair note

本包保留全部提供文件的可追溯性：原始 V6 路径若因旧版本、旧 hash 或历史命名被规范化重命名，则其原始路径副本进入 `source-archive/v6-paths/**`，只用于审计与覆盖证明。Codex 当前执行只允许读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` 之后再进入 batch 或 per-file prompt。

## v2.21 中文手动启动提示词

```text
你是 Codex，正在接手 COC AI TRPG v2.21 strict 施工包。请先读取本文件列出的权威输入、顶层设计、normalized maps、阶段矩阵、V1 验收矩阵和严格校验文件。当前任务只允许建立施工上下文，不得直接编码。请输出阶段 readiness 报告，报告必须包含下一阶段、必读文件、execution batch、per-file prompt、测试数据、CI/CD 影响、证据路径、阻塞项和风险。任何缺失材料都必须标记为 FAIL 或 BLOCKED，不得推测 PASS。
```
