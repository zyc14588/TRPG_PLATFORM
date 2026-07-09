# BATCH-045 Work Plan

Batch: `BATCH-045-12-extension-sdk`
Stage: `S12`
Primary prompts in batch: `0`

## Prompt Mapping

| Prompt ID | Current-safe target | Allowed scope | Test responsibility |
|---|---|---|---|
| `CODEX-0965-12-EXTENSION-SDK-c9272e9f13` | `docs/codex/12-extension-sdk/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_plugin_sdk.md` | Traceability Markdown only | Covered by S12 fixture acceptance command |
| `CODEX-0966-12-EXTENSION-SDK-7277a3f410` | `docs/codex/12-extension-sdk/source_processing_record_docs_platform_plugin_sdk.md` | Traceability Markdown only | Covered by S12 fixture acceptance command |
| `CODEX-0967-12-EXTENSION-SDK-3c1737e6c1` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0967-12-EXTENSION-SDK-3c1737e6c1.md` | Supplemental requirement, merged to `CODEX-0946` | No Rust ownership; primary tests own implementation |
| `CODEX-0968-12-EXTENSION-SDK-e572d9864c` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0968-12-EXTENSION-SDK-e572d9864c.md` | Supplemental requirement, merged to `CODEX-0947` | No Rust ownership; primary tests own implementation |
| `CODEX-0969-12-EXTENSION-SDK-e024be282d` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0969-12-EXTENSION-SDK-e024be282d.md` | Supplemental requirement, merged to `CODEX-0105` | No Rust ownership; primary tests own implementation |
| `CODEX-0970-12-EXTENSION-SDK-266ae0e5ee` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0970-12-EXTENSION-SDK-266ae0e5ee.md` | Supplemental requirement, merged to `CODEX-0957` | No Rust ownership; primary tests own implementation |
| `CODEX-0971-12-EXTENSION-SDK-f4f12f9055` | `docs/codex/90-traceability/supplemental-requirements/CODEX-0971-12-EXTENSION-SDK-f4f12f9055.md` | Supplemental requirement, merged to `CODEX-0107` | No Rust ownership; primary tests own implementation |

## Scope Controls

- No migration, API handler, NATS subject, workflow, metric, or production SDK behavior was added for this batch.
- Historical source names, source hashes, and old version tokens remain provenance only.
- UI role snapshot and developer boundary automation are owned by `CODEX-0865-10-TESTING-QUALITY-aea366b339` / `testing_quality::requirement_to_test_trace`, not by BATCH-045 supplemental prompts.
- The Node harness is fixture automation only: it parses the detailed S12 fixture, generates deterministic SVG snapshots, computes `sha256` hashes, and writes S12 evidence.
