# P01 Final Status

```text
BATCH_STATUS = COMPLETE
BATCH_ID = P01
BASE_HEAD = c86da75dbfa1f6ac0cf6ab7764e08a363fc91396
VERIFIED_AT_UTC = 2026-07-14T09:43:25Z
P02_EXECUTED = NO
```

This record covers only P01 product entry points, module boundaries, health, Event/Error contracts, fixture-factory isolation, and build/start smoke. It does not claim a complete business vertical slice or product release readiness.

## Delivered boundaries

- Five release binaries exist for API, Realtime, Agent Worker, Admin, and Migration Runner.
- Every process owns a runtime loop. Each readiness request performs a bounded heartbeat and re-runs the process-specific initialization check; a stopped loop produces a failing component check.
- The Web application builds to `apps/web/dist`, starts through its own preview server, and performs live/ready checks for all five processes.
- Cargo dependency policy resolves renamed, workspace-inherited, build, and target-specific dependencies.
- Product-source policy rejects construction metadata, fixed fixture factories, empty Service/Repository shells, byte-identical modules, and unlisted public `*_impl` compatibility modules.
- Canonical events have one typed registry that generates schema and OpenAPI metadata. Ruleset event construction resolves that registry before append, and its schema version/id are mandatory.
- Domain, shared-kernel, Agent, and Extension errors map through `WireErrorCode`; all registered codes are tested for uniqueness and SCREAMING_SNAKE_CASE.

## Command evidence

All successful commands below returned exit code `0`.

| Command | Result |
|---|---|
| `RUSTUP_TOOLCHAIN=stable cargo build --workspace --all-targets --release --locked` | Five product binaries and all release targets built |
| `RUSTUP_TOOLCHAIN=stable cargo test -p trpg-contracts --all-targets --locked` | 7 contract tests passed |
| `RUSTUP_TOOLCHAIN=stable cargo test --workspace --all-features --locked` | All workspace, integration, and doc tests passed |
| `pnpm --filter ./apps/web... build` | Web dist built with five configured services |
| `pnpm --filter ./apps/web... test` | Five ready responses plus degraded and unavailable behavior passed |
| `./scripts/ci/service-process-smoke.sh` | Five services and Web started, responded, and exited on SIGTERM |
| `python3 scripts/ci/check_dependency_directions.py` | Dependency directions passed |
| `python3 scripts/ci/test_dependency_directions.py` | 3 negative gate tests passed |
| `python3 scripts/ci/check_product_boundaries.py` | Product source boundaries passed |
| `python3 scripts/ci/test_product_boundaries.py` | 5 negative gate tests passed |
| `RUSTUP_TOOLCHAIN=stable python3 scripts/ci/validate_workflows.py` | Workflow static validation passed |
| `RUSTUP_TOOLCHAIN=stable python3 scripts/ci/discover_tests.py --check` | 191 Rust test targets and no orphan fixture reported |
| `GIT_INDEX_FILE=<temporary-complete-tree-index> python3 scripts/ci/manifest.py --check` | Post-commit source manifest view verified |
| `python3 scripts/ci/verify_evidence_schema.py` | Evidence schema valid; historical PASS evidence rejected |
| `RUSTUP_TOOLCHAIN=stable cargo fmt --all -- --check` | Formatting passed |
| `git diff --check` | No whitespace errors |

The first `npm test` attempt failed because subprocesses selected the sandbox's implicit rustup path and Python 3.14.4. The required versions were then made explicit (`Rust 1.96.0`, `Python 3.14.6`, `Node 24.17.0`, `pnpm 11.9.0`), and the same 10 tests passed. The first Web behavior run inside the restricted sandbox failed with `listen EPERM`; the identical test passed after loopback binding was approved.

`python3 scripts/ci/repo_truth.py --check` remains non-zero because that gate intentionally requires a clean committed worktree. P01 is currently an uncommitted implementation tree; no claim is made that post-commit GitHub-hosted CI has run. The three source manifests were generated and checked through a temporary Git index containing the complete intended tree, without altering the user's real staging area.

## Negative evidence

- A shared-kernel dependency on API is rejected when direct, renamed with `package =`, placed in `build-dependencies`, or placed under a target-specific table.
- Batch/stage fixture metadata, a fixed governed command factory, an empty unit Service, a duplicate module, and an unlisted public compatibility module are rejected.
- Stopping a role runtime loop changes its readiness component to failure.
- `UnregisteredEvent` returns `EVENT_CONTRACT_UNKNOWN` and leaves the Event Store empty.
- `TRPG_API_SERVER_BIND=not-an-address` exits `1` with `SERVICE_CONFIGURATION_INVALID`.
- Event registry version drift and unknown names are rejected.

## Audit closure

| Audit ID | Status | Evidence |
|---|---|---|
| AUD-001 | CLOSED_PASS | Five binaries, Web dist, dynamic per-process readiness, process smoke |
| AUD-018 | CLOSED_PASS | Production metadata/factory gate, domain registries renamed, behavior-bearing service check |
| AUD-026 | CLOSED_PASS | Governed fixture construction exists only in `trpg-test-support` |
| AUD-039 | CLOSED_PASS | Exact duplicate detection, deleted duplicate modules, enforced public compatibility inventory |
| AUD-040 | CLOSED_PASS | Typed `WireErrorCode::ALL` uniqueness/casing contract |
| AUD-044 | CLOSED_PASS | Exact registry fixture/schema/OpenAPI/projection contract and append rejection test |

## Risk and rollback

The main residual risk is that the large P01 dependency/API adaptation has not yet been committed and exercised by GitHub-hosted CI. Local required commands and full workspace regression are complete.

Rollback should use ordinary reverts grouped by service entry points, boundary gates, and contract registry. Preserve the canonical Event/Error registry when reverting unrelated service scaffolding; do not restore duplicated protocols or production fixture factories.
