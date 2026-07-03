# COC AI KP Governance

> Prompt ID: CODEX-0127-00-INDEX-57961fa613
> Role: documentation-or-traceability
> Current module: docs_governance::coc_ai_kp
> Current output: `docs/codex/00-index/coc_ai_kp.md`

This file records the current-safe documentation boundary for AI Keeper
governance in the 00-index layer.

## Boundary

- AI Keeper requirements are governed by the top-level design and Authority
  Contract rules.
- This document may summarize governance constraints for traceability.
- This document does not implement providers, tools, orchestrators, rule
  engines, database writes, or event-log writes.

## Protected Constraints

- Business logic must not call a bare LLM provider directly.
- AI capability must route through Agent Gateway and the agent runtime layer.
- AI must not forge dice, bypass rules or state services, write formal state
  directly, mutate Authority Contract, or leak restricted visibility content.
- Local providers are first-class providers, but cross-boundary fallback must be
  explicit, configured, and audited.

## Test Responsibility

Validate that Batch-002 keeps this output as Markdown governance only and does
not create implementation artifacts for AI Keeper behavior.
