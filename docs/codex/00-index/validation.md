# 00-index Validation Boundary

> Prompt ID: CODEX-0004-00-INDEX-7216d1d127
> Role: documentation-or-traceability
> Current module: docs_governance::validation

This file records the validation boundary for 00-index governance outputs.

## Current Validation Rules

- Prompt IDs must be unique.
- Batch rows must reference existing per-file prompt files.
- Output paths must use current-safe normalized targets.
- Documentation prompts must not create business Rust, database, API, event,
  NATS, metric, workflow, or model-provider artifacts.
- Historical source labels remain provenance only.

## Batch-001 Checks

- Count `B001.md` prompt rows: 25.
- Count B001 primary prompts: 0.
- Check all B001 prompt files exist.
- Check all B001 targets are Markdown governance outputs.
