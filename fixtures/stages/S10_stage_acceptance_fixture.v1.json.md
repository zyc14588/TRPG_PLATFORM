# S10 Stage Acceptance Fixture v1

```json
{
  "stage": "S10",
  "stage_dir": "s10-ops-migration-runbooks",
  "required_documents": [
    "stages/s10-ops-migration-runbooks/START_PROMPT.md",
    "stages/s10-ops-migration-runbooks/ACCEPTANCE_PROMPT.md",
    "stages/s10-ops-migration-runbooks/TEST_PLAN.md",
    "stages/s10-ops-migration-runbooks/TEST_DATA.md",
    "stages/s10-ops-migration-runbooks/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S10_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S10_TEST_RESULTS.md",
    "docs/reports/stages/S10_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
