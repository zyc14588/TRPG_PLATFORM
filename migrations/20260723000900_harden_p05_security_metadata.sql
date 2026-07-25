-- P05 follow-up: SQL CHECK treats NULL as satisfied. Require every
-- security-critical derivation/deletion field explicitly, constrain persisted
-- provider endpoints to credential-free URLs, and retain privacy evidence.

ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_rag_derivation_fields_check,
    ADD CONSTRAINT event_store_rag_derivation_fields_check CHECK (
        (event_type = 'RagChunkDerived'
         AND derived_source_event_sequence IS NOT NULL
         AND derived_source_event_sequence > 0
         AND derived_snapshot_id IS NOT NULL
         AND btrim(derived_snapshot_id) <> ''
         AND derived_chunk_id IS NOT NULL
         AND btrim(derived_chunk_id) <> ''
         AND derived_content_hash IS NOT NULL
         AND derived_content_hash ~ '^[0-9a-f]{64}$')
        OR
        (event_type <> 'RagChunkDerived'
         AND derived_source_event_sequence IS NULL
         AND derived_snapshot_id IS NULL
         AND derived_chunk_id IS NULL
         AND derived_content_hash IS NULL)
    ) NOT VALID,
    DROP CONSTRAINT IF EXISTS event_store_deletion_request_fields_check,
    ADD CONSTRAINT event_store_deletion_request_fields_check CHECK (
        (event_type = 'platform.security_privacy_copyright.data_deletion_requested'
         AND deletion_job_id IS NOT NULL
         AND btrim(deletion_job_id) <> ''
         AND deletion_subject_id IS NOT NULL
         AND btrim(deletion_subject_id) <> ''
         AND deletion_requested_by IS NOT NULL
         AND btrim(deletion_requested_by) <> ''
         AND deletion_retention_policy IS NOT NULL
         AND btrim(deletion_retention_policy) <> ''
         AND deletion_subject_id = data_subject_id
         AND deletion_subject_id = resource_id
         AND resource_type = 'data_subject'
         AND fact_provenance_kind = 'user_statement'
         AND fact_recorded_by = deletion_requested_by)
        OR
        (event_type <> 'platform.security_privacy_copyright.data_deletion_requested'
         AND deletion_job_id IS NULL
         AND deletion_subject_id IS NULL
         AND deletion_requested_by IS NULL
         AND deletion_retention_policy IS NULL)
    ) NOT VALID;

ALTER TABLE cloud_egress_route_snapshots
    DROP CONSTRAINT IF EXISTS cloud_egress_route_security_binding_valid,
    ADD CONSTRAINT cloud_egress_route_security_binding_valid CHECK (
        btrim(source_credential_id) <> ''
        AND source_credential_version > 0
        AND btrim(target_credential_id) <> ''
        AND target_credential_version > 0
        AND fallback_policy IN ('explicit_audited_only', 'historical_unavailable')
        AND privacy_boundary IN (
            'explicit_consent_no_silent_fallback', 'historical_unavailable'
        )
        AND (
            decision <> 'allow'
            OR (
                fallback_policy = 'historical_unavailable'
                AND privacy_boundary = 'historical_unavailable'
            )
            OR (
                consent_id IS NOT NULL
                AND consent_expires_at_unix_ms > created_at_unix_ms
                AND fallback_policy = 'explicit_audited_only'
                AND privacy_boundary = 'explicit_consent_no_silent_fallback'
            )
        )
    ),
    DROP CONSTRAINT IF EXISTS cloud_egress_route_endpoint_model_valid,
    ADD CONSTRAINT cloud_egress_route_endpoint_model_valid CHECK (
        source_endpoint ~ '^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]{1,5})?(/[^?#[:space:]]*)?$'
        AND target_endpoint ~ '^https://([A-Za-z0-9][A-Za-z0-9.-]*|\[[0-9A-Fa-f:]+\])(:[0-9]{1,5})?(/[^?#[:space:]]*)?$'
        AND btrim(model_id) <> ''
    );

ALTER TABLE cloud_egress_audit
    DROP CONSTRAINT IF EXISTS cloud_egress_audit_security_binding_valid,
    ADD CONSTRAINT cloud_egress_audit_security_binding_valid CHECK (
        btrim(source_credential_id) <> ''
        AND source_credential_version > 0
        AND btrim(target_credential_id) <> ''
        AND target_credential_version > 0
        AND fallback_policy IN ('explicit_audited_only', 'historical_unavailable')
        AND privacy_boundary IN (
            'explicit_consent_no_silent_fallback', 'historical_unavailable'
        )
        AND (
            decision <> 'allow'
            OR (
                fallback_policy = 'historical_unavailable'
                AND privacy_boundary = 'historical_unavailable'
            )
            OR (
                fallback_policy = 'explicit_audited_only'
                AND privacy_boundary = 'explicit_consent_no_silent_fallback'
            )
        )
    ),
    DROP CONSTRAINT IF EXISTS cloud_egress_audit_endpoint_model_valid,
    ADD CONSTRAINT cloud_egress_audit_endpoint_model_valid CHECK (
        source_endpoint ~ '^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]{1,5})?(/[^?#[:space:]]*)?$'
        AND target_endpoint ~ '^https://([A-Za-z0-9][A-Za-z0-9.-]*|\[[0-9A-Fa-f:]+\])(:[0-9]{1,5})?(/[^?#[:space:]]*)?$'
        AND btrim(source_provider) <> ''
        AND btrim(target_provider) <> ''
        AND btrim(model_id) <> ''
    );

CREATE OR REPLACE FUNCTION reject_privacy_evidence_removal()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    RAISE EXCEPTION 'privacy deletion evidence cannot be removed';
END;
$$;

