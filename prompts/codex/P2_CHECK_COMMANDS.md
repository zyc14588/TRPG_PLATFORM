# P2 Check Commands

These commands are written for Windows PowerShell from the repository root. Use Bash syntax only when the active shell is clearly Bash-like.

Windows Codex App 默认使用 PowerShell。所有可复制执行的命令块必须使用 `powershell` fence。避免 Bash-only 的错误吞咽、条件链、pipefail 初始化、POSIX 递归建目录参数、Bash 环境变量导出写法和 Unix 临时目录路径；改用 `$LASTEXITCODE`、`New-Item -Force`、`$env:VAR = "value"` 和 `$env:TEMP` 等 PowerShell 形式。

P2 implementation must not start until the P1.5 Fix Gate is green. These checks are command guidance only; they do not mark P2 complete by themselves.

## Preflight

```powershell
git status --short
git branch --show-current
git diff --check

rg -n "P2|RAG|ingest|retrieval|document" docs README.md crates apps schemas
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while scanning P2/RAG context"
}

rg -n "TODO|FIXME|panic!|unwrap\(|expect\(" crates apps docs
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while scanning TODO/FIXME patterns"
}

rg -n "/healthz|/readyz|/metrics" crates/server crates/observability schemas/openapi.json
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while checking health/readiness/metrics routes"
}
```

## Windows prompt command hygiene

This command fails if Bash-only command fragments remain in Windows Codex prompt docs.

```powershell
$bashOnlyPattern = '\|\| true|\&\& exit 1|set\s+-euo\s+pipefail|cat .*> [\/]tmp[\/]|mkdir\s+-p|export\s+[A-Za-z_][A-Za-z0-9_]*=|[\/]tmp[\/]'
$bashOnlyMatches = rg -n $bashOnlyPattern .codex docs/p2 prompts/codex CODEX_P2_MASTER_PROMPT.md AGENTS.P2.ADDENDUM.md
if ($LASTEXITCODE -eq 0) {
  throw "Bash-only command fragments remain in Windows Codex prompts"
} elseif ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} else {
  throw "rg failed while checking Bash-only command fragments"
}
```

## Generated artifact hygiene

This command fails if generated build artifacts are tracked.

```powershell
$artifactPattern = '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)'
$artifactMatches = git ls-files | Select-String -Pattern $artifactPattern
if ($artifactMatches) {
  $artifactMatches | ForEach-Object { $_.Line }
  throw "Tracked generated artifacts found."
}
```

## Rust quick gate

```powershell
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Rust tests

Use narrow package tests during batches, then run the workspace gate before completion.

```powershell
cargo test -p rag_core
cargo test -p document_ingestor
cargo test -p storage
cargo test -p server
cargo test --workspace
```

## SQLx / DB gate

Requires a real PostgreSQL database with pgvector if migrations require it. Use a non-superuser application role for realistic RLS checks.

```powershell
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

## Frontend gate

Run from the repository root unless the package manager setup says otherwise.

```powershell
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

## Local development environment example

PowerShell environment variables for local testing:

```powershell
$env:TRPG_AUTH_MODE = "development"
$env:TRPG_AUTH_SECRET = "development-secret-at-least-32-bytes-change-me"
$env:TRPG_ALLOW_IN_MEMORY_STORE = "true"
$env:TRPG_CONFIG_PATH = "config/default.toml"
$env:DATABASE_URL = "postgres://trpg_app:trpg_app@localhost:5432/trpg_platform"
$env:NEXT_PUBLIC_API_BASE_URL = "http://127.0.0.1:8080"
```

For production validation tests, do not use `postgres` as the application login user.

## Preparation batch - docs/control only

Use this for P2 Master Preparation when only planning or check-command Markdown files changed.

```powershell
git status --short
git branch --show-current
git diff --check
rg -n "P2|RAG|ingest|retrieval|document" docs README.md crates apps schemas
if ($LASTEXITCODE -eq 1) {
  $global:LASTEXITCODE = 0
} elseif ($LASTEXITCODE -ne 0) {
  throw "rg failed while scanning P2/RAG context"
}
```

If Rust files changed, also run `cargo fmt --all --check`. If frontend files changed, also run `pnpm lint`.

## Batch-specific commands

### Batch 00 - P1.5 Fix Gate

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test -p server
cargo test -p storage
pnpm lint
pnpm test
```

### Batch 01 - Domain

```powershell
cargo fmt --all --check
cargo test -p rag_core -p document_ingestor
```

### Batch 02 - Storage/RLS

```powershell
cargo fmt --all --check
cargo test -p storage
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

### Batch 03 - Ingest Worker

```powershell
cargo fmt --all --check
cargo test -p rag_core -p document_ingestor -p storage
cargo test --workspace
```

### Batch 04 - Server API

```powershell
cargo fmt --all --check
cargo test -p server
cargo test --workspace
```

### Batch 05 - Frontend UI

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

### Batch 06 - Hardening

Run the full P2 gate and copy exact results into `docs/status/P2_STATUS.md`.

## Full P2 gate

Run commands one by one in PowerShell so failures are visible and attributable:

```powershell
git status --short
git diff --check
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

Then repeat the artifact and P2/RAG searches from the Preflight section and record results in the batch summary.
