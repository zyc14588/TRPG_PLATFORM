# Extractable workflow for `.github/workflows/ci.yml`

> Codex 提取本文件第一个 fenced `yaml` 代码块，写入 `.github/workflows/ci.yml`。

# `.github/workflows/ci.yml`

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
