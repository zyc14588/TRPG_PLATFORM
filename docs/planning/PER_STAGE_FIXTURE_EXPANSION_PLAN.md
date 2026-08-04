# Per-stage Fixture Expansion Plan — 每阶段测试数据扩展方案

## 1. 目标

上版每阶段 `TEST_DATA.md` 偏规划级。本版新增 `fixtures/**`，把阶段验收转为可由 Codex 提取并落库的 fixture。所有 fixture 仍以 Markdown 包装，满足 Markdown-only 交付要求。

## 2. 阶段到 fixture 映射

| 阶段 | 必读 fixture | 主要验证 |
| --- | --- | --- |
| S00 | fixtures/stages/S00_stage_acceptance_fixture.v1.json.md | 自包含源材料、AGENTS、docs/codex、prompt 边界落库 |
| S01 | fixtures/stages/S01_stage_acceptance_fixture.v1.json.md | shared kernel、CommandEnvelope、idempotency、correlation |
| S02 | fixtures/authority/authority_contract_cases.v1.json.md; fixtures/authority/fork_lineage_cases.v1.json.md | Authority immutable、KP 互斥、fork |
| S03 | fixtures/event_store/golden_event_stream_expected.v1.json.md; fixtures/rag/rag_snapshot_cases.v1.json.md | Event Store 正史、expected_version、idempotency、RAG snapshot |
| S04 | fixtures/security/permission_matrix.v1.json.md; fixtures/visibility/visibility_redaction_matrix.v1.json.md | OpenFGA/OPA、权限、Visibility redaction |
| S05 | fixtures/rules/coc7_character_creation_review.v1.json.md; fixtures/rules/coc7_dice_matrix.v1.json.md; fixtures/rules/coc7_san_combat_chase_flow.v1.json.md | 车卡、骰子、检定、SAN、战斗、追逐 |
| S06 | fixtures/agent/ai_decision_record_cases.v1.json.md; fixtures/event_store/golden_event_stream_expected.v1.json.md | Decision Commit Pipeline、HUMAN_KP draft、AI_KP tool commit |
| S07 | fixtures/agent/agent_tool_gate_cases.v1.json.md; fixtures/provider/model_certification_matrix.v1.json.md; fixtures/rag/rag_snapshot_cases.v1.json.md | Agent Gateway、Tool Gate、本地模型认证、无静默 fallback |
| S08 | fixtures/api/api_ws_nats_contract_cases.v1.json.md | REST、WebSocket、NATS contract、前端不得直连模型 |
| S09 | fixtures/provider/model_certification_matrix.v1.json.md; fixtures/ops/backup_restore_projection_rebuild.v1.json.md | Docker Compose、Provider security boundary、健康检查 |
| S10 | fixtures/ops/backup_restore_projection_rebuild.v1.json.md | 备份恢复、Projection rebuild、rollback |
| S11 | fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md; fixtures/scenarios/golden_salt_bell.scenario.yaml.md; fixtures/actions/golden_salt_bell_action_sequence.v1.json.md | Tutorial/Golden Scenario、泄露负例、export snapshot |
| S12 | fixtures/security/permission_matrix.v1.json.md; fixtures/api/api_ws_nats_contract_cases.v1.json.md | SDK / UI boundary，不绕过 Tool Grant 与 Policy Gate |
| S13 | fixtures/ci/v1_acceptance_evidence_schema.v1.json.md; 全部 fixtures | 17 条 V1 验收证据矩阵 |


## 3. Codex 落库提示

```text
你是 Codex 测试 fixture 落库代理。
读取 fixtures/README.md 与本文件。
把每个 *.json.md / *.yaml.md 的 fenced 代码块提取为 tests/fixtures 下同名去除 .md 的文件。
不得改变 case_id、expected、visibility、authority_mode、error code。
提取后为每个 fixture 创建至少一个加载测试，验证 schema、case_id 唯一、expected 字段非空。
```


## v2.21 detailed fixture completion

The strict fixture set now includes detailed expected-record fixtures for all stages, including S00, S01, S09, S10, and S12:

- `fixtures/stages/detailed/S00_governance_onboarding.current.json.md`
- `fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md`
- `fixtures/stages/detailed/S09_platform_infrastructure_deployment_expected.current.json.md`
- `fixtures/stages/detailed/S10_ops_migration_runbooks_expected.current.json.md`
- `fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md`

Each stage acceptance must convert its detailed fixture into executable assertions before PASS.
