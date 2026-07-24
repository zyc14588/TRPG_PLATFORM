ALTER TABLE privacy_deletion_jobs
    ADD COLUMN IF NOT EXISTS evidence_status TEXT NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS command_id TEXT,
    ADD COLUMN IF NOT EXISTS correlation_id TEXT,
    ADD COLUMN IF NOT EXISTS causation_id TEXT,
    ADD COLUMN IF NOT EXISTS canonical_event_type TEXT,
    ADD COLUMN IF NOT EXISTS canonical_event_sequence BIGINT,
    ADD COLUMN IF NOT EXISTS canonical_event_integrity_hash TEXT;

ALTER TABLE privacy_deletion_jobs
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_evidence_status_check,
    ADD CONSTRAINT privacy_deletion_jobs_evidence_status_check
        CHECK (evidence_status IN ('pending', 'confirmed')),
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
              AND canonical_event_integrity_hash ~ '^sha256:[0-9a-f]{64}$'))
    ) NOT VALID;

-- Rows created before the evidence handshake are intentionally left pending.
-- They are retained for audit but can never enter an executable state.
CREATE OR REPLACE FUNCTION enforce_privacy_deletion_job_evidence()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
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

DROP TRIGGER IF EXISTS privacy_deletion_job_evidence_guard ON privacy_deletion_jobs;
CREATE TRIGGER privacy_deletion_job_evidence_guard
BEFORE INSERT OR UPDATE ON privacy_deletion_jobs
FOR EACH ROW EXECUTE FUNCTION enforce_privacy_deletion_job_evidence();

CREATE OR REPLACE FUNCTION enforce_privacy_deletion_target_transition()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.job_id, NEW.target) IS DISTINCT FROM ROW(OLD.job_id, OLD.target) THEN
            RAISE EXCEPTION 'deletion target identity is immutable';
        END IF;
        IF OLD.status = 'verified' AND NEW.status <> 'verified' THEN
            RAISE EXCEPTION 'verified deletion target cannot regress';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS privacy_deletion_target_transition_guard
    ON privacy_deletion_job_targets;
CREATE TRIGGER privacy_deletion_target_transition_guard
BEFORE UPDATE ON privacy_deletion_job_targets
FOR EACH ROW EXECUTE FUNCTION enforce_privacy_deletion_target_transition();

CREATE TABLE IF NOT EXISTS privacy_subject_deletion_fences (
    subject_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES privacy_deletion_jobs(job_id),
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
