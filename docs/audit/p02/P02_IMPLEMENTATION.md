# P02 Complete Repair Implementation

Base: `0f52f27493f6737d0a82974f0f402520ad4b23d9`

## Reproduced findings and repairs

| Finding | Implemented repair | Verification | Local status |
|---|---|---|---|
| Per-connection EOF could terminate a service accept loop | Keep listener ownership outside connection handlers and treat EOF/malformed input as a connection-local error | Release process smoke sends EOF-only connections to all five services and rechecks liveness | Closed |
| Formal commits accepted caller-owned authority/audit state | Runtime and agent stores own `FormalCommitAuthorizer`; identity resolves the locked Authority Contract; real OpenFGA and OPA evidence is required | Identity/authority negative tests and real-policy integration | Closed |
| Runtime/agent formal events stopped at an in-memory store | Add `CanonicalCommitPort`; persist the complete authorized batch before publishing candidate in-memory events; inject `PostgresCanonicalCommitPort` from the production API composition root | Production composition source-to-sink test plus restart/version-conflict negative | Closed |
| Event, audit, outbox, and recovery were not one durable protocol | Add a PostgreSQL atomic primary transaction, idempotent formal-commit marker, HMAC-bound event batch, outbox rows, and crash-gap recovery | SQL fault injection, retry, recovery, and integrity verification | Closed |
| Audit head could be re-genesis after deleting local files | Add an independently addressed PostgreSQL witness with PREPARED/COMMITTED/ABORTED HMAC chain and require primary/witness endpoint separation | Endpoint-separation negative and primary/witness integrity checks | Closed |
| Replay scope was publicly constructible and transport-unbound | Make `ReplayAuthorization` identity-minted, live-session/campaign-bound, and add authenticated canonical replay route with stored visibility reconstruction | API replay integration covers player, keeper, cross-campaign, logout, and unauthenticated cases | Closed |
| Login throttling and password work were process-local/unbounded | Add Redis Lua distributed windows and a bounded Argon2 concurrency gate; retain dummy Argon2 work for unknown users | Two-instance Redis limiter and bounded-work unit tests | Closed |
| Remote PostgreSQL lacked verified TLS | Reject remote plaintext endpoints and require a supplied CA with hostname verification for remote TLS | Dedicated TLS PostgreSQL accepts trusted CA and rejects the untrusted path | Closed |
| Workflow/backends/extensions were contract-only | Wire durable PostgreSQL workflow leases, SQLx canonical store, JetStream acked outbox, versioned Redis projection cache, PostgreSQL 18 backup/restore, and a fuel/memory-bounded Wasmi plugin host | Real integration tests and release process readiness | Closed |

## Additional integrity repairs

- Exact command payload binding prevents command A from committing decision B.
- Canonical policy audit rows carry real OpenFGA/OPA decision IDs and revisions; deny/unavailable paths remain fail-closed.
- OpenFGA no longer accepts caller-created contextual role tuples.
- Event integrity uses framed payload-inclusive hashing; recorded payload and integrity internals are not caller-mintable.
- Canonical HUMAN_KP membership cannot be moved, renamed, revoked, deleted, or downgraded in PostgreSQL.
- Formal dice use opaque server RNG output; callers cannot submit a formal outcome.
- Generic runtime/agent appenders and generic COC7 formal-event selection are unavailable across the package boundary.
- The canonical test port is stateful and enforces the same version/idempotency contract as PostgreSQL; it no longer grants arbitrary receipt ranges.
- Canonical migrations use PostgreSQL advisory locks, and process smoke starts migration-runner to readiness before traffic services.
- Package-boundary compile-fail coverage now rejects 10 bypass classes.

The serialization additions across workspace payload types are required for payload-bound event integrity. They do not introduce a new product API.
