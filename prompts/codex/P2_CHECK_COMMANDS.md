# P2 Check Commands — Windows Friendly

## Baseline

```powershell
git status --short
git branch --show-current
git log --oneline -8
cargo metadata --no-deps
```

## Rust

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Package-specific examples:

```powershell
cargo test -p rag_core
cargo test -p storage
cargo test -p document_ingestor
cargo test -p agent_engine
cargo test -p server
```

## SQLx / PostgreSQL

```powershell
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

If DB is unavailable, record exact error and continue non-DB checks.

## Frontend

```powershell
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

If E2E is configured and environment is ready:

```powershell
pnpm test:e2e
```

## Static searches

```powershell
rg -n "TODO|FIXME|panic!|unwrap\(|expect\(" crates apps docs
rg -n "pending_review|denied|LocalOnly|citation|content_hash|top_k|RLS|review" crates apps docs schemas migrations
rg -n "API_KEY|SECRET|DATABASE_URL|Authorization|Bearer|Cookie|csrf" crates apps docs schemas .env.example
```

## Generated artifact hygiene

```powershell
$matches = git ls-files | rg '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)'
if ($LASTEXITCODE -eq 0) {
  Write-Host "Tracked generated artifacts found:"
  Write-Host $matches
  exit 1
}
elseif ($LASTEXITCODE -eq 1) {
  Write-Host "No tracked generated artifacts found."
}
else {
  exit $LASTEXITCODE
}
```

## Diff hygiene

```powershell
git diff --check
git diff --stat
git diff --name-status
```
