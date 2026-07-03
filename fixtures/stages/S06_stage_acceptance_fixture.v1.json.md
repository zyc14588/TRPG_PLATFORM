# S06 Stage Acceptance Fixture v1

```json
{
  "stage": "S06",
  "stage_dir": "s06-runtime-orchestration-decision-pipeline",
  "required_documents": [
    "stages/s06-runtime-orchestration-decision-pipeline/START_PROMPT.md",
    "stages/s06-runtime-orchestration-decision-pipeline/ACCEPTANCE_PROMPT.md",
    "stages/s06-runtime-orchestration-decision-pipeline/TEST_PLAN.md",
    "stages/s06-runtime-orchestration-decision-pipeline/TEST_DATA.md",
    "stages/s06-runtime-orchestration-decision-pipeline/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S06_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S06_TEST_RESULTS.md",
    "docs/reports/stages/S06_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
