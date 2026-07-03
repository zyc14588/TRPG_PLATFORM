# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S02",
  "purpose": "验证 Authority Contract 不可变、Campaign fork-only 策略、DecisionRecord 和 confirmed fact 来源限制。",
  "inputs": {
    "authority_contract": {
      "authority_mode": "AI_KP",
      "locked": true,
      "change_policy": "FORK_ONLY"
    },
    "patch_attempt": {
      "authority_mode": "HUMAN_KP"
    },
    "confirmed_fact_candidate_sources": [
      "GameEvent",
      "DecisionRecord",
      "DiceRoll",
      "CharacterSheetVersion",
      "ClueRevealEvent",
      "AgentDraft",
      "NpcClaim"
    ]
  },
  "actions": [
    {
      "id": "attempt_authority_patch",
      "type": "domain_command",
      "command": "PatchAuthorityContract"
    },
    {
      "id": "fork_campaign",
      "type": "domain_command",
      "command": "ForkCampaign"
    },
    {
      "id": "promote_fact",
      "type": "domain_command",
      "command": "PromoteFactToConfirmed"
    }
  ],
  "expected_events": [
    {
      "type": "AuthorityMutationRejected",
      "error": "AUTHORITY_CONTRACT_IMMUTABLE"
    },
    {
      "type": "CampaignForked",
      "fields": [
        "parent_campaign_id",
        "new_authority_contract_id",
        "fork_reason"
      ]
    },
    {
      "type": "FactPromoted",
      "allowed_sources": [
        "GameEvent",
        "DecisionRecord",
        "DiceRoll",
        "CharacterSheetVersion",
        "ClueRevealEvent"
      ]
    }
  ],
  "expected_records": [
    {
      "record": "AuthorityContract",
      "locked": true,
      "change_policy": "FORK_ONLY"
    },
    {
      "record": "DecisionRecord",
      "required_fields": [
        "decided_by",
        "authority_mode",
        "rules_reference",
        "source_context_hash"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "patch_locked_authority",
      "error": "AUTHORITY_CONTRACT_IMMUTABLE"
    },
    {
      "case": "agent_draft_to_confirmed",
      "error": "INVALID_CONFIRMED_FACT_SOURCE"
    }
  ],
  "failure_cases": [
    {
      "id": "in_place_authority_change",
      "expected_error": "AUTHORITY_CONTRACT_IMMUTABLE"
    },
    {
      "id": "npc_claim_confirmed",
      "expected_error": "INVALID_CONFIRMED_FACT_SOURCE"
    }
  ],
  "required_evidence": [
    "evidence/stages/S02/authority-contract-tests.txt",
    "evidence/stages/S02/fact-provenance-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-domain authority_contract fact_provenance --all-features",
  "pass_criteria": [
    "authority_patch_rejected",
    "fork_creates_new_contract",
    "confirmed_fact_sources_limited"
  ]
}
```
