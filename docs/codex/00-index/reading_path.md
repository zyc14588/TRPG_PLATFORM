# Reading Path

> Prompt ID: CODEX-0013-00-INDEX-42bafcf994
> Role: documentation-or-traceability
> Current module: docs_governance::reading_path

## Required Reading Order

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. Current batch file.
9. Current batch per-file prompts.

## Rule

Do not execute a batch or per-file prompt before applying the normalized
current-safe maps.

## Batch-002 Path Resolution

The historical batch path
`docs/codex/90-traceability/execution-batches/batch-002-00-index.md` resolves
to the current batch file `batches/B002.md` through the inventory rewrite maps.

Batch-002 per-file prompts must be read from `codex-prompts/00-index/` and
applied only after resolving their safe modules and output paths through
`CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`,
`CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`, and
`CURRENT_TOKEN_REWRITE_TABLE.md`.
