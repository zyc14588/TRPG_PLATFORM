# P05 repair baseline

Recorded: 2026-07-24 (Australia/Brisbane)

## Patch contract

- Scope: repair the defects recorded in the strict P05 review, without weakening a policy, deleting a test, or treating an unavailable integration as a pass.
- Security invariants: data-subject ownership is independent of visibility; a deletion job is executable only after a keyed and witnessed canonical event is verified; OpenFGA and OPA authorize the same action; durable consumers verify canonical integrity before using a row; production roles cannot forge canonical, privacy, consent, key, membership, or terminal-state records.
- Compatibility: historical provenance remains readable, but unverified historical rows cannot be promoted to current trusted evidence.
- Verification order: focused regressions, crate tests, live PostgreSQL/NATS/Redis/OpenFGA/OPA/object-store integrations, migrations and schema assertions, production Compose validation, then workspace fmt/clippy/test.

## Inherited repository state

The repair starts from commit `fb6e146612e4df66a508292245da6b995bbe64fb` with a pre-existing dirty worktree containing combined P03/P04/P05 work. A clean-HEAD P05 diff is therefore unavailable. This repair uses pre-edit SHA-256 values and final per-file byte comparisons; unrelated inherited changes must not be attributed to this repair.

Representative pre-edit hashes:

| File | SHA-256 |
|---|---|
| `crates/trpg-shared-kernel/src/shared_kernel.rs` | `0b3abeaa121ce3fecac4c222d7db42f2334686020204403efa1ff70440ed99d3` |
| `crates/trpg-security-governance/src/formal_commit_audit.rs` | `e39dd6033633ebf16f6abe632b71e70fdf1d8d025dabe32c4b9824158c79c965` |
| `crates/trpg-security-governance/src/policy_adapter.rs` | `fb018bda739c34bd34dde6f54274e537c333a5e714b1ebce69dd7ac1c4b7171d` |
| `crates/trpg-platform/src/security_privacy_copyright.rs` | `6e102d7f327e5946ac9f5701974a888b24d9aba47eaeea0611bc07353492d9d1` |
| `crates/trpg-data-eventing/src/event_store_sqlx_outbox_projection.rs` | `f25523c717ed82ca424aba9a16d07d321470d35cab4040639e9884e2ba0fcb4d` |
| `crates/trpg-data-eventing/src/cache_redis_impl.rs` | `01582db638273eae53daf4fdf7a86ee5fa5f41b4bbbe77894c4825d1bb2a3675` |
| `crates/trpg-privacy/src/lib.rs` | `cccef930a406991612791674fc5c4b1c14dc26b163398fa0166cb17a9c33f5ab` |
| `policy/openfga/security_governance.fga` | `fa8a10077d396acc983ee82e7042462d20681f9b488c3005ee059a44e9df9ff6` |
| `policy/opa/security_governance.rego` | `1d642e1767853c58ffb26dfd3d38ac5cb087154404712a504c2c0a08f3b3c5bf` |

## Honest failing baseline

| Command | Result |
|---|---|
| `cargo test -p trpg-security-governance --test derived_visibility_matrix -- --nocapture` | PASS, 4/4 |
| `cargo test -p trpg-domain-core --test fact_provenance -- --nocapture` | PASS, 6/6 |
| `cargo test -p trpg-privacy --test data_deletion_e2e -- --nocapture` | FAIL: 1 passed, 3 failed because `P05_DATABASE_URL` was not configured |
| Docker daemon access | BLOCKED: permission denied on `/var/run/docker.sock` |
| Docker Compose plugin | unavailable |

No row above may be upgraded to PASS without a new command log proving it.
