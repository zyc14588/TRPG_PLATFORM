# Contract Index

> Prompt ID: CODEX-0116-00-INDEX-5fe281d240
> Role: documentation-or-traceability
> Current module: docs_governance::contract_index

## Protected Contracts

- Authority Contract is immutable and fork-only.
- AI capabilities go through Agent Gateway, Runtime, and Provider Adapter.
- Formal state writes go through Command, Workflow, Decision, Event Store, and
  Projection.
- Event Store is canon.
- Visibility labels and fact provenance propagate through all derived outputs.

## Batch-001 Boundary

This file indexes governance contracts only. It does not define executable API,
event, database, NATS, metric, or workflow artifacts.
