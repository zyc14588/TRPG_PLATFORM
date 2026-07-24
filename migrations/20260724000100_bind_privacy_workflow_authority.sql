-- P05 repair: bind deletion workflow state to its campaign and canonical
-- request, enforce real state-machine transitions, and prevent workload roles
-- from manufacturing terminal privacy/outbox evidence with direct SQL.

ALTER TABLE public.privacy_deletion_jobs
    ADD COLUMN IF NOT EXISTS campaign_id TEXT;

UPDATE public.privacy_deletion_jobs AS job
   SET campaign_id = canonical.campaign_id
  FROM public.event_store AS canonical
 WHERE job.campaign_id IS NULL
   AND job.canonical_event_sequence = canonical.sequence
   AND job.evidence_status = 'confirmed'
   AND canonical.event_type =
       'platform.security_privacy_copyright.data_deletion_requested'
   AND canonical.deletion_job_id = job.job_id
   AND canonical.deletion_subject_id = job.subject_id;

-- Pre-handshake rows are retained as audit evidence, but are deliberately
-- unscoped and can never execute. No campaign is inferred for them.
UPDATE public.privacy_deletion_jobs
   SET campaign_id = 'historical_unscoped'
 WHERE campaign_id IS NULL;

ALTER TABLE public.privacy_deletion_jobs
    ALTER COLUMN campaign_id SET NOT NULL,
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_campaign_id_valid,
    ADD CONSTRAINT privacy_deletion_jobs_campaign_id_valid
        CHECK (btrim(campaign_id) <> '' AND length(campaign_id) <= 160);

CREATE INDEX IF NOT EXISTS privacy_deletion_jobs_campaign_idx
    ON public.privacy_deletion_jobs(campaign_id, created_at DESC, job_id);

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

    IF NEW.status IN ('running', 'verifying', 'completed')
       AND NEW.evidence_status <> 'confirmed' THEN
        RAISE EXCEPTION
            'deletion job cannot execute without canonical event evidence';
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
        ) THEN
            RAISE EXCEPTION
                'deletion job cannot complete before targets and fence are verified';
        END IF;
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
           OR NEW.verified_at IS NOT NULL THEN
            RAISE EXCEPTION
                'deletion target requires a confirmed requested parent job';
        END IF;
        RETURN NEW;
    END IF;

    IF ROW(NEW.job_id, NEW.target)
       IS DISTINCT FROM ROW(OLD.job_id, OLD.target) THEN
        RAISE EXCEPTION 'deletion target identity is immutable';
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

DROP TRIGGER IF EXISTS privacy_deletion_target_transition_guard
    ON public.privacy_deletion_job_targets;
CREATE TRIGGER privacy_deletion_target_transition_guard
BEFORE INSERT OR UPDATE ON public.privacy_deletion_job_targets
FOR EACH ROW EXECUTE FUNCTION
    public.enforce_privacy_deletion_target_transition();

CREATE OR REPLACE FUNCTION public.enforce_outbox_delivery_transition()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.delivery_status <> 'pending'
           OR NEW.retry_count <> 0
           OR NEW.published_at IS NOT NULL
           OR NEW.dead_lettered_at IS NOT NULL
           OR NEW.claimed_at IS NOT NULL
           OR NEW.claim_owner IS NOT NULL
           OR NEW.claim_token IS NOT NULL
           OR NEW.locked_until IS NOT NULL THEN
            RAISE EXCEPTION 'new outbox records must begin pending';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.delivery_status IN ('published', 'dead_lettered')
       AND ROW(
           NEW.delivery_status, NEW.published_at, NEW.dead_lettered_at,
           NEW.retry_count, NEW.available_at, NEW.claimed_at,
           NEW.claim_owner, NEW.claim_token, NEW.locked_until, NEW.last_error
       ) IS DISTINCT FROM ROW(
           OLD.delivery_status, OLD.published_at, OLD.dead_lettered_at,
           OLD.retry_count, OLD.available_at, OLD.claimed_at,
           OLD.claim_owner, OLD.claim_token, OLD.locked_until, OLD.last_error
       ) THEN
        RAISE EXCEPTION 'terminal outbox delivery evidence is immutable';
    END IF;

    IF NEW.delivery_status IS DISTINCT FROM OLD.delivery_status THEN
        IF NEW.delivery_status = 'claimed' THEN
            IF OLD.delivery_status NOT IN ('pending', 'retrying', 'claimed')
               OR (OLD.delivery_status = 'claimed'
                   AND OLD.locked_until > statement_timestamp()) THEN
                RAISE EXCEPTION 'outbox claim requires an available nonterminal row';
            END IF;
        ELSIF NEW.delivery_status IN ('published', 'retrying') THEN
            IF OLD.delivery_status <> 'claimed'
               OR OLD.locked_until <= statement_timestamp()
               OR btrim(COALESCE(OLD.claim_owner, '')) = ''
               OR btrim(COALESCE(OLD.claim_token, '')) = '' THEN
                RAISE EXCEPTION
                    'outbox terminal/retry transition requires a live claim';
            END IF;
        ELSIF NEW.delivery_status = 'dead_lettered' THEN
            IF OLD.delivery_status <> 'claimed'
               AND NOT (
                   OLD.delivery_status IN ('pending', 'retrying')
                   AND (
                       OLD.integrity_status <> 'verified_hmac'
                       OR OLD.request_hash_source <> 'formal_commit'
                       OR OLD.commit_id IS NULL
                       OR EXISTS (
                           SELECT 1
                             FROM public.event_store AS event
                            WHERE event.sequence = OLD.event_sequence
                              AND event.data_subject_id <> 'not_applicable'
                              AND NOT EXISTS (
                                  SELECT 1
                                    FROM public.privacy_subject_keys AS subject_key
                                   WHERE subject_key.subject_id =
                                         event.data_subject_id
                                     AND subject_key.key_reference =
                                         event.payload_key_reference
                                     AND subject_key.wrapped_key IS NOT NULL
                                     AND subject_key.destroyed_at IS NULL
                              )
                       )
                   )
               ) THEN
                RAISE EXCEPTION
                    'verified outbox dead-letter transition requires a live claim';
            END IF;
        ELSE
            RAISE EXCEPTION 'invalid outbox delivery transition: % -> %',
                OLD.delivery_status, NEW.delivery_status;
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS event_outbox_delivery_transition_guard
    ON public.event_outbox;
