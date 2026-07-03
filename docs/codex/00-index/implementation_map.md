# Implementation Map

> Prompt ID: CODEX-0010-00-INDEX-68fb192697
> Role: documentation-or-traceability
> Current module: docs_governance::implementation_map

This map records the implementation boundary for BATCH-001 and BATCH-002.

## Batch-001 Boundary

- Prompt count: 25.
- Primary prompts: 0.
- Allowed outputs: Markdown governance and traceability documents.
- Disallowed outputs: concrete Rust implementation, tests, migrations, handlers,
  event schemas, NATS subjects, workflows, metrics, and provider code.

## Current Status

BATCH-001 is implemented as documentation governance output only. Any source
excerpt that describes code, SQL, API, NATS, or tests remains provenance until a
future primary implementation prompt owns that artifact.

## Batch-002 Boundary

- Prompt count: 23.
- Primary prompts: 0.
- Prompt IDs: `CODEX-0126` through `CODEX-0146`, plus `CODEX-1108` and
  `CODEX-1109`.
- Allowed outputs: Markdown governance, index, mapping, traceability, and
  evidence documents.
- Disallowed outputs: concrete Rust implementation, tests, migrations, handlers,
  event schemas, NATS subjects, workflows, metrics, and provider code.

Batch-002 may normalize documentation references through the current-safe maps.
It must not adopt historical V3/V4/V5/V6 names as current implementation names.
