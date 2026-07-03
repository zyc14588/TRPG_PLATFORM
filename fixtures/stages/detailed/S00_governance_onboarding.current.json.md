# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S00",
  "purpose": "验证治理入口、normalized overlay、目录边界和证据目录初始化是否可被自动化检查。",
  "inputs": {
    "required_files": [
      "AGENTS.md",
      "CODEX_STANDALONE_BOOTSTRAP_PROMPT.md",
      "DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md",
      "STRICT_LINK_AND_REFERENCE_VALIDATION.md"
    ],
    "repository_state": "fresh_checkout"
  },
  "actions": [
    {
      "id": "read_authority_files",
      "type": "read_files",
      "files": [
        "AGENTS.md",
        "CODEX_STANDALONE_BOOTSTRAP_PROMPT.md"
      ]
    },
    {
      "id": "create_evidence_root",
      "type": "filesystem",
      "path": "evidence/stages/S00/"
    }
  ],
  "expected_events": [
    {
      "type": "GovernanceReadinessRecorded",
      "visibility": "system_only",
      "must_include": [
        "authority_order",
        "normalized_overlay",
        "provenance_boundary"
      ]
    }
  ],
  "expected_records": [
    {
      "record": "StageReadinessReport",
      "path": "evidence/stages/S00/readiness.md",
      "required_fields": [
        "scope",
        "inputs",
        "blocked_items",
        "next_stage"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "missing_authority_file",
      "error": "GOVERNANCE_INPUT_MISSING"
    }
  ],
  "failure_cases": [
    {
      "id": "uses_source_materials_as_current",
      "expected_error": "PROVENANCE_USED_AS_AUTHORITY"
    }
  ],
  "required_evidence": [
    "evidence/stages/S00/readiness.md",
    "evidence/stages/S00/directory-boundary-check.txt"
  ],
  "automation_target": "cargo test governance_onboarding_contract || pwsh ./scripts/verify-governance-boundary.ps1",
  "pass_criteria": [
    "all_required_files_read",
    "source_materials_not_current",
    "evidence_root_created"
  ]
}
```