CREATE TRIGGER event_outbox_delivery_transition_guard
BEFORE INSERT OR UPDATE ON public.event_outbox
FOR EACH ROW EXECUTE FUNCTION public.enforce_outbox_delivery_transition();

CREATE OR REPLACE FUNCTION public.require_running_deletion_authority()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    affected_subject TEXT;
BEGIN
    affected_subject := CASE
        WHEN TG_TABLE_NAME = 'privacy_erased_subjects' THEN NEW.subject_id
        ELSE NEW.subject_id
    END;
    IF NOT EXISTS (
        SELECT 1
          FROM public.privacy_subject_deletion_fences AS fence
          JOIN public.privacy_deletion_jobs AS job
            ON job.job_id = fence.job_id
           AND job.subject_id = fence.subject_id
         WHERE fence.subject_id = affected_subject
           AND fence.status = 'running'
           AND job.status = 'running'
           AND job.evidence_status = 'confirmed'
    ) THEN
        RAISE EXCEPTION
            'privacy erasure mutation requires a running confirmed deletion job';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS privacy_erased_subjects_creation_authority
    ON public.privacy_erased_subjects;
CREATE TRIGGER privacy_erased_subjects_creation_authority
BEFORE INSERT ON public.privacy_erased_subjects
FOR EACH ROW EXECUTE FUNCTION public.require_running_deletion_authority();

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
              AND job.status = 'running'
              AND job.evidence_status = 'confirmed'
       ) THEN
        RAISE EXCEPTION
            'subject key destruction requires a running confirmed deletion job';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION public.reject_erased_subject_consent()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NEW.granted AND EXISTS (
        SELECT 1
          FROM public.privacy_erased_subjects
         WHERE subject_id = NEW.subject_id
    ) THEN
        RAISE EXCEPTION 'cloud consent cannot be granted to an erased subject';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS cloud_egress_consents_erasure_guard
    ON public.cloud_egress_consents;
CREATE TRIGGER cloud_egress_consents_erasure_guard
BEFORE INSERT OR UPDATE ON public.cloud_egress_consents
FOR EACH ROW EXECUTE FUNCTION public.reject_erased_subject_consent();

CREATE OR REPLACE FUNCTION public.enforce_outbound_membership_revocation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    affected_campaign TEXT;
    affected_user TEXT;
    remains_group_eligible BOOLEAN;
BEGIN
    affected_campaign := CASE WHEN TG_OP = 'DELETE'
        THEN OLD.campaign_id ELSE NEW.campaign_id END;
    affected_user := CASE WHEN TG_OP = 'DELETE'
        THEN OLD.user_id ELSE NEW.user_id END;
    remains_group_eligible := TG_OP <> 'DELETE'
        AND NEW.revoked_at IS NULL
        AND NEW.role IN ('CAMPAIGN_OWNER', 'PLAYER');
    IF NOT remains_group_eligible THEN
        UPDATE public.campaign_group_memberships
           SET revoked_at = COALESCE(revoked_at, statement_timestamp())
         WHERE campaign_id = affected_campaign
           AND user_id = affected_user
           AND revoked_at IS NULL;
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS campaign_memberships_group_revocation
    ON public.campaign_memberships;
CREATE TRIGGER campaign_memberships_group_revocation
AFTER UPDATE OR DELETE ON public.campaign_memberships
FOR EACH ROW EXECUTE FUNCTION public.enforce_outbound_membership_revocation();

DO $least_privilege$
BEGIN
    -- Schema privileges inherited through PUBLIC survive per-role REVOKE.
    -- Production service roles receive only their explicit grants below.
    REVOKE ALL PRIVILEGES ON SCHEMA public FROM PUBLIC;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        REVOKE UPDATE, DELETE ON public.users FROM trpg_api_service;
        GRANT UPDATE (login_normalized, password_hash, disabled_at)
            ON public.users TO trpg_api_service;
        REVOKE UPDATE, DELETE ON public.campaign_memberships
            FROM trpg_api_service;
        GRANT UPDATE (role, granted_by, granted_at, revoked_at)
            ON public.campaign_memberships TO trpg_api_service;
        REVOKE UPDATE, DELETE ON public.campaign_group_memberships
            FROM trpg_api_service;
        GRANT UPDATE (granted_by, granted_at, revoked_at)
            ON public.campaign_group_memberships TO trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE ON public.cloud_egress_consents
            FROM trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE ON public.cloud_egress_notices
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        REVOKE DELETE ON public.campaign_memberships,
            public.campaign_group_memberships FROM trpg_worker_service;
        GRANT UPDATE (revoked_at) ON public.campaign_memberships,
            public.campaign_group_memberships TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
