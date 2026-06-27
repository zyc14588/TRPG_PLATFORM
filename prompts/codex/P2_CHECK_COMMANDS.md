# P2 Check Commands

## Preflight

```bash
git status --short
rg -n "TODO|FIXME|panic!|unwrap\(|expect\(" crates apps docs || true
rg -n "/healthz|/readyz|/metrics" crates/server crates/observability schemas/openapi.json || true
git ls-files | rg '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)' && exit 1 || true
```

## Rust quick gate

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Rust tests

```bash
cargo test -p rag_core
cargo test -p document_ingestor
cargo test -p storage
cargo test -p server
cargo test --workspace
```

## SQLx / DB gate

Requires a real PostgreSQL database with pgvector if migrations require it.

```bash
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

## Frontend gate

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

## Full P2 gate

```bash
set -euo pipefail

git status --short
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
rg -n "TODO|FIXME|panic!|unwrap\(|expect\(" crates apps docs || true
git ls-files | rg '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)' && exit 1 || true
```

## Local development environment example

Use a non-superuser app role for realistic RLS checks. Exact passwords and DB names may differ.

```bash
export TRPG_AUTH_MODE=development
export TRPG_AUTH_SECRET=development-secret-at-least-32-bytes-change-me
export TRPG_CONFIG_PATH=config/default.toml
export DATABASE_URL=postgres://trpg_app:trpg_app@localhost:5432/trpg_platform
export NEXT_PUBLIC_API_BASE_URL=http://127.0.0.1:8080
```

## Batch-specific commands

### Batch 00

```bash
cargo test -p server
pnpm test
```

### Batch 01

```bash
cargo test -p rag_core -p document_ingestor
```

### Batch 02

```bash
cargo test -p storage
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
```

### Batch 03

```bash
cargo test -p rag_core -p document_ingestor -p storage
cargo test --workspace
```

### Batch 04

```bash
cargo test -p server
cargo test --workspace
```

### Batch 05

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

### Batch 06

Run full P2 gate.
