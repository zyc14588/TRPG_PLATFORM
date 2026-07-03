# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S11",
  "purpose": "验证 Tutorial/Golden Scenario、visibility leakage、export diff、model eval 和 CI gate。",
  "inputs": {
    "scenario": "golden_scenario_001",
    "players": [
      "p1",
      "p2"
    ],
    "hidden_clues": [
      "clue_keeper_001"
    ],
    "exports": [
      "player",
      "keeper",
      "audit"
    ]
  },
  "actions": [
    {
      "id": "run_golden",
      "type": "scenario_test"
    },
    {
      "id": "visibility_leakage",
      "type": "privacy_test"
    },
    {
      "id": "export_diff",
      "type": "snapshot_diff"
    },
    {
      "id": "model_eval",
      "type": "local_model_cert_eval"
    }
  ],
  "expected_events": [
    {
      "type": "GoldenScenarioCompleted"
    },
    {
      "type": "VisibilityLeakageTestPassed"
    },
    {
      "type": "ExportSnapshotCompared"
    }
  ],
  "expected_records": [
    {
      "record": "ScenarioTestReport",
      "required_fields": [
        "steps",
        "dice",
        "decisions",
        "final_state_hash"
      ]
    },
    {
      "record": "ExportDiffReport",
      "required_fields": [
        "player_export_hash",
        "keeper_export_hash",
        "audit_export_hash",
        "redacted_fields"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "hidden_clue_in_player_export",
      "error": "VISIBILITY_LEAKAGE_DETECTED"
    },
    {
      "case": "golden_roll_ignored",
      "error": "GOLDEN_SCENARIO_RULE_VIOLATION"
    }
  ],
  "failure_cases": [
    {
      "id": "ai_reveals_keeper_secret",
      "expected_error": "KEEPER_SECRET_REVEALED"
    }
  ],
  "required_evidence": [
    "evidence/stages/S11/golden-scenario.txt",
    "evidence/stages/S11/visibility-leakage.txt",
    "evidence/stages/S11/export-diff.txt"
  ],
  "automation_target": "cargo test golden_scenarios visibility_leakage export_diff --all-features",
  "pass_criteria": [
    "golden_scenario_passes",
    "no_visibility_leakage",
    "exports_diff_as_expected",
    "eval_report_written"
  ]
}
```
