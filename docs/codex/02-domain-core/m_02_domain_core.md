# 02-domain-core Batch 007 Trace

## Scope

`BATCH-007-02-domain-core` establishes the first `trpg-domain-core`
slice for Authority Contract, Command/CQRS, DecisionRecord, fork lineage,
Visibility, and Fact Provenance.

## Current-safe Outputs

All Rust outputs use `crates/trpg-domain-core/src/<module>.rs` flat files
from `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`. Historical version/hash/path
tokens remain provenance only.

## Batch 007 Prompt Handling

- Documentation prompt: `CODEX-0023-02-DOMAIN-CORE-afd26a0bfd`.
- Primary implementation prompts: `CODEX-0024`, `CODEX-0025`, `CODEX-0026`,
  `CODEX-0027`, `CODEX-0028`, `CODEX-0029`, `CODEX-0030`, `CODEX-0237`,
  `CODEX-0239`, `CODEX-0245`, `CODEX-0246`, `CODEX-0247`, `CODEX-0248`.
- Supplemental prompts: `CODEX-0238`, `CODEX-0240`, `CODEX-0241`,
  `CODEX-0242`, `CODEX-0243`, `CODEX-0244`, `CODEX-0249`,
  `CODEX-0250`, `CODEX-0251`, `CODEX-0252`, `CODEX-0253`.

## Non-goals

No SQLx migrations, API handlers, NATS subjects, provider calls, or direct
LLM access are introduced by this batch.
