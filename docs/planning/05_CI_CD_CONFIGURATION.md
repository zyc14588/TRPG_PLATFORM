# 05 — CI/CD 配置

## 1. CI/CD 目标

CI/CD 必须证明：Rust workspace 可编译、格式与 clippy 无 warning、单元/集成/契约/Golden/泄露/模型认证测试通过、Docker Compose 可启动、迁移可运行、备份恢复与 projection rebuild 可验证。

## 2. GitHub Actions：`.github/workflows/ci.yml`

```yaml

name: ci

on:
  pull_request:
  push:
    branches: [main]

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  rust-workspace:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo tools
        run: |
          if ! command -v cargo-deny >/dev/null 2>&1; then cargo install cargo-deny --locked; fi
          if ! command -v cargo-audit >/dev/null 2>&1; then cargo install cargo-audit --locked; fi
      - name: Format
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - name: Test
        run: cargo test --workspace --all-features
      - name: Dependency policy
        run: cargo deny check
      - name: Security audit
        run: cargo audit
```

## 3. GitHub Actions：`.github/workflows/contracts.yml`

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

## 4. GitHub Actions：`.github/workflows/golden-scenarios.yml`

```yaml

name: golden-scenarios

on:
  pull_request:
  workflow_dispatch:

jobs:
  golden:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Golden Scenario CI
        run: cargo test -p trpg-testing --test golden_scenarios_ci --all-features -- --nocapture
      - name: Model Certification Tests
        run: cargo test -p trpg-testing --test model_certification_tests --all-features -- --nocapture
      - name: Export snapshots
        run: cargo test -p trpg-testing --test export_snapshot_tests --all-features -- --nocapture
      - name: Upload golden artifacts
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: golden-scenario-artifacts
          path: |
            target/golden/**
            artifacts/test-reports/**
```

## 5. GitHub Actions：`.github/workflows/docker-compose-smoke.yml`

```yaml

name: docker-compose-smoke

on:
  pull_request:
  workflow_dispatch:

jobs:
  compose:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build and start
        run: docker compose -f docker-compose.ci.yml up -d --build
      - name: Wait for health
        run: ./scripts/ci/wait-for-health.sh http://localhost:8080/healthz 120
      - name: Init smoke
        run: ./scripts/ci/init-smoke.sh
      - name: Provider boundary smoke
        run: ./scripts/ci/provider-boundary-smoke.sh
      - name: Logs
        if: always()
        run: docker compose -f docker-compose.ci.yml logs --no-color > compose.log
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: compose-logs
          path: compose.log
      - name: Stop
        if: always()
        run: docker compose -f docker-compose.ci.yml down -v
```

## 6. GitHub Actions：`.github/workflows/release.yml`

```yaml

name: release-gate

on:
  workflow_dispatch:
    inputs:
      release_candidate:
        description: Release candidate tag
        required: true

jobs:
  release-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Full workspace tests
        run: |
          cargo fmt --all -- --check
          cargo clippy --workspace --all-targets --all-features -- -D warnings
          cargo test --workspace --all-features
      - name: V1 acceptance suite
        run: ./scripts/ci/v1-acceptance-suite.sh
      - name: Backup restore drill
        run: ./scripts/backup_restore/smoke.sh
      - name: Projection rebuild drill
        run: ./scripts/projection_rebuild/verify.sh
      - name: Generate release evidence
        run: ./scripts/ci/generate-v1-acceptance-report.sh
      - uses: actions/upload-artifact@v4
        with:
          name: v1-acceptance-evidence
          path: docs/reports/V1_ACCEPTANCE_REPORT.md
```

## 7. CI 失败时交给 Codex 的修复流程

1. 使用 `prompts/persistent/12_CI_FAILURE_TRIAGE_PROMPT.md`。
2. 输入失败 job、日志片段、当前分支、相关阶段编号、最近变更文件。
3. Codex 必须先分类：编译、格式、clippy、测试、migration、contract、leakage、Golden、compose、provider boundary。
4. Codex 只能最小修复；禁止删除测试、弱化 policy、绕过 Event Store、关闭 visibility redaction。
5. 修复后必须重跑失败命令及其上游相关命令。

## 8. 发布门禁

发布只接受 `release-gate` 产出的 `docs/reports/V1_ACCEPTANCE_REPORT.md`，且 P0/P1 defects 为 0。


## v2.21 Canonical CI/CD Source

当前唯一 workflow 提取源是 `ci-cd/workflows-extractable/target-*.yml.md`。历史 `github-actions-*.yml.md` 已转入 `source-archive/provenance/**`，只用于 provenance。


## v2.21 GitHub Actions service-command correction

The canonical workflow source uses `jobs.<job_id>.services.<service_id>.command` for service container command overrides such as NATS JetStream (`-js`) and MinIO (`server /data`). `services.options` is reserved for Docker create resource options and must not be used to pass image commands. Tool installation steps are fail-fast: `cargo-deny`, `cargo-audit`, and `sqlx-cli` must either already exist or install successfully before downstream checks run.
