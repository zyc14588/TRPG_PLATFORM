# S03 Acceptance Evidence

Date: `2026-07-21`
Conclusion: `LOCAL_TECHNICAL_PASS`; Hosted CI `NOT_RUN`; product release `NO`

## Acceptance closure

- Event Store remains the sole canon. Projection, Redis cache and RAG are derived, damageable and
  deterministically rebuildable without rewriting canonical history.
- Formal commit persists event, outbox, audit and completion state atomically; canonical witness verification
  uses a separate PostgreSQL 18.4 endpoint, while backup/restore uses a distinct empty target database.
- JetStream receives only Event Store-derived envelopes and verifies all configured NATS 2.10 fields before
  readiness. Redis remains a versioned read model.
- RAG uses a real pgvector table/query, inherits source Visibility/Fact Provenance/copyright metadata, filters
  in SQL, and shares an advisory lock between repository replacement and raw inserts.
- PostgreSQL recomputes Projection hash v3 and uses bytewise C collation for canonical JSON keys. Nested
  Unicode objects match Rust `serde_json` bytes exactly.
- `private_to_group` requires an active, persisted `(campaign, group, subject)` membership minted by Identity;
  player self-grant, cross-group access and access after revoke fail closed.
- Agent-worker readiness requires a completed fresh background cycle and fails on stale, stopped or panicked
  loops; configuration-only health cannot produce PASS.
- Machine evidence schema p00-4 binds hashed environment inputs, executable service-version probes, exact
  stdout/stderr bytes, Cargo/JUnit cases and complete worktree state.

## Migration and schema values

| Migration | SHA-256 | SQLx SHA-384 |
| --- | --- | --- |
| `20260717000100` | `730da5a67fdb64e60898dee3ad911e0758261026d64afe5d414078e02cd69024` | `d67991333d4d9e06b5c1c51a9f3c17855bddfbb64558873f4821e7338700981a98fe78f1c05441244c95e5010e156adc` |
| `20260721000100` | `bdf7831cbd46da473502f35b791f73a7e58ab834bc334b1d8eb37d83fdbd9f2f` | `89f3a6b7554f9ab392ac0d5f9c06e3d886de9e607415e71fe34585439fee3c82c3580aa8494ffae153533683546163e9` |

Catalog signatures: constraint `74abd245d298fcbc86ffaef6ab33a216`, trigger
`3d0d5cdb9fbaa52551125b4a0c935bcb`, trigger/function
`55817f97d554378f6cc4bd789115ebf5`.

## Evidence boundary

Tracked files under `evidence/` and `docs/audit/p04/` are human-authored summaries. Machine evidence p00-4
is outside the repository at `/tmp/p04-final-evidence-20260721/`. Data/Eventing and PostgreSQL
backup/restore have separate manifests, raw logs, JUnit and SARIF because they are separate Cargo package
targets. The generator re-executed both commands; `verify_evidence_schema.py` validates pinned tool versions,
bound environment digests, executable service probes, complete worktree state and report hashes.

The initial report-state compile failure and the earlier false attribution of the `trpg-ops` backup target to
the 66-test `trpg-data-eventing` command are retained in `docs/audit/p04/P04_TEST_RESULTS.md`. Clean-worktree
repository truth remains blocked by the intentionally uncommitted repair, so this is not a release PASS.

Forward-only canon/security migrations are never destructively reverted for acceptance. Rollback means
retaining schema/history, reverting or disabling the application worker, restoring backup where required,
applying forward migrations and replaying derived models.
