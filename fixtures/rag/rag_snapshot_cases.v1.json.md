# RAG Snapshot Cases v1

```json
{
  "chunks": [
    {
      "chunk_id": "rules_coc7_skill_check_001",
      "source_type": "ruleset_pack",
      "visibility": "public",
      "copyright_status": "system_original_or_allowed",
      "version": "coc7-pack-0.1.0",
      "owner": "system",
      "allowed_use": "internal_gameplay",
      "embedding_model": "local-embedding-test",
      "chunk_hash": "91a0e921167aaa57c134c2b1bc549d4ff0c75f4a3f7503dd23a396726d63b831"
    },
    {
      "chunk_id": "scenario_keeper_truth_001",
      "source_type": "scenario",
      "visibility": "keeper_only",
      "copyright_status": "original",
      "version": "golden_salt_bell-0.1.0",
      "owner": "campaign_owner",
      "allowed_use": "campaign_only",
      "expected_player_context": "REDACTED"
    }
  ],
  "snapshot_tests": [
    {
      "case_id": "public_rules_chunk_available",
      "principal": "user_player_a",
      "chunk_id": "rules_coc7_skill_check_001",
      "expected": "ALLOW"
    },
    {
      "case_id": "keeper_truth_not_in_player_rag",
      "principal": "user_player_a",
      "chunk_id": "scenario_keeper_truth_001",
      "expected": "DENY"
    }
  ]
}
```
