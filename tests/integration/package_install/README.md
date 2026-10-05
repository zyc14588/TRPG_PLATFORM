<!-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 -->

# M1-B003 real-service gate

Use a dedicated temporary PostgreSQL instance bound to loopback with a bootstrap
database named `b003`. The tests create random `b003_*` databases and remove only
those databases. `B003_POSTGRES_DSN` is required; missing service access fails the
gate instead of silently skipping. Objects and staging live in private temporary
directories. No existing account, room, Session, production database or deploy
configuration is used.

```
B003_POSTGRES_DSN='postgres://b003:b003-test-only@127.0.0.1:PORT/b003?sslmode=disable' \
  go test -count=1 -json -tags=integration ./tests/integration/package_install/...
```

The native Linux gate records the exact PostgreSQL image digest/version, source
commit/tree, command and exit status alongside the JSON test stream. Cases cover
validation-before-visibility, real transaction rollback and backend termination,
lost commit acknowledgement, cancellation, exact retries, concurrent publication,
tenant authorization, immutable deduplication, unsupported migration preflight,
production-runner execution and canonical/opaque extension preservation. Portable
archive/policy regressions remain in `internal/package/install` for Windows CI.
