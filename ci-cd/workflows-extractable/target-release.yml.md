# Canonical source for `.github/workflows/release.yml`

```yaml
name: release-readiness

on:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: release-readiness-${{ github.ref }}
  cancel-in-progress: true

jobs:
  require-ready:
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
      - name: Generate full release evidence
        run: |
          python3 scripts/ci/generate_evidence.py --report "$RUNNER_TEMP/release-evidence.json" --artifact MANIFEST.md -- bash scripts/ci/test-all.sh
          python3 scripts/ci/verify_evidence_schema.py "$RUNNER_TEMP/release-evidence.json"
      - name: Fail closed until real product runtime exists
        run: python3 scripts/ci/release_readiness.py --report "$RUNNER_TEMP/release-readiness.json" --evidence "$RUNNER_TEMP/release-evidence.json" --require-ready
      - name: Upload release evidence
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: release-readiness-${{ github.run_id }}-${{ github.run_attempt }}
          path: |
            ${{ runner.temp }}/release-evidence.json
            ${{ runner.temp }}/release-evidence.log
            ${{ runner.temp }}/release-evidence.junit.xml
            ${{ runner.temp }}/release-evidence.sarif
            ${{ runner.temp }}/release-readiness.json
          if-no-files-found: error
          retention-days: 30
```
