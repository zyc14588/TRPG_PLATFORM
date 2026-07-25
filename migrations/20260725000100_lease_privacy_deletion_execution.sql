-- Deletion execution is crash-recoverable but remains fail closed. A worker
-- may mutate protected surfaces only while both the job and subject fence
-- carry a live lease. Expired executions are retained as failed evidence and
-- can be reclaimed by the same canonical deletion job.

-- Do not "repair" a legacy invariant violation by destroying key material in
-- DDL. Key destruction is a formal state mutation and requires the canonical
-- deletion workflow below. Hold writers out while checking so the named
-- constraint cannot race an invalid insert. Operators must investigate and
-- remediate any violating row through an authorized deletion job.
LOCK TABLE public.privacy_subject_keys IN SHARE ROW EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM public.privacy_subject_keys
         WHERE destroyed_at IS NOT NULL
           AND wrapped_key IS NOT NULL
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE =
                'privacy subject key material remains after a claimed destruction',
            DETAIL =
                'migration refuses to clear wrapped_key without canonical deletion evidence',
            HINT =
                'investigate and remediate through an authorized deletion workflow before retrying';
    END IF;
END;
$$;

ALTER TABLE public.privacy_deletion_jobs
    ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS lease_recovery_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_lease_expired_at TIMESTAMPTZ;

ALTER TABLE public.privacy_subject_deletion_fences
    ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;

ALTER TABLE public.privacy_deletion_job_targets
    ADD COLUMN IF NOT EXISTS progress_cursor BIGINT NOT NULL DEFAULT 1,
    DROP CONSTRAINT IF EXISTS privacy_deletion_target_progress_cursor_check,
    ADD CONSTRAINT privacy_deletion_target_progress_cursor_check
        CHECK (progress_cursor > 0);

ALTER TABLE public.privacy_subject_keys
    DROP CONSTRAINT IF EXISTS privacy_subject_keys_destroyed_material_check,
    ADD CONSTRAINT privacy_subject_keys_destroyed_material_check
        CHECK (destroyed_at IS NULL OR wrapped_key IS NULL);

-- A pre-migration running row has no defensible live owner. Preserve it as an
-- explicit expired execution instead of manufacturing a successful result.
UPDATE public.privacy_subject_deletion_fences
   SET status = 'failed',
       lease_expires_at = NULL,
       updated_at = statement_timestamp()
 WHERE status = 'running';

UPDATE public.privacy_deletion_jobs
   SET status = 'failed',
       failure_code = 'DELETION_LEASE_EXPIRED',
       lease_expires_at = NULL,
       lease_recovery_count = lease_recovery_count + 1,
       last_lease_expired_at = statement_timestamp(),
       updated_at = statement_timestamp()
 WHERE status IN ('running', 'verifying');

ALTER TABLE public.privacy_deletion_jobs
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_live_lease_check,
    ADD CONSTRAINT privacy_deletion_jobs_live_lease_check CHECK (
        (status IN ('running', 'verifying') AND lease_expires_at IS NOT NULL)
        OR
        (status NOT IN ('running', 'verifying') AND lease_expires_at IS NULL)
    ),
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_lease_recovery_check,
    ADD CONSTRAINT privacy_deletion_jobs_lease_recovery_check CHECK (
        lease_recovery_count BETWEEN 0 AND 3
        AND (
            (lease_recovery_count = 0 AND last_lease_expired_at IS NULL)
            OR
            (lease_recovery_count > 0 AND last_lease_expired_at IS NOT NULL)
        )
    );

ALTER TABLE public.privacy_subject_deletion_fences
    DROP CONSTRAINT IF EXISTS privacy_deletion_fences_live_lease_check,
    ADD CONSTRAINT privacy_deletion_fences_live_lease_check CHECK (
        (status = 'running' AND lease_expires_at IS NOT NULL)
        OR
        (status <> 'running' AND lease_expires_at IS NULL)
    );

CREATE INDEX IF NOT EXISTS privacy_deletion_jobs_expired_lease_idx
    ON public.privacy_deletion_jobs(lease_expires_at, job_id)
    WHERE status IN ('running', 'verifying');

