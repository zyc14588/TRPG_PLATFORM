# Canonical source for `.github/workflows/docker-compose-smoke.yml`

```yaml
name: docker-compose-smoke

on:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: docker-compose-smoke-${{ github.ref }}
  cancel-in-progress: true

jobs:
  require-real-runtime:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
      - uses: actions/setup-python@a309ff8b426b58ec0e2a45f0f869d46889d02405 # v6.2.0
        with:
          python-version: "3.14.6"
      - name: Refuse placeholder Compose services
        run: bash scripts/ci/init-smoke.sh "$RUNNER_TEMP/release-readiness.json"
      - name: Upload blocked-runtime evidence
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: docker-compose-smoke-${{ github.run_id }}-${{ github.run_attempt }}
          path: ${{ runner.temp }}/release-readiness.json
          if-no-files-found: error
          retention-days: 30
```
