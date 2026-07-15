# P02 Complete Repair Test Results

All rows marked PASS completed with exit code `0` on 2026-07-15. Conditional integration tests were supplied their real backend environment; their process-level PASS was not inferred from a skipped test.

## Full and real-backend verification

| Gate | Result |
|---|---|
| `cargo test --workspace --all-targets --all-features --locked` on fresh `p02_complete4_*` databases | PASS; PostgreSQL primary/witness, Redis, NATS JetStream, OpenFGA, OPA, and verified-TLS PostgreSQL paths executed |
| Production API composition formal commit | PASS; OpenFGA → OPA → Runtime → canonical event/audit/outbox → independent witness; stale restart command rejected without in-memory publish |
| Canonical failure and concurrent-startup suite | PASS; primary rollback, witness finalization recovery, idempotent retry, tamper checks, and two concurrent `prepare_for_service` calls |
| Authenticated canonical replay transport | PASS; public/private/keeper filtering, live campaign membership, logout revocation, and unauthenticated denial |
| Distributed identity security | PASS; two-instance Redis rate limit, dummy Argon2 path, bounded password work, persistence restart, membership guards |
| Remote PostgreSQL TLS | PASS; trusted CA accepted and untrusted chain rejected |
| JetStream/Redis projection integration | PASS; outbox waits for JetStream ack and Redis remains a versioned rebuildable read model |
| Durable workflow integration | PASS; state, leases, and reconstruction persisted in PostgreSQL |
| Wasmi plugin host | PASS; no WASI/privileged imports, digest mismatch and fuel exhaustion fail closed |
| PostgreSQL 18 backup/restore | PASS; custom archive restored into an independent fresh PG18 database, table counts matched, tampered manifest rejected |
| `cargo build --workspace --all-targets --release --locked` | PASS |
| Release `service-process-smoke.sh` on fresh databases | PASS; migration-runner first, five services plus Web ready/live, EOF survival, clean SIGTERM |
| PR #5 current-head GitHub Actions | PASS; head `baa3a0241cd1607b010acbf3d3a4206ff37fee84`, generated merge `909f66a6edeb808cb1a916b3914ac04082857edc`, runs `29434692578`, `29434692684`, and `29434692568` all completed successfully |
| Hosted workspace evidence artifact | PASS; `workspace-ci-909f66a6edeb808cb1a916b3914ac04082857edc-29434692568-1`, non-expired, 27,865 bytes |

## Static, policy, boundary, and frontend gates

- Full Clippy with `-D warnings`: PASS.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.
- Dependency directions and their negative tests: PASS.
- Product source boundaries and their negative tests: PASS.
- P02 external package bypass regression: 10/10 rejected.
- Workflow validator, Actionlint, ShellCheck, and shell syntax: PASS.
- OPA policy tests: 16/16 PASS.
- S00 PowerShell governance boundary and dev smoke parse: PASS with XDG cache/data/config redirected to `/tmp`.
- Node 24.17.0 / pnpm 11.9.0 repository tests, Web build, and Web behavior tests: PASS.

## Failures found rather than concealed

1. Source review found that the durable canonical adapter was only exercised directly, not from runtime formal commits. A persistence port and production composition source-to-sink test were added.
2. The first fresh full-suite run caught a stateless test canonical port accepting expected version `1` on an empty stream. The test port now maintains stream versions/idempotency and the entire workspace passed from a second fresh database set.
3. The first release process smoke exposed concurrent API/migration-runner startup DDL/recovery failures. PostgreSQL migration locks and migration-first service ordering were added; a new-database smoke then passed.
4. An initial PowerShell run failed before script execution because its default cache/data locations were read-only. The same scripts passed after redirecting only XDG runtime state to `/tmp`.
5. A ShellCheck invocation initially used the wrong binary path and exited `127`; the actual checksum-verified binary was run afterward and returned `0`.

No failed or skipped attempt above is reported as a PASS. Hosted CI was checked only after the normal implementation commit was pushed; the PR-generated merge SHA is recorded separately from the implementation head rather than being misrepresented as the same commit.
