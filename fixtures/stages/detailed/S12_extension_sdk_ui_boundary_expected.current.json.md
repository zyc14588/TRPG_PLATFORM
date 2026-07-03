# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S12",
  "purpose": "验证 Extension SDK、UI 分层、developer tools 与 player/KP/admin 可见边界。",
  "inputs": {
    "sdk_extension": "coc7_sample_extension",
    "ui_roles": [
      "player",
      "human_kp",
      "admin",
      "developer"
    ],
    "visibility_cases": [
      "player_hidden_keeper_note",
      "developer_debug_redacted"
    ]
  },
  "actions": [
    {
      "id": "sdk_contract",
      "type": "sdk_contract_test"
    },
    {
      "id": "ui_role_snapshot",
      "type": "ui_snapshot_test"
    },
    {
      "id": "developer_debug_view",
      "type": "debug_boundary_test"
    }
  ],
  "expected_events": [
    {
      "type": "ExtensionLoaded",
      "extension_id": "coc7_sample_extension"
    },
    {
      "type": "UiRoleBoundaryVerified"
    }
  ],
  "expected_records": [
    {
      "record": "SdkCompatibilityReport",
      "required_fields": [
        "extension_id",
        "ruleset_version",
        "tool_schema_version",
        "compatibility_result"
      ]
    },
    {
      "record": "UiSnapshotDiff",
      "required_fields": [
        "role",
        "snapshot_hash",
        "redacted_fields"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "extension_state_write_bypass",
      "error": "EXTENSION_STATE_WRITE_FORBIDDEN"
    },
    {
      "case": "player_sees_developer_debug",
      "error": "UI_ROLE_BOUNDARY_VIOLATION"
    }
  ],
  "failure_cases": [
    {
      "id": "sdk_can_call_llm_directly",
      "expected_error": "EXTENSION_DIRECT_LLM_FORBIDDEN"
    }
  ],
  "required_evidence": [
    "evidence/stages/S12/sdk-contract.txt",
    "evidence/stages/S12/ui-role-snapshots.txt",
    "evidence/stages/S12/developer-boundary.txt"
  ],
  "automation_target": "pnpm test -- sdk-boundary ui-role-snapshots && cargo test extension_sdk_boundary --all-features",
  "pass_criteria": [
    "sdk_cannot_bypass_state",
    "ui_roles_separated",
    "debug_data_redacted",
    "extension_compatibility_checked"
  ]
}
```
