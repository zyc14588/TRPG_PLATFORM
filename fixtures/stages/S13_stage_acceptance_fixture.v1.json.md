# S13 Stage Acceptance Fixture v1

```json
{
  "stage": "S13",
  "stage_dir": "s13-v1-release-hardening",
  "required_documents": [
    "stages/s13-v1-release-hardening/START_PROMPT.md",
    "stages/s13-v1-release-hardening/ACCEPTANCE_PROMPT.md",
    "stages/s13-v1-release-hardening/TEST_PLAN.md",
    "stages/s13-v1-release-hardening/TEST_DATA.md",
    "stages/s13-v1-release-hardening/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S13_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S13_TEST_RESULTS.md",
    "docs/reports/stages/S13_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
