# S11 TEST_DATA — Testing Quality：Tutorial/Golden Scenario、Contract、Leakage、Model Certification CI

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


本阶段使用以下根目录测试数据文件：

- `test-data/tutorial_scenario_yaml.md`
- `test-data/golden_scenario_yaml.md`
- `test-data/export_expected_snapshots.md`
- `test-data/provider_model_certification_cases.md`

## 最小 smoke case

```json
{
  "stage": "S11",
  "name": "Testing Quality：Tutorial/Golden Scenario、Contract、Leakage、Model Certification CI",
  "authority_modes": ["HUMAN_KP", "AI_KP"],
  "visibility_labels": ["public", "party_visible", "private_to_player", "keeper_only", "ai_internal", "system_only"],
  "expected_invariants": [
    "authority_contract_locked",
    "event_store_is_canon",
    "policy_gate_default_deny",
    "no_direct_llm_call",
    "no_private_leakage"
  ]
}
```


## v2.21 当前阶段扩展 fixture

本阶段至少读取：

```text
fixtures/stages/S11_stage_acceptance_fixture.v1.json.md
```

再按 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md` 读取对应领域 fixture，并把自动化测试中的输入动作、预期事件、预期错误码、预期 visibility redaction、预期 projection hash、预期 export diff 写入仓库真实测试目录。


## v2.21 detailed expected-record fixture

Use `fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md` as the detailed machine-readable expected-record fixture for this stage. The fixture must be converted into automated assertions before the stage can pass strict acceptance.