CREATE TRIGGER privacy_deletion_jobs_delete_guard
BEFORE DELETE OR TRUNCATE ON privacy_deletion_jobs
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

CREATE TRIGGER privacy_deletion_targets_delete_guard
BEFORE DELETE OR TRUNCATE ON privacy_deletion_job_targets
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

CREATE TRIGGER privacy_deletion_fences_delete_guard
BEFORE DELETE OR TRUNCATE ON privacy_subject_deletion_fences
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

CREATE TRIGGER privacy_erased_subjects_mutation_guard
BEFORE UPDATE OR DELETE OR TRUNCATE ON privacy_erased_subjects
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

CREATE TRIGGER privacy_subject_keys_delete_guard
BEFORE DELETE OR TRUNCATE ON privacy_subject_keys
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

CREATE TRIGGER privacy_legal_holds_delete_guard
BEFORE DELETE OR TRUNCATE ON privacy_legal_holds
FOR EACH STATEMENT EXECUTE FUNCTION reject_privacy_evidence_removal();

DROP TRIGGER IF EXISTS cloud_egress_consents_no_truncate ON cloud_egress_consents;
DROP TRIGGER IF EXISTS cloud_egress_route_snapshots_no_truncate ON cloud_egress_route_snapshots;
DROP TRIGGER IF EXISTS cloud_egress_audit_no_truncate ON cloud_egress_audit;

REVOKE TRUNCATE ON privacy_deletion_jobs, privacy_deletion_job_targets,
    privacy_subject_deletion_fences, privacy_erased_subjects,
    privacy_subject_keys, privacy_legal_holds
    FROM PUBLIC;

-- A generic application credential previously inherited write access to every
-- table. Split authority by workload. These grants are conditional so the
-- repository migration remains usable in owner-only developer databases; the
-- production role bootstrap creates the isolated workload roles before
-- migrations run.
DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_application') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM trpg_application;
        REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM trpg_application;
        REVOKE ALL PRIVILEGES ON SCHEMA public FROM trpg_application;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM trpg_api_service;
        REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM trpg_api_service;
        GRANT USAGE ON SCHEMA public TO trpg_api_service;

        GRANT SELECT, INSERT, UPDATE, DELETE ON
            users, sessions, campaign_memberships, campaign_groups,
            campaign_group_memberships, authority_contracts
            TO trpg_api_service;
        GRANT SELECT, INSERT ON audit_log TO trpg_api_service;

        GRANT SELECT ON
            event_store, event_outbox, canonical_audit_log, formal_commits
            TO trpg_api_service;
        GRANT SELECT, INSERT ON
            privacy_deletion_jobs, privacy_deletion_job_targets,
            privacy_subject_deletion_fences
            TO trpg_api_service;
        GRANT SELECT ON
            privacy_erased_subjects, privacy_legal_holds, privacy_subject_keys,
            privacy_deletion_surface_records
            TO trpg_api_service;
        GRANT SELECT, INSERT, UPDATE ON cloud_egress_consents TO trpg_api_service;
        GRANT SELECT, INSERT ON
            cloud_egress_route_snapshots, cloud_egress_audit
            TO trpg_api_service;
        GRANT SELECT, INSERT ON cloud_egress_notices TO trpg_api_service;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM trpg_canonical_service;
        REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM trpg_canonical_service;
        GRANT USAGE ON SCHEMA public TO trpg_canonical_service;
        GRANT SELECT, INSERT ON
            event_store, event_outbox, canonical_audit_log, formal_commits
            TO trpg_canonical_service;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public
            TO trpg_canonical_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM trpg_worker_service;
        REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM trpg_worker_service;
        GRANT USAGE ON SCHEMA public TO trpg_worker_service;

        GRANT SELECT ON ALL TABLES IN SCHEMA public TO trpg_worker_service;
        GRANT UPDATE (
            delivery_status, published_at, retry_count, available_at,
            claimed_at, claim_owner, claim_token, locked_until, last_error,
            dead_lettered_at
        ) ON event_outbox TO trpg_worker_service;
        GRANT UPDATE (
            status, failure_code, evidence_status, canonical_event_sequence,
            canonical_event_integrity_hash, updated_at
        ) ON privacy_deletion_jobs TO trpg_worker_service;
        GRANT UPDATE (
            status, error_code, deleted_at, verified_at
        ) ON privacy_deletion_job_targets TO trpg_worker_service;
        GRANT INSERT, UPDATE ON privacy_subject_deletion_fences TO trpg_worker_service;
        GRANT INSERT ON
            privacy_erased_subjects, privacy_deletion_surface_records
            TO trpg_worker_service;
        GRANT UPDATE (wrapped_key, destroyed_at)
            ON privacy_subject_keys TO trpg_worker_service;

        GRANT UPDATE (login_normalized, password_hash, disabled_at)
            ON users TO trpg_worker_service;
        GRANT UPDATE (granted, updated_at)
            ON cloud_egress_consents TO trpg_worker_service;
        GRANT DELETE ON sessions, campaign_memberships, campaign_group_memberships
            TO trpg_worker_service;
        GRANT DELETE ON rag_snapshot_chunk TO trpg_worker_service;

        GRANT SELECT, INSERT, UPDATE ON workflow_instances TO trpg_worker_service;
        GRANT SELECT, INSERT ON workflow_transitions TO trpg_worker_service;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO trpg_worker_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_realtime_service') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM trpg_realtime_service;
        REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM trpg_realtime_service;
        GRANT USAGE ON SCHEMA public TO trpg_realtime_service;
        GRANT SELECT ON
            event_store, event_outbox, formal_commits, canonical_audit_log,
            canonical_event_projection, projection_checkpoint
            TO trpg_realtime_service;
    END IF;
END;
$least_privilege$;
