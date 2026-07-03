# Document Organization and Audit Boundary — v2.21

## Purpose

This file is the authoritative boundary document for the v2.21 strict package. It tells auditors and Codex which directories are current execution inputs, which directories are design/reference materials, and which directories are provenance only.

## Project boundary

The package supports a COC 7 first-release AI / human Keeper online TRPG platform. The implementation target is not a chatbot. Codex must build a server-side game runtime with rules, event sourcing, visibility labels, Agent-governed AI, COC7 flows, deployment, testing, and release evidence.

## Current execution areas

The following paths are active Codex execution inputs:

```text
AGENTS.md
CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
CODEX_MASTER_EXECUTION_GUIDE.md
CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md
CODEX_STRICT_OPERATION_CHECKLIST.md
codex-operator-guides/**
prompts/persistent/**
stages/**
docs/codex/**
fixtures/**
ci-cd/workflows-extractable/**
```

## Design and audit areas

The following paths are for human review, design traceability, or acceptance evidence:

```text
docs/top-level-design/**
00_INPUT_ANALYSIS_AND_TRACEABILITY.md
01_OVERALL_CONSTRUCTION_PLAN.md
02_STAGE_CONFIRMATION_MATRIX.md
03_ENGINEERING_DIRECTORY_PLAN.md
04_TEST_STRATEGY_AND_TEST_DATA.md
05_CI_CD_CONFIGURATION.md
V1_ACCEPTANCE_EVIDENCE_MATRIX.md
inventory/**
manifests/**
DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md
STRICT_LINK_AND_REFERENCE_VALIDATION.md
```

## Provenance-only areas

The following paths are not current execution inputs. Codex may inspect them only to trace historical source material, not to choose current names, gates, prompts, or CI/CD sources.

```text
source-archive/**
```

## Canonical token rewrite rule

There is exactly one active canonical token rewrite file:

```text
docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
```

This file must exist and is the only current authority for rewriting historical V3/V4/V5/V6 or earlier package naming into current-safe construction names.

Historical token-rewrite aliases, must not exist outside `source-archive/**`. They may be preserved only as provenance and must never be read as current execution input.

## Canonical CI/CD source rule

The only active workflow extraction source is:

```text
ci-cd/workflows-extractable/target-*.yml.md
```

Historical `github-actions-*.yml.md` files are provenance only. They must not exist in active `ci-cd/` and must not be used by Codex when creating `.github/workflows/*.yml`.

## Review checklist

An audit pass must confirm:

```text
1. README.md describes the project before repair history.
2. README.md only declares directories that exist in the package.
3. Codex persistent prompts are under prompts/persistent/** and stages/**.
4. Human design and audit documents are separated from persistent prompts.
5. docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md exists.
6. No historical token-rewrite alias file exists outside source-archive/**.
7. ci-cd/workflows-extractable/target-*.yml.md is the sole active CI/CD source.
8. docs/codex/** current injected headers use v2.21 wording.
9. Codex guide files use real Markdown headings, not indented pseudo-headings or indented body prose.
10. source-archive/** is treated as provenance only.
```

## v2.21 extended boundary update

Human-programmer batch prompt references live under ``batch-prompts/start/B###.md` 与 `batch-prompts/accept/B###.md``. They are operator-facing prompt references, not source design documents. Canonical CI/CD extraction remains limited to `ci-cd/workflows-extractable/target-*.yml.md`; historical workflow examples, if any, belong only under `source-archive/**`.
