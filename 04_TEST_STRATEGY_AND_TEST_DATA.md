# 04 — 测试方案与测试数据总览

## 1. 测试分层

| 层级 | 目标 | 必测对象 | 失败处理 |
|---|---|---|---|
| Unit | 领域规则与纯函数 | Authority guard、Visibility lattice、COC dice/SAN、error mapping | Codex 最小修复，不删除测试。 |
| Integration | 数据库、事务、外部组件 | SQLx migration、Event Store、outbox、OpenFGA/OPA、Redis/NATS/pgvector | 保留失败日志与 fixture。 |
| Contract | API/Event/WS/NATS schema | OpenAPI、JSON Schema、WebSocket delta、NATS subject | schema 与实现同步。 |
| Golden Replay | 固定输入固定状态 | Tutorial、Golden Scenario、Projection replay、Export snapshots | 生成 diff 并修复实现。 |
| Leakage / Safety | 隐私与越权负例 | keeper_only/private/ai_internal、prompt injection、tool gate | 不得通过降级安全门禁解决。 |
| Model Certification | 本地模型等级认证 | JSON/tool-call stability、visibility、rules mini-eval、latency | 未达 Level 4 不可 AI_KP。 |
| E2E / Deployment | V1 可玩闭环 | docker compose、初始化、模型、campaign、导出、备份恢复 | 记录操作证据。 |

## 2. 全局测试命令目录

```bash

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
sqlx migrate run
cargo test --test event_store_contract
cargo test --test projection_replay
cargo test --test visibility_leakage
cargo test --test openapi_contract
cargo test --test websocket_contract
cargo test --test nats_subject_contract
cargo test --test golden_scenarios_ci
cargo test --test model_certification_tests
docker compose up -d --build
```

## 3. 必备测试数据文件

| 文件 | 用途 |
|---|---|
| `test-data/seed_users_campaigns.md` | Server Owner、Campaign Owner、Human KP、AI KP、玩家、旁观者、管理员、Campaign seed。 |
| `test-data/authority_contract_cases.md` | HUMAN_KP / AI_KP 锁定、不可变、fork、model route snapshot。 |
| `test-data/visibility_leakage_cases.md` | public/party/private/keeper_only/ai_internal/system_only 继承与泄露负例。 |
| `test-data/tutorial_scenario_yaml.md` | 原创教学模组：车卡、调查、检定、SAN、线索、结局。 |
| `test-data/golden_scenario_yaml.md` | 原创验收模组：Agent、Visibility、复议、分组、暗骰、导出。 |
| `test-data/dice_san_combat_chase_cases.md` | COC7 骰子、成功等级、奖励/惩罚、SAN、基础战斗、追逐。 |
| `test-data/agent_tool_call_cases.md` | HUMAN_KP/AI_KP 模式差异、Tool Gate、Agent output protocol。 |
| `test-data/provider_model_certification_cases.md` | Ollama/llama.cpp/OpenAI-compatible local/cloud provider 与 certification。 |
| `test-data/api_ws_contract_samples.md` | REST、OpenAPI、WebSocket、NATS subject 示例。 |
| `test-data/export_expected_snapshots.md` | 玩家版、KP 版、审计版、单玩家私密版导出期望。 |
| `test-data/event_store_stream_cases.md` | Event Store append/replay/projection/outbox stream。 |
| `test-data/rag_snapshot_cases.md` | RAG chunk source/visibility/copyright/version/embedding snapshot。 |
| `test-data/fork_lineage_cases.md` | campaign fork、canon/non-canon/what-if/emergency-fork lineage。 |
| `test-data/permission_matrix_cases.md` | 角色权限与平台管理权/游戏裁定权分离。 |
| `test-data/change_control_cases.md` | Scope Control 与新增需求 gate。 |

## 4. 每阶段测试要求

每阶段必须至少满足：

1. 新增写命令：正向、幂等冲突、expected_version 冲突、权限拒绝、visibility 泄露负例。
2. 新增 API/Event/WS/NATS 契约：schema、contract test、trace map 同步。
3. 新增 Agent 工具：Tool Gate 允许/拒绝、mode difference、audit、budget、visibility context negative。
4. 新增 migration：forward/revert、空库/带数据升级、projection rebuild、backup restore。
5. 新增导出/摘要/RAG：玩家版/KP版/审计版 snapshot 和 keeper_only/private/ai_internal redaction。

## 5. 验收报告格式

每阶段最终生成：

```text
# SXX Acceptance Report

## 1. 关联输入
- Batch：...
- Prompt ID：...

## 2. 变更清单
- crate：...
- migration：...
- schema：...
- test：...

## 3. 测试命令与结果
- command：PASS/FAIL，日志路径。

## 4. 门禁结论
- Authority：PASS/FAIL
- Event Store：PASS/FAIL
- Visibility：PASS/FAIL
- Agent Governance：PASS/FAIL
- CI：PASS/FAIL

## 5. 风险与 TODO
- P0/P1/P2 分类。
```


## v2.21 detailed fixture completion

The strict fixture set now includes detailed expected-record fixtures for all stages, including S00, S01, S09, S10, and S12:

- `fixtures/stages/detailed/S00_governance_onboarding.current.json.md`
- `fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md`
- `fixtures/stages/detailed/S09_platform_infrastructure_deployment_expected.current.json.md`
- `fixtures/stages/detailed/S10_ops_migration_runbooks_expected.current.json.md`
- `fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md`

Each stage acceptance must convert its detailed fixture into executable assertions before PASS.
