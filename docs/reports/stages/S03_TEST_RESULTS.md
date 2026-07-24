# S03 Test Results

Date: `2026-07-21`
Base HEAD: `fb6e146612e4df66a508292245da6b995bbe64fb`
Scope: uncommitted P04 repair worktree

## Current results

| Gate | Result |
| --- | --- |
| Data/Eventing all-features with PostgreSQL 18.4 primary/witness, NATS 2.10.27 JetStream and Redis 8.0.5 | PASS: 66 passed, 0 failed, 0 ignored |
| PostgreSQL custom-format backup/restore (`trpg-ops`) | PASS: 1 passed, 0 failed, 0 ignored; independent target DB and tamper rejection |
| SQLx ledger | PASS: 9 installed migrations; repeated `migrate run` was no-op |
| Schema assertions | PASS: `P04_SCHEMA_ASSERTION_OK` |
| Canonical commit repeatability | PASS: same guarded test command passed twice after each explicit dedicated-schema reset |
| Identity authoritative group persistence | PASS: 15 unit + 1 PostgreSQL + 1 Redis test; TLS behavior NOT_RUN |
| API canonical replay | PASS: 1/1, seven events including live group grant/revoke semantics |
| Agent worker liveness | PASS: 4/4 pending/stale/stopped/cycle failure tests |
| Durable workflow PostgreSQL | PASS: 1/1 across process reconstruction |
| Workspace check / Clippy / fmt / diff | PASS |
| Test discovery / dependency / product / workflow / evidence schema components | PASS |
| Repository truth tests | PASS: 11/11 under pinned Node 24/pnpm toolchain |
| Clean-worktree repository truth | EXPECTED BLOCK: repair worktree is uncommitted |
| CodeRabbit review | PRIOR_FIX_CYCLE_ONLY: initial 3 findings, then 0; not rerun after additional repair |
| Hosted CI | NOT_RUN |

The data/eventing total is:

```text
20+3+3+8+5+5+4+1+4+5+1+1+1+1+2+1+1 = 66
```

Real dependency cases include canonical atomic commit, JetStream/Redis ACK, migration upgrade,
leased Outbox, Projection resume, pgvector RAG snapshot, and an Event Store integration that performs
backup—destroy—restore followed by deterministic Projection rebuild. A separate `trpg-ops` runbook target
also generated a PostgreSQL 18.4 custom archive, restored it into an independent empty database, compared
four canonical table counts and rejected a tampered manifest.

## Failed attempts retained as failures

1. The first command copied from the prior P04 report did not compile because the RAG PostgreSQL decoder
   and contract test accessed private `RagSnapshotChunk` fields. The decoder now uses the validated
   persisted DTO and tests use public accessors; only the subsequent 66/0 run is PASS.
2. The first post-Unicode migration run failed because `assert-schema.sql` still contained the prior
   SHA-384; the second exposed the expected changed function signature. Both gates were updated from
   PostgreSQL output, then the upgrade test passed.
3. The first full rerun failed with `canonical_audit_hmac_mismatch` because the integration test silently
   assumed an empty database. The test now requires explicit reset authorization and exact local database
   names; two consecutive targeted runs and the full run passed.
4. One final evidence invocation removed required `P04_*` reset and recovery variables and correctly failed
   closed. Source inspection confirmed that `postgres_event_store_integration` embeds the backup/recovery
   drill; the exact P04 database and executable variables were restored before the complete rerun.
5. Evidence tests first failed 2/11 under Node 22 with no pnpm, then passed 11/11 under the repository-pinned
   Node 24.17.0/pnpm 11.9.0 environment. The first Redis version probe also failed integrity exit 86 until
   its real dynamic-library environment was bound and the evidence run repeated.
6. The TLS integration binary printed `ok` because its existing environment guard returned early; it is
   excluded from PASS. Real OpenFGA/OPA and hosted CI were not provisioned and are not claimed.

No current Event Store/security migration was reverted. The old 2026-07-05 run/revert/run result is
historical provenance for `20260705000100`, not evidence that later forward-only migrations support down.
