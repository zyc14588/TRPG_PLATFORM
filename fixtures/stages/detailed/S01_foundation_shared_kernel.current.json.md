# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S01",
  "purpose": "验证 shared kernel 的 ID、版本、错误码、visibility label 和 timestamp 基础类型可支撑后续领域模型。",
  "inputs": {
    "sample_ids": [
      "campaign_001",
      "session_001",
      "character_001"
    ],
    "visibility_labels": [
      "public",
      "keeper_only",
      "private_to_player",
      "system_only"
    ]
  },
  "actions": [
    {
      "id": "parse_ids",
      "type": "unit_test",
      "target": "shared_kernel::ids"
    },
    {
      "id": "compose_error",
      "type": "unit_test",
      "target": "shared_kernel::errors"
    }
  ],
  "expected_events": [
    {
      "type": "SharedKernelTypesValidated",
      "visibility": "system_only"
    }
  ],
  "expected_records": [
    {
      "record": "KernelContractSnapshot",
      "required_fields": [
        "id_format",
        "version_policy",
        "visibility_enum",
        "error_codes"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "invalid_visibility_label",
      "error": "UNKNOWN_VISIBILITY_LABEL"
    },
    {
      "case": "empty_id",
      "error": "INVALID_ENTITY_ID"
    }
  ],
  "failure_cases": [
    {
      "id": "stringly_typed_ids",
      "expected_error": "KERNEL_TYPE_SAFETY_REGRESSION"
    }
  ],
  "required_evidence": [
    "evidence/stages/S01/kernel-contract-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-shared-kernel --all-features",
  "pass_criteria": [
    "typed_ids_enforced",
    "visibility_enum_closed",
    "errors_are_stable"
  ]
}
```
