# V1 Acceptance Evidence Schema v1

```json
{
  "evidence_schema": {
    "required_fields": [
      "v1_acceptance_item",
      "stage",
      "commit_sha",
      "test_command",
      "test_exit_code",
      "fixture_paths",
      "artifact_paths",
      "artifact_hashes",
      "reviewer",
      "decision"
    ],
    "allowed_decisions": [
      "PASS",
      "FAIL",
      "BLOCKED"
    ],
    "fail_on_missing_artifact": true,
    "fail_on_unlinked_fixture": true
  }
}
```
