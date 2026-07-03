# S07 Stage Acceptance Fixture v1

```json
{
  "stage": "S07",
  "stage_dir": "s07-agent-runtime-provider-memory-rag",
  "required_documents": [
    "stages/s07-agent-runtime-provider-memory-rag/START_PROMPT.md",
    "stages/s07-agent-runtime-provider-memory-rag/ACCEPTANCE_PROMPT.md",
    "stages/s07-agent-runtime-provider-memory-rag/TEST_PLAN.md",
    "stages/s07-agent-runtime-provider-memory-rag/TEST_DATA.md",
    "stages/s07-agent-runtime-provider-memory-rag/REPAIR_PROMPT.md"
  ],
  "required_evidence": [
    "docs/reports/stages/S07_ACCEPTANCE_EVIDENCE.md",
    "docs/reports/stages/S07_TEST_RESULTS.md",
    "docs/reports/stages/S07_TRACEABILITY.md"
  ],
  "acceptance_policy": {
    "p0_findings_allowed": 0,
    "p1_findings_allowed": 0,
    "may_skip_original_batch": false,
    "may_weaken_tests": false
  }
}
```
