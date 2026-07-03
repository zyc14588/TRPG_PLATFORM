# Historical Cleanup Policy

> Prompt ID: CODEX-0011-00-INDEX-b7086d0435
> Role: documentation-or-traceability
> Current module: docs_governance::historical_cleanup_policy

Historical source material is retained for auditability, not as a current
construction authority.

## Policy

- Historical version labels may remain in provenance notes.
- Current output names must use normalized current-safe names.
- Old reports, manifests, and validation summaries are not current acceptance
  gates.
- `source-archive/**` is read-only provenance.

## Batch-001 Check

No B001-created file introduces a business artifact or a current name derived
from historical paths, hashes, or version labels.
