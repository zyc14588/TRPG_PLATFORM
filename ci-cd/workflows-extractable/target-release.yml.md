# Extractable workflow for `.github/workflows/release.yml`

> Codex 提取本文件第一个 fenced `yaml` 代码块，写入 `.github/workflows/release.yml`。

# `.github/workflows/release.yml`

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
