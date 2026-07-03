# Visibility Redaction Matrix v1

```json
{
  "redaction_cases": [
    {
      "case_id": "keeper_secret_not_in_player_export",
      "source_labels": [
        "keeper_only"
      ],
      "derived_object": "player_export",
      "principal": "user_player_a",
      "expected": "REDACTED"
    },
    {
      "case_id": "private_to_player_not_party_visible",
      "source_labels": [
        "private_to_player:user_player_a"
      ],
      "derived_object": "session_summary_party",
      "principal": "user_player_b",
      "expected": "REDACTED"
    },
    {
      "case_id": "ai_internal_never_exported",
      "source_labels": [
        "ai_internal"
      ],
      "derived_object": "any_player_or_kp_export",
      "principal": "user_human_kp",
      "expected": "REDACTED_OR_AUDIT_ONLY"
    },
    {
      "case_id": "combined_visibility_most_restrictive",
      "source_labels": [
        "public",
        "keeper_only"
      ],
      "derived_object": "agent_context_result",
      "expected_label": "keeper_only"
    },
    {
      "case_id": "rag_keeper_chunk_not_in_player_context",
      "source_labels": [
        "keeper_only"
      ],
      "derived_object": "agent_context_for_player",
      "principal": "user_player_a",
      "expected": "OMITTED"
    }
  ]
}
```
