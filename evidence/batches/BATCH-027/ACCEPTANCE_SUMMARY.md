# BATCH-027 Acceptance Summary

Batch: BATCH-027-06-data-eventing - Strict Governance Final
Stage: S03 data-eventing persistence
Last verification rerun: 2026-07-07

## Implemented Scope

- Added 10 current-safe documentation-or-traceability records under docs/codex/06-data-eventing during the earlier B027 pass.
- Repaired CODEX-0661 by adding crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs.rs and crates/trpg-data-eventing/tests/adr_0002_event_sourcing_cqrs_contract_tests.rs.
- Repaired CODEX-0663 by adding crates/trpg-data-eventing/src/adr_0005_postgres_pgvector.rs and crates/trpg-data-eventing/tests/adr_0005_postgres_pgvector_contract_tests.rs.
- Registered both new current-safe modules in crates/trpg-data-eventing/src/lib.rs.
- Preserved 13 supplemental prompts as prompt-only merge inputs; no extra implementation output was created for them.

## Changed Files

- crates/trpg-data-eventing/src/lib.rs
- crates/trpg-data-eventing/src/adr_0002_event_sourcing_cqrs.rs
- crates/trpg-data-eventing/tests/adr_0002_event_sourcing_cqrs_contract_tests.rs
- crates/trpg-data-eventing/src/adr_0005_postgres_pgvector.rs
- crates/trpg-data-eventing/tests/adr_0005_postgres_pgvector_contract_tests.rs
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_readme.md
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_06_data_eventing_snapshot_strategy.md
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_07_api_realtime_contracts_nats_subject_contracts.md
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_cache_redis.md
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_event_bus_nats.md
- docs/codex/06-data-eventing/source_processing_record_docs_implementation_90_traceability_source_breakdown_platform_persistence_postgresql.md
- docs/codex/06-data-eventing/source_processing_record_docs_platform_cache_redis.md
- docs/codex/06-data-eventing/source_processing_record_docs_platform_event_bus_nats.md
- docs/codex/06-data-eventing/source_processing_record_docs_platform_persistence_postgresql.md
- docs/codex/06-data-eventing/source_processing_record_docs_schemas_event_json_schema.md
- evidence/batches/BATCH-027/WORK_PLAN.md
- evidence/batches/BATCH-027/TEST_RESULTS.md
- evidence/batches/BATCH-027/ACCEPTANCE_SUMMARY.md

## Verification

- Strict acceptance status: FAIL, because live SQLx migration run/revert was rerun and still could not be completed without `DATABASE_URL`, Docker/Podman, WSL PostgreSQL/Docker fallback, or local PostgreSQL runtime.
- B027 traceability file existence self-check: pass.
- B027 Prompt ID coverage in WORK_PLAN.md: pass for all 25 rows.
- B027 current-safe traceability token/path scan: pass.
- cargo fmt --all -- --check: pass.
- cargo test -p trpg-data-eventing --test adr_0002_event_sourcing_cqrs_contract_tests: pass, 3 tests.
- cargo test -p trpg-data-eventing --test adr_0005_postgres_pgvector_contract_tests: pass, 3 tests.
- cargo test -p trpg-data-eventing --all-features: pass.
- cargo test -p trpg-data-eventing --test event_store_contract: pass.
- cargo test -p trpg-data-eventing --test projection_replay: pass.
- cargo clippy -p trpg-data-eventing --all-targets --all-features -- -D warnings: pass.
- cargo test --workspace --all-features: pass.
- cargo clippy --workspace --all-targets --all-features -- -D warnings: pass.
- Fixture presence and sensitive visibility label inspection: pass as static/test-backed inspection.
- sqlx migrate run/revert: not passed on the 2026-07-07 rerun because `DATABASE_URL` is unset, Docker/Podman is unavailable, no WSL distribution is listed, and no local PostgreSQL runtime/service was found.
- pnpm: available, but no package.json/pnpm-lock entrypoint exists in this repository snapshot.

## Governance Evidence

- CODEX-0661 and CODEX-0663 use current-safe module/output names from B027 and the current-safe maps.
- No historical V3/V4/V5/V6 current semantics were introduced into Rust module names, test names, event names, NATS subjects, metrics, or output paths.
- Added event appends use the existing governed data-eventing append path rather than direct database writes.
- Authority Contract checks, authority mode immutability assumptions, expected-version checks, idempotency checks, visibility labels, fact provenance, projection checkpoints, and replay eligibility are covered by the new target tests.
- No Agent Runtime / Provider Adapter bypass or direct LLM/provider call path was added.
- No formal game state bypass of State Service / Event Log semantics was added; Event Store remains canonical and Projection/RAG/Snapshot outputs remain rebuildable read models.
- No B027 supplemental prompt created implementation output beyond its owning primary prompt boundary.
- Sensitive fixture labels were inspected and remain in fixture/test/policy contexts; player-visible leakage is covered by existing redaction tests.

## Unresolved Environment Risk

- Live SQLx migration run/revert remains blocked in this shell. `sqlx-cli 0.9.0` is installed, but `DATABASE_URL` is not set; Docker/Podman is not installed or running; no WSL distribution is listed; and no `psql`/`postgres`/`pg_ctl`/`initdb` command, PostgreSQL install path, or PostgreSQL service was found.

## Next Batch Handoff

- Do not start BATCH-028 from this evidence.
- If continuing S03, keep using current-safe output names only.
- Do not treat source-archive or historical path/hash fragments as current module, migration, event, metric, workflow, test, or output names.
- Keep all future formal data-eventing writes behind Command -> Workflow -> Decision -> Event Store -> Projection, with visibility and fact provenance preserved.
