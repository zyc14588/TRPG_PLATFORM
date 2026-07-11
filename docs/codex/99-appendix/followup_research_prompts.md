# Follow-up research prompt template

> BATCH-051/BATCH-052 current-safe documentation output. No research topic is activated
> merely by appearing in this template.

## Current-safe ownership

| Prompt ID | Prompt file | Current crate | Current module | Current output | Source file (provenance only) | Source SHA256 |
|---|---|---|---|---|---|---|
| `CODEX-1074-99-APPENDIX-6adee21312` | `codex-prompts/99-appendix/P0003.md` | `trpg-docs-governance` | `appendix_research::followup_research_prompts` | `docs/codex/99-appendix/followup_research_prompts.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-followup-research-prompts-bc4ccd10d8.v5-code-ready.md` | `f47e69c914983039131aae1917cc34c5d6237e593fa610cbfca7dcee339ec34b` |
| `CODEX-1090-99-APPENDIX-110bb4e11e` | `codex-prompts/99-appendix/P0019.md` | `trpg-docs-governance` | `appendix_research::followup_research_prompts` | `docs/codex/99-appendix/followup_research_prompts.md` | `docs/implementation/90-traceability/per-file-code-ready/99-appendix/sources-v3-baseline-document-group-docs-implementation-99-appendix-followup-research-prompts-0e08a2e0b2.v5-code-ready.md` | `5389dbaf1c56e7d2140307185b5f4f367cf683b7a555739f10b3cb9c4e9e66f3` |
| `CODEX-1101-99-APPENDIX-d70d0fbeb4` | `codex-prompts/99-appendix/P0026.md` | `trpg-docs-governance` | `appendix_research::followup_research_prompts` | `docs/codex/99-appendix/followup_research_prompts.md` | `docs/implementation/99-appendix/followup-research-prompts.md` | `e3cef411e4f489eadf72a6ad378aaa2fe6552b62960e5edf5368177161c02b2e` |

## Copy-ready structure

~~~text
Research question: <one bounded question>
Decision owner: <stage or component owner>
Current authority inputs: <repository paths>
Current facts already verified: <facts>
Unknowns: <unknowns>
Allowed sources: official documentation, official repository, or primary paper
Required comparison: correctness, security, operations, migration, and cost
Required output: evidence-backed options, recommendation, risks, and no-change option
Prohibited: implementation, scope expansion, or changing an accepted invariant
~~~

Research results remain advisory until the owning stage accepts them. They may
not alter Authority, Agent Gateway, Event Store, Visibility, Fact Provenance,
or V1 scope through an appendix update.

In particular, the Authority Contract remains immutable and `HUMAN_KP` /
`AI_KP` remain mutually exclusive; research cannot weaken the default-deny
Policy Gate.

## B051/B052 boundary and test responsibility

The three source prompts provide no bounded current research question, so B051
and B052 do not invent or execute one. Checks cover all owners, metadata, map
agreement, the single shared target, fence parity, and the docs-only boundary.