CREATE OR REPLACE FUNCTION public.enforce_privacy_deletion_job_evidence()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    canonical public.event_store%ROWTYPE;
BEGIN
    IF TG_OP = 'INSERT' AND NEW.evidence_status <> 'confirmed' THEN
        RAISE EXCEPTION
            'new deletion jobs require an already verified canonical request';
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF ROW(
               NEW.job_id, NEW.campaign_id, NEW.subject_id, NEW.requested_by,
               NEW.retention_policy, NEW.command_id, NEW.correlation_id,
               NEW.causation_id, NEW.canonical_event_type, NEW.created_at
           ) IS DISTINCT FROM ROW(
               OLD.job_id, OLD.campaign_id, OLD.subject_id, OLD.requested_by,
               OLD.retention_policy, OLD.command_id, OLD.correlation_id,
               OLD.causation_id, OLD.canonical_event_type, OLD.created_at
           ) THEN
            RAISE EXCEPTION
                'deletion job identity and expected event evidence are immutable';
        END IF;
        IF OLD.evidence_status = 'confirmed'
           AND ROW(
               NEW.evidence_status, NEW.canonical_event_sequence,
               NEW.canonical_event_integrity_hash
           ) IS DISTINCT FROM ROW(
               OLD.evidence_status, OLD.canonical_event_sequence,
               OLD.canonical_event_integrity_hash
           ) THEN
            RAISE EXCEPTION 'confirmed deletion event evidence is immutable';
        END IF;

        IF NEW.lease_recovery_count < OLD.lease_recovery_count THEN
            RAISE EXCEPTION 'deletion lease recovery count is monotonic';
        END IF;
        IF NEW.lease_recovery_count IS DISTINCT FROM OLD.lease_recovery_count
           AND NOT (
               OLD.status IN ('running', 'verifying')
               AND NEW.status = 'failed'
               AND NEW.failure_code = 'DELETION_LEASE_EXPIRED'
               AND NEW.lease_recovery_count = OLD.lease_recovery_count + 1
               AND NEW.lease_expires_at IS NULL
               AND NEW.last_lease_expired_at
                   IS DISTINCT FROM OLD.last_lease_expired_at
           ) THEN
            RAISE EXCEPTION
                'deletion lease recovery count requires an expired execution';
        END IF;
        IF NEW.last_lease_expired_at
               IS DISTINCT FROM OLD.last_lease_expired_at
           AND NEW.lease_recovery_count = OLD.lease_recovery_count THEN
            RAISE EXCEPTION
                'deletion lease expiry evidence requires a recovery increment';
        END IF;
        IF OLD.status = 'failed'
           AND NEW.status = 'failed'
           AND NEW.failure_code IS DISTINCT FROM OLD.failure_code THEN
            RAISE EXCEPTION 'failed deletion evidence is immutable';
        END IF;
        IF OLD.status = 'failed'
           AND OLD.failure_code = 'DELETION_LEASE_EXPIRED'
           AND OLD.lease_recovery_count >= 3
           AND ROW(
                   NEW.status, NEW.failure_code, NEW.lease_recovery_count,
                   NEW.last_lease_expired_at
               ) IS DISTINCT FROM ROW(
                   OLD.status, OLD.failure_code, OLD.lease_recovery_count,
                   OLD.last_lease_expired_at
               ) THEN
            RAISE EXCEPTION
                'exhausted deletion lease recovery evidence is terminal';
        END IF;

        IF NEW.status IS DISTINCT FROM OLD.status
           AND NOT (
               (OLD.status = 'requested'
                    AND NEW.status IN ('blocked_legal_hold', 'running', 'failed'))
               OR (OLD.status = 'blocked_legal_hold'
                    AND NEW.status IN ('blocked_legal_hold', 'running', 'failed'))
               OR (OLD.status = 'running'
                    AND NEW.status IN ('verifying', 'failed'))
               OR (OLD.status = 'verifying'
                    AND NEW.status IN ('completed', 'failed'))
               OR (OLD.status = 'failed'
                    AND OLD.failure_code = 'DELETION_LEASE_EXPIRED'
                    AND OLD.lease_recovery_count < 3
                    AND NEW.lease_recovery_count = OLD.lease_recovery_count
                    AND NEW.failure_code IS NULL
                    AND NEW.status IN ('blocked_legal_hold', 'running'))
           ) THEN
            RAISE EXCEPTION 'invalid deletion job state transition: % -> %',
                OLD.status, NEW.status;
        END IF;
    END IF;

    IF NEW.evidence_status = 'confirmed' THEN
        SELECT * INTO canonical
          FROM public.event_store
         WHERE sequence = NEW.canonical_event_sequence;
        IF NOT FOUND
           OR canonical.campaign_id IS DISTINCT FROM NEW.campaign_id
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
           OR canonical.deletion_retention_policy IS DISTINCT FROM
              NEW.retention_policy
           OR canonical.data_subject_id IS DISTINCT FROM NEW.subject_id
           OR canonical.resource_type IS DISTINCT FROM 'data_subject'
           OR canonical.resource_id IS DISTINCT FROM NEW.subject_id
           OR canonical.fact_provenance_kind IS DISTINCT FROM 'user_statement'
           OR canonical.fact_recorded_by IS DISTINCT FROM NEW.requested_by
           OR NOT (canonical.payload_json ? 'protected_payload') THEN
            RAISE EXCEPTION
                'deletion evidence does not match the canonical request event';
        END IF;
    END IF;

    IF NEW.status IN ('running', 'verifying') THEN
        IF NEW.evidence_status <> 'confirmed'
           OR NEW.lease_expires_at IS NULL
           OR NEW.lease_expires_at <= statement_timestamp() THEN
            RAISE EXCEPTION
                'active deletion execution requires confirmed evidence and a live lease';
        END IF;
    ELSIF NEW.lease_expires_at IS NOT NULL THEN
        RAISE EXCEPTION 'inactive deletion execution cannot retain a lease';
    END IF;

    IF NEW.status = 'completed' THEN
        IF EXISTS (
            SELECT 1
              FROM public.privacy_deletion_job_targets AS target
             WHERE target.job_id = NEW.job_id
               AND target.status <> 'verified'
        ) OR NOT EXISTS (
            SELECT 1
              FROM public.privacy_subject_deletion_fences AS fence
             WHERE fence.subject_id = NEW.subject_id
               AND fence.job_id = NEW.job_id
               AND fence.status = 'completed'
               AND fence.lease_expires_at IS NULL
        ) THEN
            RAISE EXCEPTION
                'deletion job cannot complete before targets and fence are verified';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION public.require_running_deletion_authority()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    affected_subject TEXT;
