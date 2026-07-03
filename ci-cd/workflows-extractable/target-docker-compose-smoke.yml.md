# Extractable workflow for `.github/workflows/docker-compose-smoke.yml`

> Codex 提取本文件第一个 fenced `yaml` 代码块，写入 `.github/workflows/docker-compose-smoke.yml`。

# `.github/workflows/docker-compose-smoke.yml`

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
