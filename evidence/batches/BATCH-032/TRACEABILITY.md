# BATCH-032 Traceability

## Changed Implementation Files

- `crates/trpg-platform/src/lib.rs`
- `crates/trpg-platform/src/api_contracts_impl.rs`
- `crates/trpg-platform/src/deployment_ops_impl.rs`
- `crates/trpg-platform/src/observability_impl.rs`
- `crates/trpg-platform/src/plugin_sdk_impl.rs`
- `crates/trpg-platform/src/policy_authz_impl.rs`
- `crates/trpg-platform/src/reliability_performance_impl.rs`
- `crates/trpg-platform/src/security_privacy_copyrightmpl.rs`

## Changed Test Files

- `crates/trpg-platform/tests/api_contracts_impl_contract_tests.rs`
- `crates/trpg-platform/tests/deployment_ops_impl_contract_tests.rs`
- `crates/trpg-platform/tests/observability_impl_contract_tests.rs`
- `crates/trpg-platform/tests/plugin_sdk_impl_contract_tests.rs`
- `crates/trpg-platform/tests/policy_authz_impl_contract_tests.rs`
- `crates/trpg-platform/tests/reliability_performance_impl_contract_tests.rs`
- `crates/trpg-platform/tests/security_privacy_copyrightmpl_contract_tests.rs`
- `crates/trpg-platform/tests/s09_fixture_acceptance_contract_tests.rs`

## Changed Stage Smoke Files

- `compose.yml`
- `docker-compose.ci.yml`
- `evidence/stages/S09/docker-compose-config.txt`
- `evidence/stages/S09/docker-compose-smoke.txt`
- `evidence/stages/S09/health-checks.json`

## Changed Traceability Files

- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_local_dev_environment.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_background_workers.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_observability.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_deployment_ops.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_readme.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_object_storage.md`
- `docs/codex/08-platform-infrastructure/source_processing_record_docs_implementation_08_platform_infrastructure_performance_budget.md`

## Prompt To Output Mapping

- `CODEX-0752` -> `api_contracts_impl`: command contract registered event, current-safe metrics, authority/visibility/provenance tests.
- `CODEX-0753` -> `deployment_ops_impl`: deployment operation applied event, provider boundary reuse, authority/visibility/provenance tests.
- `CODEX-0754` -> `observability_impl`: platform observation recorded event, restricted detail redaction, metric-prefix tests.
- `CODEX-0755` -> `plugin_sdk_impl`: plugin tool grant registered event, direct-write grant denial, authority/visibility/provenance tests.
- `CODEX-0756` -> `policy_authz_impl`: authorization granted event, deny decision fail-closed behavior, authority/visibility/provenance tests.
- `CODEX-0757` -> `reliability_performance_impl`: guard recorded event, capped retry delay reuse, authority/visibility/provenance tests.
- `CODEX-0758` -> `security_privacy_copyrightmpl`: review event, restricted visibility export denial, redaction and provenance tests.
- `CODEX-0759` -> `source_processing_record_docs_implementation_08_platform_infrastructure_local_dev_environment.md`: documentation-or-traceability output generated.
- `CODEX-0760` -> `source_processing_record_docs_implementation_08_platform_infrastructure_background_workers.md`: documentation-or-traceability output generated.
- `CODEX-0761` -> `source_processing_record_docs_implementation_08_platform_infrastructure_observability.md`: documentation-or-traceability output generated.
- `CODEX-0762` -> `source_processing_record_docs_implementation_08_platform_infrastructure_deployment_ops.md`: documentation-or-traceability output generated.
- `CODEX-0763` -> `source_processing_record_docs_implementation_08_platform_infrastructure_readme.md`: documentation-or-traceability output generated.
- `CODEX-0764` -> `source_processing_record_docs_implementation_08_platform_infrastructure_object_storage.md`: documentation-or-traceability output generated.
- `CODEX-0765` -> `source_processing_record_docs_implementation_08_platform_infrastructure_performance_budget.md`: documentation-or-traceability output generated.

Supplemental prompts `CODEX-0741` through `CODEX-0751` were treated as merged constraints only. Traceability prompts `CODEX-0759` through `CODEX-0765` produced Markdown traceability outputs only and did not expand Rust implementation ownership.

## S09 Runtime Evidence

- `docker compose config`: PASS, config output captured in `evidence/stages/S09/docker-compose-config.txt`.
- `docker compose up -d --build --force-recreate`: PASS, core services started.
- `scripts/dev/smoke.ps1`: PASS, `health-checks.json` reports API `healthy`.
- `curl.exe -f http://localhost:8080/healthz`: PASS, returned the API health payload.
- `docker compose ps`: PASS, web, api, realtime, agent-worker, postgres, redis, nats, minio, reverse-proxy, and admin are healthy.
- `init_wizard_completes`: NON_CODE_REASON, because B032 has no executable admin init wizard endpoint/app; `InitialAdminCreated` and `ProviderConnectionTested` are not claimed as executed by this smoke.

The S09 strict fixture gate now parses the detailed fixture and runtime evidence. It fails on blocked compose evidence, missing service health rows, unhealthy health checks, or missing init wizard non-code reason, and passes against the current PASS evidence.

## Governance Coverage

- No business/platform module calls a model provider directly.
- All formal writes in the new modules enter through `CommandEnvelope` and append to a shared-kernel `EventStore`.
- Direct agent/business write paths remain blocked by shared-kernel validation or module policy.
- Authority mismatch (`HUMAN_KP` command with AI actor) returns `TrpgError::AuthorityViolation` and appends no event.
- Event replay respects visibility labels; public replay cannot see keeper/system private events.
- Event envelopes copy `FactProvenance`, correlation, causation, command id, idempotency key, authority mode/version, and visibility from the command envelope.
- Restricted observability/export surfaces redact or deny restricted detail.
- Current-safe module names are used for modules, tests, events, and metric module labels.
