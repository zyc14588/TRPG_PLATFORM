# Canonical source for `.github/workflows/ci.yml`

```yaml
name: ci

on:
  pull_request:
    branches: [master, main]
  push:
    branches: [master, main]

permissions:
  contents: read

concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  workspace:
    runs-on: ubuntu-24.04
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
        with:
          fetch-depth: 2
      - uses: actions/setup-python@a309ff8b426b58ec0e2a45f0f869d46889d02405 # v6.2.0
        with:
          python-version: "3.14.6"
      - uses: actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e # v6.4.0
        with:
          node-version-file: .nvmrc
      - name: Pin pnpm
        run: corepack enable && corepack prepare pnpm@11.9.0 --activate
      - name: Install Node dependencies
        run: pnpm install --frozen-lockfile
      - name: Full CI with evidence
        run: |
          python3 scripts/ci/generate_evidence.py --report "$RUNNER_TEMP/ci-evidence.json" --artifact MANIFEST.md -- bash scripts/ci/test-all.sh
          python3 scripts/ci/verify_evidence_schema.py "$RUNNER_TEMP/ci-evidence.json"
      - name: Upload CI evidence
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: ci-${{ github.run_id }}-${{ github.run_attempt }}
          path: |
            ${{ runner.temp }}/ci-evidence.json
            ${{ runner.temp }}/ci-evidence.log
            ${{ runner.temp }}/ci-evidence.junit.xml
            ${{ runner.temp }}/ci-evidence.sarif
          if-no-files-found: error
          retention-days: 30
```
