# Source To Output Strict Map

> Prompt ID: CODEX-0135-00-INDEX-bcdd84df00
> Role: documentation-or-traceability
> Current module: docs_governance::source_to_output_strict_map
> Current output: `docs/codex/00-index/source_to_output_strict_map.md`

This file records Batch-002 source-to-output handling after current-safe
normalization.

| Rule | Batch-002 interpretation |
|---|---|
| Historical source path | Provenance only |
| Historical V3/V4/V5/V6 token | Provenance only |
| Current prompt file | `codex-prompts/00-index/P####.md` |
| Current module | Value from `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` |
| Current output | Value from `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` |
| Allowed output type | Markdown governance, traceability, or evidence |

## Strict Boundary

If a source document mentions implementation code, SQL, events, NATS subjects,
metrics, tests, workflows, providers, or runtime behavior, Batch-002 records the
requirement only. It does not create the implementation artifact.
