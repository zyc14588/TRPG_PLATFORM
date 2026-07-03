# Detailed Stage Acceptance Fixture — v2.21

```json
{
  "stage": "S07",
  "purpose": "验证 Provider Adapter、本地模型认证、RAG visibility 和 No Silent Cloud Fallback。",
  "inputs": {
    "local_model": {
      "provider": "ollama",
      "model_id": "qwen-coc-local",
      "level": "LOCAL_MODEL_LEVEL_3"
    },
    "fallback_policy": "disabled",
    "rag_chunks": [
      {
        "id": "rule_001",
        "visibility": "public"
      },
      {
        "id": "secret_001",
        "visibility": "keeper_only"
      }
    ]
  },
  "actions": [
    {
      "id": "capability_probe",
      "type": "model_certification"
    },
    {
      "id": "tool_use_eval",
      "type": "model_certification"
    },
    {
      "id": "rag_search_player",
      "type": "rag_query"
    },
    {
      "id": "fallback_attempt",
      "type": "provider_route"
    }
  ],
  "expected_events": [
    {
      "type": "ModelCertificationRecorded",
      "level": "LOCAL_MODEL_LEVEL_3"
    },
    {
      "type": "FallbackBlocked",
      "reason": "NO_SILENT_CLOUD_FALLBACK"
    }
  ],
  "expected_records": [
    {
      "record": "ModelRouteSnapshot",
      "required_fields": [
        "provider_type",
        "model_id",
        "fallback_policy",
        "privacy_boundary"
      ]
    },
    {
      "record": "RAGChunk",
      "required_fields": [
        "source_type",
        "visibility",
        "version",
        "allowed_use"
      ]
    }
  ],
  "expected_errors": [
    {
      "case": "level3_as_ai_keeper",
      "error": "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP"
    },
    {
      "case": "silent_cloud_fallback",
      "error": "SILENT_FALLBACK_FORBIDDEN"
    },
    {
      "case": "player_rag_keeper_chunk",
      "error": "RAG_VISIBILITY_SCOPE_VIOLATION"
    }
  ],
  "failure_cases": [
    {
      "id": "business_layer_calls_llm",
      "expected_error": "DIRECT_LLM_CALL_FORBIDDEN"
    }
  ],
  "required_evidence": [
    "evidence/stages/S07/provider-adapter-tests.txt",
    "evidence/stages/S07/model-certification-tests.txt",
    "evidence/stages/S07/rag-visibility-tests.txt"
  ],
  "automation_target": "cargo test -p trpg-agent provider_adapter model_certification rag_visibility --all-features",
  "pass_criteria": [
    "provider_adapter_only",
    "level4_required_for_ai_kp",
    "no_silent_fallback",
    "rag_visibility_enforced"
  ]
}
```
