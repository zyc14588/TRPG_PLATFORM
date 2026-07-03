# S11 Stage Acceptance Fixture v1

```json
{
  "stage": "S11",
  "stage_dir": "s11-testing-quality-golden-ci",
  "required_documents": [
    "stages/s11-testing-quality-golden-ci/START_PROMPT.md",
    "stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md",
    "stages/s11-testing-quality-golden-ci/TEST_PLAN.md",
    "stages/s11-testing-quality-golden-ci/TEST_DATA.md",
    "stages/s11-testing-quality-golden-ci/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S11_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S11_TEST_RESULTS.md",
    "docs/reports/stages/S11_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
