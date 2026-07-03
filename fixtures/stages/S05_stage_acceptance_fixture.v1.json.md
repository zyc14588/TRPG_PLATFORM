# S05 Stage Acceptance Fixture v1

```json
{
  "stage": "S05",
  "stage_dir": "s05-ruleset-coc7-engine",
  "required_documents": [
    "stages/s05-ruleset-coc7-engine/START_PROMPT.md",
    "stages/s05-ruleset-coc7-engine/ACCEPTANCE_PROMPT.md",
    "stages/s05-ruleset-coc7-engine/TEST_PLAN.md",
    "stages/s05-ruleset-coc7-engine/TEST_DATA.md",
    "stages/s05-ruleset-coc7-engine/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S05_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S05_TEST_RESULTS.md",
    "docs/reports/stages/S05_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
