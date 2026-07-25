ALTER TABLE event_store
    ADD COLUMN IF NOT EXISTS deletion_job_id TEXT,
    ADD COLUMN IF NOT EXISTS deletion_subject_id TEXT,
    ADD COLUMN IF NOT EXISTS deletion_requested_by TEXT,
    ADD COLUMN IF NOT EXISTS deletion_retention_policy TEXT;

ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_deletion_request_fields_check,
    ADD CONSTRAINT event_store_deletion_request_fields_check CHECK (
        (event_type = 'platform.security_privacy_copyright.data_deletion_requested'
         AND btrim(deletion_job_id) <> ''
         AND btrim(deletion_subject_id) <> ''
         AND btrim(deletion_requested_by) <> ''
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

ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_provenance_kind_valid,
    ADD CONSTRAINT event_store_provenance_kind_valid CHECK (
        fact_provenance_kind IN (
            'user_statement', 'human_keeper_statement', 'rules_engine_decision',
            'tool_result', 'agent_proposal', 'imported_source', 'system_fixture'
        )
    ) NOT VALID;

ALTER TABLE privacy_deletion_jobs
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_evidence_binding_check,
    ADD CONSTRAINT privacy_deletion_jobs_evidence_binding_check CHECK (
        command_id IS NOT NULL
        AND correlation_id IS NOT NULL
        AND causation_id IS NOT NULL
        AND canonical_event_type IS NOT NULL
        AND ((evidence_status = 'pending'
              AND canonical_event_sequence IS NULL
              AND canonical_event_integrity_hash IS NULL)
          OR (evidence_status = 'confirmed'
              AND canonical_event_sequence > 0
              AND canonical_event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'))
    ) NOT VALID;

CREATE OR REPLACE FUNCTION enforce_privacy_deletion_job_evidence()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    canonical public.event_store%ROWTYPE;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.job_id, NEW.subject_id, NEW.requested_by, NEW.retention_policy,
               NEW.command_id, NEW.correlation_id, NEW.causation_id,
               NEW.canonical_event_type, NEW.created_at)
           IS DISTINCT FROM
           ROW(OLD.job_id, OLD.subject_id, OLD.requested_by, OLD.retention_policy,
               OLD.command_id, OLD.correlation_id, OLD.causation_id,
               OLD.canonical_event_type, OLD.created_at) THEN
            RAISE EXCEPTION 'deletion job identity and expected event evidence are immutable';
        END IF;
        IF OLD.evidence_status = 'confirmed'
           AND ROW(NEW.evidence_status, NEW.canonical_event_sequence,
                   NEW.canonical_event_integrity_hash)
               IS DISTINCT FROM
               ROW(OLD.evidence_status, OLD.canonical_event_sequence,
                   OLD.canonical_event_integrity_hash) THEN
            RAISE EXCEPTION 'confirmed deletion event evidence is immutable';
        END IF;
    END IF;

    IF NEW.evidence_status = 'confirmed' THEN
        SELECT * INTO canonical
          FROM public.event_store
         WHERE sequence = NEW.canonical_event_sequence;
        IF NOT FOUND
           OR canonical.event_type IS DISTINCT FROM NEW.canonical_event_type
           OR canonical.event_type IS DISTINCT FROM
              'platform.security_privacy_copyright.data_deletion_requested'
           OR canonical.command_id IS DISTINCT FROM NEW.command_id
           OR canonical.correlation_id IS DISTINCT FROM NEW.correlation_id
           OR canonical.causation_id IS DISTINCT FROM NEW.causation_id
           OR canonical.event_integrity_hash IS DISTINCT FROM
              NEW.canonical_event_integrity_hash
           OR canonical.integrity_status IS DISTINCT FROM 'verified_hmac'
           OR canonical.request_hash_source IS DISTINCT FROM 'formal_commit'
           OR canonical.deletion_job_id IS DISTINCT FROM NEW.job_id
           OR canonical.deletion_subject_id IS DISTINCT FROM NEW.subject_id
           OR canonical.deletion_requested_by IS DISTINCT FROM NEW.requested_by
           OR canonical.deletion_retention_policy IS DISTINCT FROM NEW.retention_policy
           OR canonical.data_subject_id IS DISTINCT FROM NEW.subject_id
           OR canonical.fact_provenance_kind IS DISTINCT FROM 'user_statement'
           OR canonical.fact_recorded_by IS DISTINCT FROM NEW.requested_by
           OR NOT (canonical.payload_json ? 'protected_payload') THEN
            RAISE EXCEPTION 'deletion evidence does not match the canonical request event';
        END IF;
    END IF;

    IF NEW.status IN ('running', 'verifying', 'completed')
       AND NEW.evidence_status <> 'confirmed' THEN
        RAISE EXCEPTION 'deletion job cannot execute without canonical event evidence';
    END IF;
    IF NEW.status = 'completed' AND EXISTS (
        SELECT 1
          FROM public.privacy_deletion_job_targets AS target
         WHERE target.job_id = NEW.job_id
           AND target.status <> 'verified'
    ) THEN
        RAISE EXCEPTION 'deletion job cannot complete before every target is verified';
    END IF;
    RETURN NEW;
END;
$$;
