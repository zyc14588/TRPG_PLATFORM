# Crate To Doc Map

> Prompt IDs: CODEX-0007-00-INDEX-60bd308841, CODEX-0117-00-INDEX-a7ff60b697
> Role: documentation-or-traceability
> Current module: docs_governance::crate_to_doc_map

This map keeps crate ownership aligned with documentation governance.

| Current crate | Documentation scope |
|---|---|
| trpg-docs-governance | `docs/codex/**`, batch plans, prompt manifests, traceability reports |

## Rules

- A crate name in a previous source file is advisory provenance unless it also
  appears in the current-safe maps.
- Documentation governance prompts do not create business code.
- Shared output ownership is allowed only for Markdown governance documents.

## Batch-001 Responsibility

- Both mapped Prompt IDs converge on this file.
- No duplicate Rust owner is created for this shared documentation target.

## Batch-002 Responsibility

- `CODEX-0137-00-INDEX-e702d6dd13` records the crate-to-document governance
  map for Batch-002.
- The current crate remains `trpg-docs-governance`.
- The mapping is documentary only and does not create a Rust crate or module.
