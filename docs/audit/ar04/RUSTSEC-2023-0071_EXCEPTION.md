# AR04 RUSTSEC-2023-0071 time-bounded exception

## Decision

| Field | Value |
| --- | --- |
| Finding | `F-013` |
| Advisory | `RUSTSEC-2023-0071` |
| Locked package | `rsa 0.9.7` |
| Decision | Approved, time-bounded exception for an unreachable lockfile-only package |
| Owner | `zyc14588` (repository operator) |
| Approved by | `zyc14588` (repository operator) |
| Effective date | 2026-07-30 |
| Expiration | 2026-09-30, 23:59:59 UTC |
| Configuration | `.cargo/audit.toml` |

Approval is limited to this advisory and this unreachable dependency state. It
does not approve a MySQL production path, an RSA private-key operation, another
advisory, or release readiness. The exception must be removed or renewed with
new evidence before it expires.

## Reachability evidence

The vulnerable sink is an RSA private-key operation observable by a remote
attacker. Current production manifests enable SQLx with `default-features =
false` and PostgreSQL features only. No manifest or Rust source enables a MySQL
driver, imports `sqlx::mysql`, or performs an RSA operation.

`rsa 0.9.7` remains in `Cargo.lock` only through inactive optional package
metadata:

```text
sqlx-macros-core 0.8.6
└── optional sqlx-mysql 0.8.6
    └── rsa 0.9.7
```

The following checks were run from the repository root on 2026-07-30:

```bash
cargo tree -i quick-xml --workspace --all-features
# package specification did not match any package: quick-xml is absent

cargo tree -i rsa@0.9.7 --workspace --all-features
# exit 0, no inverse dependency path

cargo tree -i rsa@0.9.7 --workspace --all-features --target all
# exit 0, no inverse dependency path for any target

rg -n --glob 'Cargo.toml' '(mysql|rsa)' . --glob '!target/**'
rg -n --glob '*.rs' \
  '(sqlx::mysql|MySql|mysql://|rsa::|RsaPrivateKey|RsaPublicKey)' \
  crates apps --glob '!target/**'
# both searches return no matches
```

`quick-xml` and its former `rust-s3 -> aws-creds` production path are already
absent on the AR04 starting HEAD. `cargo audit --no-fetch` therefore reports no
`quick-xml` advisory.

## Why the package is not updated or removed in this batch

The RustSec advisory has no fixed `rsa` release. The package is not in the
compiled production graph, so replacing cryptography would not change runtime
behavior. Removing it from lockfile metadata would require replacing SQLx
derive/migration macro use across production persistence code or taking a
breaking SQLx framework upgrade. Both exceed AR04's minimal dependency-change
boundary and would create compatibility risk without removing a reachable
sink.

## Compensating controls

- PostgreSQL remains the only enabled SQLx driver; SQLx default features remain
  disabled.
- `scripts/ci/check_rustsec_exceptions.py` rejects an expired or malformed
  exception and fails if `rsa 0.9.7` becomes reachable under any workspace
  feature or target.
- The exception check is part of `scripts/ci/test-all.sh`, so adding a MySQL/RSA
  path cannot silently inherit this acceptance.
- `cargo audit` ignores only `RUSTSEC-2023-0071`; every other advisory remains
  blocking.

## Review and revocation

The owner must review this exception no later than 2026-09-30, and earlier if
SQLx changes, a MySQL requirement is proposed, RustSec publishes a fixed
version, or `rsa` disappears from the lockfile.

Review commands:

```bash
cargo audit --no-fetch
cargo tree -i quick-xml --workspace --all-features
cargo tree -i rsa@0.9.7 --workspace --all-features
cargo tree -i rsa@0.9.7 --workspace --all-features --target all
python3 scripts/ci/check_rustsec_exceptions.py
git diff -- Cargo.lock
```

Remove the `RUSTSEC-2023-0071` ignore entry and this record as soon as the
package leaves `Cargo.lock` or a compatible fixed dependency path is available.
No migration or serialized product contract changes are authorized by this
exception.
