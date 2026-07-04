# Detailed Stage Acceptance Fixture - v2.21

```json
{
  "stage": "S06",
  "purpose": "Validate that agent adjudication requests can write formal state only through the Decision Commit Pipeline.",
  "inputs": {
    "authority_mode": "AI_KP",
    "agent_id": "coc_ai_keeper_orchestrator",
    "tool_request": {
      "tool": "request_skill_check",
      "args": {
        "skill": "Spot Hidden",
        "difficulty": "normal"
      }
    }
  },
  "actions": [
    {
      "id": "assemble_context",
      "type": "context_assembler"
    },
    {
      "id": "tool_gate",
      "type": "permission_gate"
    },
    {
      "id": "commit_decision",
      "type": "decision_commit_pipeline"
    }
  ],
  "expected_events": [
    {
      "type": "ToolRequestApproved",
      "tool": "request_skill_check"
    },
    {
      "type": "DecisionCommitted",
      "linked_records": [
        "DecisionRecord",
        "DiceRoll",
        "GameEvent"
      ]
    }
  ],
  "expected_records": [
    {
      "record": "AI Decision Reproducibility Record",
      "required_fields": [
        "agent_pack_version",
        "prompt_version",
        "model_provider",
        "context_hash",
        "tool_calls",
        "decision_summary"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "atmosphere_writer_state_mutation",
      "error": "AGENT_TOOL_NOT_ALLOWED"
    },
    {
      "case": "human_kp_ai_formal_commit",
      "error": "HUMAN_KP_AI_DRAFT_ONLY"
    }
  ],
  "failure_cases": [
    {
      "id": "direct_db_write_by_agent",
      "expected_error": "AGENT_DIRECT_STATE_WRITE_FORBIDDEN"
    }
  ],
  "required_evidence": [
    "evidence/stages/S06/decision-pipeline-tests.txt",
    "evidence/stages/S06/tool-gate-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-runtime --test batch_012_runtime_contract_tests --all-features",
  "pass_criteria": [
    "tool_gate_required",
    "decision_commit_evented",
    "draft_only_human_kp_enforced"
  ]
}
```
