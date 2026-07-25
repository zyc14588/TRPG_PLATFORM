# S03 Traceability

Date: `2026-07-21`
Scope: B024–B028, revalidated under audit label P04

## Batch inventory

| Batch | Primary | Supplemental | Docs/Trace | Total |
| --- | ---: | ---: | ---: | ---: |
| B024 | 15 | 9 | 1 | 25 |
| B025 | 11 | 14 | 0 | 25 |
| B026 | 9 | 4 | 12 | 25 |
| B027 | 2 | 13 | 10 | 25 |
| B028 | 1 | 6 | 0 | 7 |
| Total | 38 | 46 | 23 | 107 |

P04 is an audit label, not a normalized prompt or batch. The authoritative mapping and the seven
review findings are recorded in `docs/audit/p04/P04_FINDINGS_TRACEABILITY.md`.

## Primary implementation ownership used by this repair

| Prompt | Canonical ID | Owned surface |
| --- | --- | --- |
| P0006 | `CODEX-0062-06-DATA-EVENTING-09d943908d` | Outbox/Projection workers |
| P0007 | `CODEX-0063-06-DATA-EVENTING-f6f824261f` | Persistence migrations |
| P0020 | `CODEX-0595-06-DATA-EVENTING-f8fc21553c` | SQLx Event Store/Outbox/Projection |
| P0030 | `CODEX-0605-06-DATA-EVENTING-7aa50c4023` | PostgreSQL/SQLx/pgvector |
| P0031 | `CODEX-0606-06-DATA-EVENTING-96df5cfdb1` | SQLx migrations |
| P0050 | `CODEX-0625-06-DATA-EVENTING-181b11b4cd` | SQLx migration contract |
| Ops P0001 | `CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29` | Backup/restore runbook and its independent integration target |

`apps/agent-worker`, `crates/trpg-identity`, `apps/api-server`, and `scripts/ci` do not have exact
output-owner rows for these files in the normalized maps. Worker composition and replay authorization
are established P02 repair surfaces (#7 and #3 respectively); the current changes close P04 regressions
on those surfaces without inventing a new owner.

## Requirement-to-proof binding

| Requirement / fixture | Executable proof |
| --- | --- |
| Event Store is canon | append-only/version/idempotency tests plus database mutation guards |
| Atomic event/outbox/audit commit | `canonical_commit_postgres` and fault injection |
| JetStream derives from Event Store | real envelope/ACK test against NATS 2.10.27 |
| Projection is rebuildable | damage, missing checkpoint, forged hash, restart and embedded backup—destroy—restore tests in `trpg-data-eventing` |
| Canonical backup is independently restorable | separate `trpg-ops::postgres_backup_restore_integration` custom-archive restore and tamper rejection |
| RAG is a visibility-filtered read model | real pgvector cosine query, source binding, concurrent generation lock and relabel rejection |
| Canonical JSON is cross-engine stable | serde_json/PostgreSQL equality for nested Unicode keys under C collation |
| Private group access is authoritative | persisted campaign group membership, exact subject lookup, live revoke and self-grant denial |
| Worker readiness cannot be forged | pending/stale/stopped/panic/cycle-error health tests |
| Migration policy | forward apply, repeated no-op, old-schema upgrade, backup/restore and application rollback retaining schema |
| Evidence cannot be summary-only | environment/service/output/JUnit binding and live-context mutation tests |

The 66-test `trpg-data-eventing` command (including its embedded recovery drill) and the 1-test `trpg-ops`
backup/restore command are intentionally reported separately. Their package boundary and distinct assertions
must not be collapsed into one test count.

Current migration `20260717000100` and identity group migration `20260721000100` are forward-only.
Application rollback retains schema and Event Store history. Historical run/revert/run transcripts for
`20260705000100` are provenance for that explicitly reversible base migration only.
