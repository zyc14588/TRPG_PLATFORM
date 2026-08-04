# V1 Acceptance Evidence Matrix — 17 条严格验收证据矩阵

| # | V1 完成标准 | 阶段 | 测试命令 / CI Gate | Fixture | 证据文件 | 通过条件 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Docker Compose 一键部署成功 | S09,S13 | bash scripts/ci/production-security-smoke.sh | fixtures/ops/backup_restore_projection_rebuild.v1.json.md | artifacts/test-reports/docker-compose-smoke/compose-health.md | 所有核心服务 healthy；初始化向导 smoke 通过 |
| 2 | 可配置云端模型、本地 Ollama、本地 llama.cpp | S07,S09,S13 | cargo test -p trpg-testing --test model_certification_tests | fixtures/provider/model_certification_matrix.v1.json.md | artifacts/test-reports/model-certification/provider-matrix.md | 三类 provider route 可配置；prod boundary 阻断占位/暴露配置 |
| 3 | 可创建 AI KP / 真人 KP Campaign | S02,S06,S08,S13 | cargo test -p trpg-domain-core --test authority_contract_contract_tests && cargo test -p trpg-api --test campaign_character_api_integration | fixtures/authority/authority_contract_cases.v1.json.md | docs/reports/stages/S02_ACCEPTANCE_EVIDENCE.md | 两种模式创建成功且 authority_mode 互斥 |
| 4 | Authority Contract 不可修改 | S02,S04,S13 | cargo test -p trpg-domain-core --test authority_contract_contract_tests authority_contract_rejects_in_place_mode_or_owner_change | fixtures/authority/authority_contract_cases.v1.json.md | artifacts/test-reports/authority/immutability.md | PATCH/override 返回 AuthorityContractImmutable 或 AuthorityViolation |
| 5 | 可完成 COC 车卡与角色审核 | S05,S06,S08,S13 | cargo test -p trpg-api --test campaign_character_api_integration | fixtures/rules/coc7_character_creation_review.v1.json.md | artifacts/test-reports/coc7/character-sheet.md | 角色派生属性、技能点、审核、初始版本锁定通过 |
| 6 | 可运行一个完整原创教学模组 | S05,S06,S07,S11,S13 | cargo test -p trpg-testing --test tutorial_complete_e2e | fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md | artifacts/test-reports/tutorial/tutorial-run.md | 开场到结局事件链完整，无版权依赖 |
| 7 | 可完成调查、检定、线索、SAN、NPC、基础战斗、基础追逐 | S05,S06,S11,S13 | cargo test -p trpg-ruleset-coc7 --all-features | fixtures/rules/coc7_dice_matrix.v1.json.md; fixtures/rules/coc7_san_combat_chase_flow.v1.json.md | artifacts/test-reports/coc7/rules-flow.md | 所有核心 COC7 flow 通过，核心线索 fail-forward |
| 8 | 可多人在线同步和分组调查 | S08,S11,S13 | cargo test -p trpg-api --test websocket_protocol_contract_tests && cargo test -p trpg-api --test realtime_room_sync_contract_tests && cargo test -p trpg-api --test realtime_sync_contract_tests | fixtures/api/api_ws_nats_contract_cases.v1.json.md | artifacts/test-reports/realtime/split-party.md | private scene delta 不跨组泄露，断线重连恢复 |
| 9 | 私密信息不泄露到摘要、RAG、导出、回放 | S04,S07,S11,S13 | cargo test -p trpg-testing --test visibility_leakage | fixtures/visibility/visibility_redaction_matrix.v1.json.md; fixtures/rag/rag_snapshot_cases.v1.json.md | artifacts/test-reports/visibility/leakage.md | keeper_only/private/ai_internal 全链路 redaction 通过 |
| 10 | AI KP 所有正式裁定都通过工具和事件日志 | S06,S07,S11,S13 | cargo test -p trpg-agent-runtime --test agent_job_contract_tests real_agent_job_worker_completes_an_npc_turn_through_all_three_providers && cargo test -p trpg-data-eventing --test event_store_contract | fixtures/agent/agent_tool_gate_cases.v1.json.md; fixtures/event_store/golden_event_stream_expected.v1.json.md | artifacts/test-reports/agent/decision-commit.md | AI 输出只能成为 tool request / decision，经 Event Store 落库 |
| 11 | 真人 KP 模式下 AI 只能生成草稿 | S06,S07,S11,S13 | cargo test -p trpg-agent-runtime --test agent_job_contract_tests human_kp_job_is_draft_only_until_a_separate_approval_event_exists | fixtures/agent/agent_tool_gate_cases.v1.json.md | artifacts/test-reports/agent/human-kp-draft-only.md | requires_human_confirmation=true；正式工具降级 draft |
| 12 | 可 fork Campaign | S02,S03,S06,S13 | cargo test -p trpg-domain-core --test fork_canon_lineage_contract_tests | fixtures/authority/fork_lineage_cases.v1.json.md | artifacts/test-reports/fork/lineage.md | 父 Campaign 不变，子 Campaign 生成新 Authority Contract |
| 13 | 可导出玩家版、KP 版、审计版战报 | S07,S10,S11,S13 | cargo test -p trpg-testing --test replay_consistency_tests_contract_tests | fixtures/export/export_snapshots_expected.v1.json.md | artifacts/test-reports/export/snapshots.md | 不同权限导出符合 must_contain/must_not_contain |
| 14 | Golden Scenario Tests 通过 | S11,S13 | bash scripts/ci/golden-product-flow.sh "$TRPG_GOLDEN_EVIDENCE_DIR" | fixtures/scenarios/golden_salt_bell.scenario.yaml.md; fixtures/actions/golden_salt_bell_action_sequence.v1.json.md | artifacts/test-reports/golden/golden-salt-bell.md | golden_scenarios_ci、Agent、Visibility、复议、分组、暗骰、导出全部通过 |
| 15 | 本地模型认证机制可运行，未认证模型不能担任 AI Keeper Orchestrator | S07,S09,S11,S13 | cargo test -p trpg-testing --test model_certification_tests | fixtures/provider/model_certification_matrix.v1.json.md | artifacts/test-reports/model-certification/cert-levels.md | Level 0-3 被阻断；Level 4 允许 AI KP |
| 16 | 不得静默从本地 fallback 到云端 | S07,S09,S13 | cargo test -p trpg-agent-runtime --test model_provider_local_cloud_impl_contract_tests model_provider_local_cloud_impl_blocks_silent_local_to_cloud_fallback | fixtures/provider/model_certification_matrix.v1.json.md | artifacts/test-reports/provider/no-silent-fallback.md | 无显式 consent/snapshot/audit 时跨边界 fallback DENY |
| 17 | 关键 AI 裁定有玩家可见解释和审计记录 | S06,S07,S11,S13 | cargo test -p trpg-domain-core --test decision_record_model_contract_tests && cargo test -p trpg-agent-runtime --test ai_agent_contract_tests | fixtures/agent/ai_decision_record_cases.v1.json.md | artifacts/test-reports/agent/decision-explanation.md | Public summary、keeper notes、audit record 三层存在且可见性正确 |

## 使用规则

本表只定义 17 项权威验收条目及其权威测试命令，不承载候选状态。候选矩阵必须由
`scripts/ci/acceptance_evidence_matrix.py` 在仓库外从机器可验证 evidence manifest
生成；tracked 的 `docs/reports/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md` 只能保留生成
说明，不得手工填入 commit、artifact hash 或 PASS/FAIL。

任何旧指南中的“填充 V1 矩阵”均解释为运行上述生成器产生仓库外候选矩阵；本规则
明确取代人工复制或维护 tracked PASS 的旧流程。任一 P0/P1 项缺少绑定当前 clean
HEAD/tree、实际权威命令、退出码、原始日志及 artifact hash 的证据即不得发布。


## v2.21 detailed fixture completion

The strict fixture set now includes detailed expected-record fixtures for all stages, including S00, S01, S09, S10, and S12:

- `fixtures/stages/detailed/S00_governance_onboarding.current.json.md`
- `fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md`
- `fixtures/stages/detailed/S09_platform_infrastructure_deployment_expected.current.json.md`
- `fixtures/stages/detailed/S10_ops_migration_runbooks_expected.current.json.md`
- `fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md`

Each stage acceptance must convert its detailed fixture into executable assertions before PASS.
