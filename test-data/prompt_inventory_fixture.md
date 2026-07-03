# Test Data — Prompt Inventory Fixture

```json
{
  "expected_prompt_count": 1109,
  "expected_batch_count": 52,
  "expected_primary_count": 257,
  "expected_supplemental_count": 451,
  "expected_documentation_count": 401,
  "rules": [
    "primary may create concrete Rust src/test outputs",
    "supplemental writes supplemental-requirements markdown only",
    "documentation writes markdown/index/matrix/report only",
    "no hash fragments in Rust module names",
    "no source path derived Rust output files"
  ]
}
```
