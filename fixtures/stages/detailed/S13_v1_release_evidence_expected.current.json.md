# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S13",
  "purpose": "验证 V1 17 项验收证据、release candidate、回滚计划和最终审计报告闭合。",
  "inputs": {
    "v1_acceptance_items": 17,
    "required_reports": [
      "V1_ACCEPTANCE_REPORT.md",
      "V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md",
      "RELEASE_CANDIDATE_REPORT.md"
    ],
    "release_candidate": "rc-v1"
  },
  "actions": [
    {
      "id": "fill_v1_matrix",
      "type": "evidence_matrix"
    },
    {
      "id": "run_release_suite",
      "type": "release_validation"
    },
    {
      "id": "verify_rollback",
      "type": "rollback_validation"
    }
  ],
  "expected_events": [
    {
      "type": "V1AcceptanceMatrixCompleted",
      "items": 17
    },
    {
      "type": "ReleaseCandidateValidated"
    },
    {
      "type": "RollbackPlanVerified"
    }
  ],
  "expected_records": [
    {
      "record": "V1AcceptanceEvidenceRow",
      "required_fields": [
        "item",
        "status",
        "command",
        "evidence_path",
        "owner"
      ]
    },
    {
      "record": "ReleaseCandidateReport",
      "required_fields": [
        "commit",
        "artifacts",
        "risks",
        "rollback"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "missing_v1_evidence",
      "error": "V1_ACCEPTANCE_EVIDENCE_MISSING"
    },
    {
      "case": "tag_before_pass",
      "error": "RELEASE_TAG_FORBIDDEN_BEFORE_PASS"
    }
  ],
  "failure_cases": [
    {
      "id": "silent_fallback_unverified",
      "expected_error": "V1_NO_SILENT_FALLBACK_UNVERIFIED"
    }
  ],
  "required_evidence": [
    "evidence/stages/S13/v1-matrix.txt",
    "evidence/release/RELEASE_CANDIDATE_REPORT.md",
    "evidence/release/ROLLBACK_PLAN.md"
  ],
  "automation_target": "pwsh ./scripts/release/verify-v1.ps1 && docker compose up -d && cargo test --workspace --all-features",
  "pass_criteria": [
    "all_17_v1_items_have_evidence",
    "release_suite_passes",
    "rollback_plan_verified",
    "no_tag_before_approval"
  ]
}
```
