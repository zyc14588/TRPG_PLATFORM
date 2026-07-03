# Extractable workflow for `.github/workflows/contracts.yml`

> Codex 提取本文件第一个 fenced `yaml` 代码块，写入 `.github/workflows/contracts.yml`。

# `.github/workflows/contracts.yml`

```yaml

name: contracts-and-integration

on:
  pull_request:
  push:
    branches: [main]

jobs:
  contracts:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: pgvector/pgvector:pg16
        env:
          POSTGRES_USER: trpg
          POSTGRES_PASSWORD: trpg
          POSTGRES_DB: trpg_test
        ports: ['5432:5432']
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
      redis:
        image: redis:7
        ports: ['6379:6379']
      nats:
        image: nats:2
        ports: ['4222:4222']
        command: -js
      minio:
        image: minio/minio:latest
        ports: ['9000:9000']
        env:
          MINIO_ROOT_USER: minio
          MINIO_ROOT_PASSWORD: minio123
        command: server /data
    env:
      DATABASE_URL: postgres://trpg:trpg@localhost:5432/trpg_test
      REDIS_URL: redis://localhost:6379
      NATS_URL: nats://localhost:4222
      OBJECT_STORE_ENDPOINT: http://localhost:9000
      RUST_LOG: info
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install sqlx-cli
        run: if ! command -v sqlx >/dev/null 2>&1; then cargo install sqlx-cli --no-default-features --features native-tls,postgres --locked; fi
      - name: Migrate
        run: sqlx migrate run
      - name: Event store contract
        run: cargo test --test event_store_contract --all-features
      - name: Projection replay
        run: cargo test --test projection_replay --all-features
      - name: Visibility leakage
        run: cargo test --test visibility_leakage --all-features
      - name: API contract
        run: cargo test --test openapi_contract --all-features
      - name: WebSocket contract
        run: cargo test --test websocket_contract --all-features
      - name: NATS subject contract
        run: cargo test --test nats_subject_contract --all-features
```
