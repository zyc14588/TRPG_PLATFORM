<!-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 -->

# Linux M1 lifecycle certification

Run the frozen command from a clean candidate with local Docker access:

```
go run ./tests/smoke/m1_lifecycle --compose-file=deploy/compose.yaml --candidate-sha=HEAD --evidence=tests/smoke/.artifacts/m1-lifecycle.json
```

The harness creates a unique project, builds static source-bound binaries,
verifies container readback, uses the pinned cached PostgreSQL image, exercises
installed five-role packages, permitted WebSocket views, commit/broadcast,
restart/replay, offline safe migration and recorded point restoration. It
stores hashes and actual process reaping observations in atomic phase artifacts.
Its test profile uses host networking only for the existing loopback-only
internal daemon; PostgreSQL publishes a random loopback port. It needs no
privileged container, Docker socket mount, external model or production service.

Cleanup is registered before startup, uses an independent bounded context on
success/error/SIGINT/SIGTERM, removes project volumes/orphans and its owned
runtime image, and checks actual container/network/volume absence. Any phase,
artifact write or cleanup error returns nonzero. For actual cleanup regression
runs, `TRPG_M1_SMOKE_FAIL_AFTER=postgres-health` injects a failure after owned SQL
startup. `TRPG_M1_SMOKE_PAUSE_AFTER=postgres-health` waits for an actual signal;
the machine result must be FAIL with cleanup verified. These switches are
confined to this test harness and do not change the platform.

Windows/macOS are NOT_RUN. This is an M1 disposable test, not deployment,
backup, scaling or operations readiness.