BEGIN
    affected_subject := NEW.subject_id;
    IF NOT EXISTS (
        SELECT 1
          FROM public.privacy_subject_deletion_fences AS fence
          JOIN public.privacy_deletion_jobs AS job
            ON job.job_id = fence.job_id
           AND job.subject_id = fence.subject_id
         WHERE fence.subject_id = affected_subject
           AND fence.status = 'running'
           AND fence.lease_expires_at > statement_timestamp()
           AND job.status = 'running'
           AND job.lease_expires_at > statement_timestamp()
           AND job.evidence_status = 'confirmed'
    ) THEN
        RAISE EXCEPTION
            'privacy erasure mutation requires a live confirmed deletion lease';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION public.prevent_destroyed_subject_key_restoration()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF OLD.destroyed_at IS NOT NULL
       AND (NEW.wrapped_key IS NOT NULL OR NEW.destroyed_at IS NULL) THEN
        RAISE EXCEPTION 'destroyed subject key cannot be restored';
    END IF;
    IF NEW.subject_id IS DISTINCT FROM OLD.subject_id
       OR NEW.key_reference IS DISTINCT FROM OLD.key_reference THEN
        RAISE EXCEPTION 'subject key identity is immutable';
    END IF;
    IF OLD.destroyed_at IS NULL
       AND NEW.destroyed_at IS NOT NULL
       AND NOT EXISTS (
           SELECT 1
             FROM public.privacy_subject_deletion_fences AS fence
             JOIN public.privacy_deletion_jobs AS job
               ON job.job_id = fence.job_id
              AND job.subject_id = fence.subject_id
            WHERE fence.subject_id = NEW.subject_id
              AND fence.status = 'running'
              AND fence.lease_expires_at > statement_timestamp()
              AND job.status = 'running'
              AND job.lease_expires_at > statement_timestamp()
              AND job.evidence_status = 'confirmed'
       ) THEN
        RAISE EXCEPTION
            'subject key destruction requires a live confirmed deletion lease';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION public.enforce_privacy_deletion_target_transition()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    parent_status TEXT;
    parent_evidence TEXT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        SELECT status, evidence_status
          INTO parent_status, parent_evidence
          FROM public.privacy_deletion_jobs
         WHERE job_id = NEW.job_id;
        IF NOT FOUND
           OR parent_status <> 'requested'
           OR parent_evidence <> 'confirmed'
           OR NEW.status <> 'pending'
           OR NEW.error_code IS NOT NULL
           OR NEW.deleted_at IS NOT NULL
           OR NEW.verified_at IS NOT NULL
           OR NEW.progress_cursor <> 1 THEN
            RAISE EXCEPTION
                'deletion target requires a confirmed requested parent job';
        END IF;
        RETURN NEW;
    END IF;

    IF ROW(NEW.job_id, NEW.target)
       IS DISTINCT FROM ROW(OLD.job_id, OLD.target) THEN
        RAISE EXCEPTION 'deletion target identity is immutable';
    END IF;
    IF NEW.progress_cursor IS DISTINCT FROM OLD.progress_cursor THEN
        IF NEW.progress_cursor <= OLD.progress_cursor
           OR OLD.status <> 'pending'
           OR NEW.status <> 'pending'
           OR NOT EXISTS (
               SELECT 1
                 FROM public.privacy_deletion_jobs AS job
                 JOIN public.privacy_subject_deletion_fences AS fence
                   ON fence.job_id = job.job_id
                  AND fence.subject_id = job.subject_id
                WHERE job.job_id = NEW.job_id
                  AND job.status = 'running'
                  AND job.lease_expires_at > statement_timestamp()
                  AND fence.status = 'running'
                  AND fence.lease_expires_at > statement_timestamp()
           ) THEN
            RAISE EXCEPTION
                'deletion target progress requires a live monotonic execution';
        END IF;
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status
       AND NOT (
           (OLD.status = 'pending' AND NEW.status IN ('deleted', 'failed'))
           OR (OLD.status = 'deleted' AND NEW.status IN ('verified', 'failed'))
       ) THEN
        RAISE EXCEPTION 'invalid deletion target state transition: % -> %',
            OLD.status, NEW.status;
    END IF;
    IF NEW.status = 'pending'
       AND (NEW.deleted_at IS NOT NULL OR NEW.verified_at IS NOT NULL
            OR NEW.error_code IS NOT NULL) THEN
        RAISE EXCEPTION 'pending deletion target cannot carry terminal evidence';
    ELSIF NEW.status = 'deleted'
       AND (NEW.deleted_at IS NULL OR NEW.verified_at IS NOT NULL
            OR NEW.error_code IS NOT NULL) THEN
        RAISE EXCEPTION 'deleted target requires delete evidence only';
    ELSIF NEW.status = 'verified'
       AND (NEW.deleted_at IS NULL OR NEW.verified_at IS NULL
            OR NEW.error_code IS NOT NULL
            OR NEW.verified_at < NEW.deleted_at) THEN
        RAISE EXCEPTION 'verified target requires ordered delete and verify evidence';
    ELSIF NEW.status = 'failed'
       AND btrim(COALESCE(NEW.error_code, '')) = '' THEN
        RAISE EXCEPTION 'failed target requires a failure code';
    END IF;
    RETURN NEW;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service'
    ) THEN
        RAISE EXCEPTION
            'required database role trpg_worker_service is not provisioned';
    END IF;
    GRANT UPDATE (
        lease_expires_at, lease_recovery_count, last_lease_expired_at
    ) ON public.privacy_deletion_jobs TO trpg_worker_service;
    GRANT UPDATE (lease_expires_at)
        ON public.privacy_subject_deletion_fences TO trpg_worker_service;
    GRANT UPDATE (progress_cursor)
        ON public.privacy_deletion_job_targets TO trpg_worker_service;
END;
$$;
