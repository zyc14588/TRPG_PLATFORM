# 00-index Module Overview

> Prompt ID: CODEX-0005-00-INDEX-be84920579
> Role: documentation-or-traceability
> Current module: docs_governance::m_00_index

The `00-index` module owns project indexing and construction-governance
materials for Codex execution.

## Scope

- Authority documents and reading paths.
- Prompt execution maps and current-safe output maps.
- Batch plans and per-file prompt manifests.
- Traceability evidence for documentation governance.

## Out Of Scope

- Business state writes.
- Rust implementation modules.
- API handlers, migrations, event schemas, NATS subjects, metrics, workflows,
  and model provider calls.

## Test Responsibility

- Validate prompt inventory closure.
- Validate Markdown target closure.
- Validate no source-derived or hash-derived current names are introduced.
