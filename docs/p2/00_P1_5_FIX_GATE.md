# P1.5 Fix Gate Before P2

P2 must not start until this gate is green. These checks are based on the current repository shape and are designed to prevent Codex from building RAG on top of unstable security, packaging, and environment assumptions.

## Gate A — local and production boot invariants

Expected behavior:

- Production requires an explicit `TRPG_AUTH_SECRET` of at least 32 bytes.
- Production requires a real `DATABASE_URL`.
- Production must reject a `postgres` superuser application login.
- Development/test may use in-memory auth store only when explicitly enabled.
- `.env.example` must include a copy-pasteable local development mode that does not accidentally trigger production validation.

Required fixes if absent:

```env
TRPG_AUTH_MODE=development
TRPG_AUTH_SECRET=development-secret-at-least-32-bytes-change-me
TRPG_ALLOW_IN_MEMORY_STORE=false
DATABASE_URL=postgres://trpg_app:<password>@localhost:5432/trpg_platform
```

Keep a commented production block showing the non-superuser requirement.

## Gate B — license declaration consistency

Status: resolved on 2026-06-27. Maintainer selected `AGPL-3.0-or-later`.

There must be one clear license story across:

- repository `LICENSE`
- GitHub license display
- `Cargo.toml` workspace package license
- README legal section

Do not proceed to P2 if these files drift away from the maintainer-selected `AGPL-3.0-or-later` policy.

## Gate C — health/readiness/metrics contract

The repo-level first-run scope calls for health/ready/metrics support. Before P2:

- Confirm exact implemented routes with `rg -n "/healthz|/readyz|/metrics" crates/server crates/observability schemas/openapi.json`.
- If absent, add:
  - `GET /healthz`: process is alive; no DB dependency.
  - `GET /readyz`: DB/migration-dependent readiness when DB is configured.
  - `GET /metrics`: Prometheus format, no secrets.
- Add route tests and OpenAPI entries.

## Gate D — generated artifact cleanup

Tracked generated artifacts must be removed from Git index. At minimum verify:

```powershell
$artifactPattern = '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)'
$artifactMatches = git ls-files | Select-String -Pattern $artifactPattern
if ($artifactMatches) {
  $artifactMatches | ForEach-Object { $_.Line }
  throw "Tracked generated artifacts found"
}
```

If `apps/web/tsconfig.tsbuildinfo` is tracked:

```powershell
git rm --cached apps/web/tsconfig.tsbuildinfo
```

## Gate E — frontend phase label and dependency stability

- Update stale “Phase 1B” UI copy to the current phase language.
- Avoid unpinned `latest` dependency ranges for production-facing app dependencies. Prefer lockfile-consistent explicit versions or semver ranges with a conscious policy.

## Gate F — server module size risk

P2 should not add another large block into `crates/server/src/lib.rs`. Before adding RAG endpoints, introduce modules such as:

```text
crates/server/src/routes/mod.rs
crates/server/src/routes/auth.rs
crates/server/src/routes/rooms.rs
crates/server/src/routes/rag.rs
crates/server/src/dto/mod.rs
crates/server/src/dto/rag.rs
crates/server/src/state.rs
crates/server/src/config.rs
```

Do this conservatively: move code without behavior change and keep tests green.

## Gate G — CSRF / cookie / bearer contract

Document and test the policy:

- Refresh/logout endpoints that depend on refresh cookies require CSRF.
- Bearer-token JSON mutations either do not require CSRF or require it consistently.
- If cookies are included with all client requests, ensure no route accidentally authenticates mutation by cookie without CSRF.

Add tests for the chosen policy.

## Gate commands

```powershell
$ErrorActionPreference = "Stop"

function Invoke-Checked {
  param(
    [scriptblock]$Command,
    [string]$Name
  )

  & $Command
  if ($LASTEXITCODE -ne 0) {
    throw "$Name failed with exit code $LASTEXITCODE"
  }
}

git status --short

Invoke-Checked { cargo fmt --all --check } "cargo fmt"
Invoke-Checked { cargo check --workspace } "cargo check"
Invoke-Checked { cargo clippy --workspace --all-targets -- -D warnings } "cargo clippy"
Invoke-Checked { cargo test --workspace } "cargo test"

if ($env:DATABASE_URL) {
  Invoke-Checked { cargo sqlx migrate run } "cargo sqlx migrate run"
  Invoke-Checked { cargo sqlx prepare --check --workspace } "cargo sqlx prepare"
} else {
  Write-Host "CONDITIONAL: DATABASE_URL is not set; skipped SQLx migrate/prepare. Do not mark this gate PASS until a DB-backed run is recorded."
}

Invoke-Checked { pnpm install --frozen-lockfile } "pnpm install"
Invoke-Checked { pnpm lint } "pnpm lint"
Invoke-Checked { pnpm typecheck } "pnpm typecheck"
Invoke-Checked { pnpm test } "pnpm test"
Invoke-Checked { pnpm test:e2e } "pnpm test:e2e"
Invoke-Checked { pnpm build } "pnpm build"

rg -n "TODO|FIXME|panic!|unwrap\(|expect\(" crates apps docs
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while scanning TODO/FIXME patterns"
}

rg -n "/healthz|/readyz|/metrics" crates/server crates/observability schemas/openapi.json
if ($LASTEXITCODE -eq 1) {
  throw "health/readiness/metrics contract references not found"
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while checking health/readiness/metrics contract"
}

$artifactPattern = '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)'
$artifactMatches = git ls-files | Select-String -Pattern $artifactPattern
if ($artifactMatches) {
  $artifactMatches | ForEach-Object { $_.Line }
  throw "Tracked generated artifacts found"
}
```

P1.5 is closed only when all commands either pass or the status report explains an intentional, reviewed exception.
