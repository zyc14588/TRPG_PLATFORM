# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S04",
  "purpose": "验证权限、Visibility Label Propagation、安全策略和导出/summary/RAG 隐私边界。",
  "inputs": {
    "sources": [
      {
        "id": "clue_secret",
        "visibility": "keeper_only"
      },
      {
        "id": "private_note",
        "visibility": "private_to_player:char_001"
      },
      {
        "id": "public_event",
        "visibility": "public"
      }
    ],
    "target_outputs": [
      "player_export",
      "party_summary",
      "rag_chunk",
      "debug_log"
    ]
  },
  "actions": [
    {
      "id": "export_player_report",
      "type": "export"
    },
    {
      "id": "generate_party_summary",
      "type": "summary"
    },
    {
      "id": "index_rag_chunk",
      "type": "rag_index"
    }
  ],
  "expected_events": [
    {
      "type": "VisibilityRedactionApplied",
      "fields": [
        "source_visibility",
        "target_visibility",
        "redaction_reason"
      ]
    }
  ],
  "expected_records": [
    {
      "record": "RedactionDecision",
      "required_fields": [
        "object_id",
        "policy",
        "actor",
        "result_visibility"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "keeper_only_to_player_export",
      "error": "VISIBILITY_DOWNGRADE_FORBIDDEN"
    },
    {
      "case": "private_to_player_to_party_summary",
      "error": "VISIBILITY_SCOPE_VIOLATION"
    },
    {
      "case": "ai_internal_to_export",
      "error": "AI_INTERNAL_EXPORT_FORBIDDEN"
    }
  ],
  "failure_cases": [
    {
      "id": "summary_leaks_keeper_secret",
      "expected_error": "VISIBILITY_LEAKAGE_DETECTED"
    }
  ],
  "required_evidence": [
    "evidence/stages/S04/visibility-redaction-tests.txt",
    "evidence/stages/S04/permission-policy-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-security visibility permission export_redaction --all-features",
  "pass_criteria": [
    "derived_visibility_never_exceeds_sources",
    "private_scope_enforced",
    "exports_redacted"
  ]
}
```
