# P2 Codex Session Start

Paste this at the beginning of every new Codex P2 session.

You are in the TRPG_PLATFORM repository.

First, run this lightweight read-only PowerShell context check from the repository root:

```powershell
$ErrorActionPreference = "Stop"

git status --short

rg -n "P2|RAG|SQLx|Windows|PowerShell" README.md docs prompts .codex
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while reading project context"
}
```

Then read:
- README.md
- CODEX_MASTER_PROMPT.md, if present
- CODEX_P2_MASTER_PROMPT.md
- docs/P1_5_FIX_PLAN.md
- docs/SECURITY_RLS_POLICY.md
- docs/LEGAL_POLICY.md
- docs/RAG_DESIGN.md
- docs/P2_CODEX_HANDOFF.md
- docs/P2_RAG_IMPLEMENTATION_SPEC.md
- docs/P2_RAG_ACCEPTANCE_TESTS.md
- docs/p2/INDEX.md
- docs/p2/00_P1_5_FIX_GATE.md
- docs/p2/01_P2_MASTER_SPEC.md
- prompts/codex/P2_CHECK_COMMANDS.md

Do not implement future batches. Ask no product-scope questions unless a blocking contradiction exists. Make the smallest coherent patch for the requested batch, add tests, run the batch checks, and summarize exact results.
