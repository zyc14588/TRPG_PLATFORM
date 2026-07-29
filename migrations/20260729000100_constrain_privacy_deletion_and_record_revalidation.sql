-- Bind destructive privacy SQL to one live deletion claim and retain every
-- post-completion revalidation result without mutating the completion proof.

ALTER TABLE public.privacy_deletion_jobs
    ADD COLUMN IF NOT EXISTS execution_claim_token TEXT;

ALTER TABLE public.privacy_subject_deletion_fences
    ADD COLUMN IF NOT EXISTS execution_claim_token TEXT;

-- A migration cannot safely inherit an in-flight worker claim because the old
-- runtime did not possess an execution token. Preserve the interrupted run as
-- lease-expiry evidence so the canonical job can be reclaimed normally.
UPDATE public.privacy_subject_deletion_fences
   SET status = 'failed',
       lease_expires_at = NULL,
       execution_claim_token = NULL,
       updated_at = statement_timestamp()
 WHERE status = 'running';

UPDATE public.privacy_deletion_jobs
   SET status = 'failed',
       failure_code = 'DELETION_LEASE_EXPIRED',
       lease_expires_at = NULL,
       execution_claim_token = NULL,
       lease_recovery_count = CASE
           WHEN lease_recovery_count < 3 THEN lease_recovery_count + 1
           ELSE lease_recovery_count
       END,
       last_lease_expired_at = CASE
           WHEN lease_recovery_count < 3 THEN statement_timestamp()
           ELSE last_lease_expired_at
       END,
       updated_at = statement_timestamp()
 WHERE status IN ('running', 'verifying');

ALTER TABLE public.privacy_deletion_jobs
    DROP CONSTRAINT IF EXISTS privacy_deletion_jobs_execution_claim_check,
    ADD CONSTRAINT privacy_deletion_jobs_execution_claim_check CHECK (
        (
            status IN ('running', 'verifying')
            AND btrim(COALESCE(execution_claim_token, '')) <> ''
            AND length(execution_claim_token) <= 160
        )
        OR
        (
            status NOT IN ('running', 'verifying')
            AND execution_claim_token IS NULL
        )
    );

ALTER TABLE public.privacy_subject_deletion_fences
    DROP CONSTRAINT IF EXISTS privacy_deletion_fences_execution_claim_check,
    ADD CONSTRAINT privacy_deletion_fences_execution_claim_check CHECK (
        (
            status = 'running'
            AND btrim(COALESCE(execution_claim_token, '')) <> ''
            AND length(execution_claim_token) <= 160
        )
        OR
        (
            status <> 'running'
            AND execution_claim_token IS NULL
        )
    );

CREATE TABLE public.privacy_deletion_revalidation_runs (
    run_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES public.privacy_deletion_jobs(job_id),
    subject_id TEXT NOT NULL,
    claim_token_hash TEXT NOT NULL
        CHECK (claim_token_hash ~ '^[0-9a-f]{64}$'),
    completion_evidence_hash TEXT NOT NULL
        CHECK (completion_evidence_hash ~ '^sha256:[0-9a-f]{64}$'),
    started_at TIMESTAMPTZ NOT NULL DEFAULT statement_timestamp(),
    lease_expires_at TIMESTAMPTZ NOT NULL,
    CHECK (btrim(run_id) <> '' AND length(run_id) <= 160),
    CHECK (btrim(subject_id) <> '' AND length(subject_id) <= 160),
    CHECK (lease_expires_at > started_at)
);

CREATE INDEX privacy_deletion_revalidation_runs_job_idx
    ON public.privacy_deletion_revalidation_runs(job_id, started_at DESC, run_id);

CREATE TABLE public.privacy_deletion_revalidation_results (
    result_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL UNIQUE
        REFERENCES public.privacy_deletion_revalidation_runs(run_id),
    result_status TEXT NOT NULL CHECK (result_status IN ('passed', 'failed')),
    failure_target TEXT CHECK (
        failure_target IS NULL OR failure_target IN (
            'database', 'rag_index', 'object_storage', 'cache',
            'queue', 'export', 'backup_key'
        )
    ),
    error_code TEXT,
    evidence_hash TEXT NOT NULL
        CHECK (evidence_hash ~ '^sha256:[0-9a-f]{64}$'),
    alert_status TEXT NOT NULL CHECK (
        alert_status IN ('not_required', 'pending_acknowledgement')
    ),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT statement_timestamp(),
    CHECK (
        (
            result_status = 'passed'
            AND failure_target IS NULL
            AND error_code IS NULL
            AND alert_status = 'not_required'
        )
        OR
        (
            result_status = 'failed'
            AND failure_target IS NOT NULL
            AND btrim(COALESCE(error_code, '')) <> ''
            AND length(error_code) <= 160
            AND alert_status = 'pending_acknowledgement'
        )
    )
);

CREATE OR REPLACE FUNCTION public.reject_privacy_revalidation_mutation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    RAISE EXCEPTION 'privacy deletion revalidation evidence is append-only';
END;
$$;

CREATE TRIGGER privacy_deletion_revalidation_runs_immutable
BEFORE UPDATE OR DELETE OR TRUNCATE
ON public.privacy_deletion_revalidation_runs
FOR EACH STATEMENT
EXECUTE FUNCTION public.reject_privacy_revalidation_mutation();

CREATE TRIGGER privacy_deletion_revalidation_results_immutable
BEFORE UPDATE OR DELETE OR TRUNCATE
ON public.privacy_deletion_revalidation_results
FOR EACH STATEMENT
EXECUTE FUNCTION public.reject_privacy_revalidation_mutation();

CREATE OR REPLACE FUNCTION public.require_privacy_deletion_claim(
    p_job_id TEXT,
    p_subject_id TEXT,
    p_claim_token TEXT
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM public.privacy_deletion_jobs AS job
          JOIN public.privacy_subject_deletion_fences AS fence
            ON fence.job_id = job.job_id
           AND fence.subject_id = job.subject_id
         WHERE job.job_id = p_job_id
           AND job.subject_id = p_subject_id
           AND job.status = 'running'
           AND job.evidence_status = 'confirmed'
           AND job.execution_claim_token = p_claim_token
           AND job.lease_expires_at > statement_timestamp()
           AND fence.status = 'running'
           AND fence.execution_claim_token = p_claim_token
           AND fence.lease_expires_at > statement_timestamp()
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = 'insufficient_privilege',
            MESSAGE =
                'privacy erasure requires the matching live deletion claim';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION public.erase_privacy_database_subject(
    p_job_id TEXT,
    p_subject_id TEXT,
    p_claim_token TEXT
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    digest_hex TEXT;
    expected_erasure_digest TEXT;
    persisted_erasure_digest TEXT;
BEGIN
    PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
            'privacy_subject_delete:' || p_subject_id,
            0
        )
    );
    PERFORM public.require_privacy_deletion_claim(
        p_job_id,
        p_subject_id,
        p_claim_token
    );

    IF EXISTS (
        SELECT 1
          FROM public.authority_contracts
         WHERE authority_owner = p_subject_id
    ) THEN
        RAISE EXCEPTION
            'canonical authority owner requires a campaign fork before erasure';
    END IF;

    digest_hex := pg_catalog.encode(
        pg_catalog.sha256(
            pg_catalog.convert_to(p_subject_id, 'UTF8')
        ),
        'hex'
    );
    expected_erasure_digest := 'sha256:' || digest_hex;

    SELECT erased.erasure_digest
      INTO persisted_erasure_digest
      FROM public.privacy_erased_subjects AS erased
     WHERE erased.subject_id = p_subject_id;
    IF FOUND
       AND persisted_erasure_digest IS DISTINCT FROM expected_erasure_digest THEN
        RAISE EXCEPTION 'persisted erasure digest does not match data subject';
    END IF;

    DELETE FROM public.sessions
     WHERE user_id = p_subject_id;

    UPDATE public.campaign_group_memberships
       SET revoked_at = COALESCE(revoked_at, statement_timestamp())
     WHERE user_id = p_subject_id;

    UPDATE public.campaign_memberships
       SET revoked_at = COALESCE(revoked_at, statement_timestamp())
     WHERE user_id = p_subject_id;

    UPDATE public.users
       SET login_normalized = 'deleted_' || digest_hex,
           password_hash = 'DELETED_ACCOUNT_NO_LOGIN_' || digest_hex,
           disabled_at = COALESCE(disabled_at, statement_timestamp())
     WHERE user_id = p_subject_id;

    UPDATE public.cloud_egress_consents
       SET granted = false,
           updated_at = statement_timestamp()
     WHERE subject_id = p_subject_id
       AND granted = true;

    INSERT INTO public.privacy_erased_subjects (
        subject_id,
        erasure_digest
    )
    VALUES (
        p_subject_id,
        expected_erasure_digest
    )
    ON CONFLICT (subject_id) DO NOTHING;
END;
$$;

CREATE OR REPLACE FUNCTION public.erase_privacy_rag_subject(
    p_job_id TEXT,
    p_subject_id TEXT,
    p_claim_token TEXT
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
            'privacy_subject_delete:' || p_subject_id,
            0
        )
    );
    PERFORM public.require_privacy_deletion_claim(
        p_job_id,
        p_subject_id,
        p_claim_token
    );

    DELETE FROM public.rag_snapshot_chunk
     WHERE visibility_subject = p_subject_id
        OR source_event_sequence IN (
            SELECT event.sequence
              FROM public.event_store AS event
             WHERE event.data_subject_id = p_subject_id
        );
END;
$$;

CREATE OR REPLACE FUNCTION public.begin_privacy_deletion_revalidation(
    p_job_id TEXT,
    p_subject_id TEXT
)
RETURNS TABLE(run_id TEXT, claim_token TEXT)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    v_run_id TEXT;
    v_claim_token TEXT;
    v_completion_material TEXT;
    v_completion_evidence_hash TEXT;
BEGIN
    PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
            'privacy_deletion_revalidation:' || p_job_id,
            0
        )
    );

    SELECT
        job.canonical_event_integrity_hash
        || E'\x1f'
        || pg_catalog.string_agg(
            target.target || ':' || target.status,
            ','
            ORDER BY target.target
        )
      INTO v_completion_material
      FROM public.privacy_deletion_jobs AS job
      JOIN public.privacy_subject_deletion_fences AS fence
        ON fence.job_id = job.job_id
       AND fence.subject_id = job.subject_id
      JOIN public.privacy_deletion_job_targets AS target
        ON target.job_id = job.job_id
     WHERE job.job_id = p_job_id
       AND job.subject_id = p_subject_id
       AND job.status = 'completed'
       AND job.evidence_status = 'confirmed'
       AND job.execution_claim_token IS NULL
       AND fence.status = 'completed'
       AND fence.execution_claim_token IS NULL
     GROUP BY job.canonical_event_integrity_hash
    HAVING pg_catalog.bool_and(target.status = 'verified')
       AND pg_catalog.count(*) = 7;

    IF v_completion_material IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = 'insufficient_privilege',
            MESSAGE =
                'revalidation requires an immutable completed deletion proof';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM public.privacy_deletion_revalidation_runs AS prior
         WHERE prior.job_id = p_job_id
           AND prior.subject_id = p_subject_id
           AND prior.lease_expires_at > statement_timestamp()
           AND NOT EXISTS (
               SELECT 1
                 FROM public.privacy_deletion_revalidation_results AS result
                WHERE result.run_id = prior.run_id
           )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = 'object_in_use',
            MESSAGE = 'deletion revalidation is already running';
    END IF;

    v_run_id := pg_catalog.gen_random_uuid()::TEXT;
    v_claim_token := pg_catalog.gen_random_uuid()::TEXT;
    v_completion_evidence_hash := 'sha256:' || pg_catalog.encode(
        pg_catalog.sha256(
            pg_catalog.convert_to(v_completion_material, 'UTF8')
        ),
        'hex'
    );

    INSERT INTO public.privacy_deletion_revalidation_runs (
        run_id,
        job_id,
        subject_id,
        claim_token_hash,
        completion_evidence_hash,
        lease_expires_at
    )
    VALUES (
        v_run_id,
        p_job_id,
        p_subject_id,
        pg_catalog.encode(
            pg_catalog.sha256(
                pg_catalog.convert_to(v_claim_token, 'UTF8')
            ),
            'hex'
        ),
        v_completion_evidence_hash,
        statement_timestamp() + pg_catalog.make_interval(secs => 300)
    );

    RETURN QUERY SELECT v_run_id, v_claim_token;
END;
$$;

CREATE OR REPLACE FUNCTION public.record_privacy_deletion_revalidation_result(
    p_run_id TEXT,
    p_job_id TEXT,
    p_subject_id TEXT,
    p_claim_token TEXT,
    p_result_status TEXT,
    p_failure_target TEXT,
    p_error_code TEXT,
    p_evidence_hash TEXT
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
    PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
            'privacy_deletion_revalidation_run:' || p_run_id,
            0
        )
    );

    IF p_evidence_hash !~ '^sha256:[0-9a-f]{64}$'
       OR (
           p_result_status = 'passed'
           AND (
               p_failure_target IS NOT NULL
               OR p_error_code IS NOT NULL
           )
       )
       OR (
           p_result_status = 'failed'
           AND (
               p_failure_target NOT IN (
                   'database', 'rag_index', 'object_storage', 'cache',
                   'queue', 'export', 'backup_key'
               )
               OR pg_catalog.btrim(COALESCE(p_error_code, '')) = ''
               OR pg_catalog.length(p_error_code) > 160
           )
       )
       OR p_result_status NOT IN ('passed', 'failed') THEN
        RAISE EXCEPTION 'invalid deletion revalidation result';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM public.privacy_deletion_revalidation_runs AS run
          JOIN public.privacy_deletion_jobs AS job
            ON job.job_id = run.job_id
           AND job.subject_id = run.subject_id
          JOIN public.privacy_subject_deletion_fences AS fence
            ON fence.job_id = job.job_id
           AND fence.subject_id = job.subject_id
         WHERE run.run_id = p_run_id
           AND run.job_id = p_job_id
           AND run.subject_id = p_subject_id
           AND run.claim_token_hash = pg_catalog.encode(
               pg_catalog.sha256(
                   pg_catalog.convert_to(p_claim_token, 'UTF8')
               ),
               'hex'
           )
           AND run.lease_expires_at > statement_timestamp()
           AND job.status = 'completed'
           AND job.execution_claim_token IS NULL
           AND fence.status = 'completed'
           AND fence.execution_claim_token IS NULL
           AND NOT EXISTS (
               SELECT 1
                 FROM public.privacy_deletion_revalidation_results AS result
                WHERE result.run_id = run.run_id
           )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = 'insufficient_privilege',
            MESSAGE =
                'revalidation result requires the matching live run claim';
    END IF;

    INSERT INTO public.privacy_deletion_revalidation_results (
        result_id,
        run_id,
        result_status,
        failure_target,
        error_code,
        evidence_hash,
        alert_status
    )
    VALUES (
        pg_catalog.gen_random_uuid()::TEXT,
        p_run_id,
        p_result_status,
        p_failure_target,
        p_error_code,
        p_evidence_hash,
        CASE
            WHEN p_result_status = 'failed'
                THEN 'pending_acknowledgement'
            ELSE 'not_required'
        END
    );
END;
$$;

-- Existing row guards remain defense in depth for other deletion surfaces.
-- They now require the job and fence to agree on a nonempty live claim.
CREATE OR REPLACE FUNCTION public.require_running_deletion_authority()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM public.privacy_subject_deletion_fences AS fence
          JOIN public.privacy_deletion_jobs AS job
            ON job.job_id = fence.job_id
           AND job.subject_id = fence.subject_id
         WHERE fence.subject_id = NEW.subject_id
           AND fence.status = 'running'
           AND fence.lease_expires_at > statement_timestamp()
           AND pg_catalog.btrim(
               COALESCE(fence.execution_claim_token, '')
           ) <> ''
           AND job.status = 'running'
           AND job.lease_expires_at > statement_timestamp()
           AND job.execution_claim_token = fence.execution_claim_token
           AND job.evidence_status = 'confirmed'
    ) THEN
        RAISE EXCEPTION
            'privacy erasure mutation requires a live confirmed deletion claim';
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
              AND pg_catalog.btrim(
                  COALESCE(fence.execution_claim_token, '')
              ) <> ''
              AND job.status = 'running'
              AND job.lease_expires_at > statement_timestamp()
              AND job.execution_claim_token = fence.execution_claim_token
              AND job.evidence_status = 'confirmed'
       ) THEN
        RAISE EXCEPTION
            'subject key destruction requires a live confirmed deletion claim';
    END IF;
    RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION public.require_privacy_deletion_claim(TEXT, TEXT, TEXT)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION public.erase_privacy_database_subject(TEXT, TEXT, TEXT)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION public.erase_privacy_rag_subject(TEXT, TEXT, TEXT)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION public.begin_privacy_deletion_revalidation(TEXT, TEXT)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION public.record_privacy_deletion_revalidation_result(
    TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
) FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE, TRUNCATE
    ON public.privacy_deletion_revalidation_runs,
       public.privacy_deletion_revalidation_results
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service'
    ) THEN
        RAISE EXCEPTION
            'required database role trpg_worker_service is not provisioned';
    END IF;

    REVOKE DELETE ON
        public.sessions,
        public.rag_snapshot_chunk,
        public.privacy_deletion_surface_records
        FROM trpg_worker_service;
    REVOKE UPDATE (revoked_at) ON
        public.campaign_memberships,
        public.campaign_group_memberships
        FROM trpg_worker_service;
    REVOKE UPDATE (login_normalized, password_hash, disabled_at)
        ON public.users FROM trpg_worker_service;
    REVOKE UPDATE (granted, updated_at)
        ON public.cloud_egress_consents FROM trpg_worker_service;
    REVOKE INSERT ON public.privacy_erased_subjects
        FROM trpg_worker_service;

    GRANT UPDATE (execution_claim_token)
        ON public.privacy_deletion_jobs,
           public.privacy_subject_deletion_fences
        TO trpg_worker_service;
    GRANT SELECT ON
        public.privacy_deletion_revalidation_runs,
        public.privacy_deletion_revalidation_results
        TO trpg_worker_service;
    GRANT EXECUTE ON FUNCTION
        public.erase_privacy_database_subject(TEXT, TEXT, TEXT),
        public.erase_privacy_rag_subject(TEXT, TEXT, TEXT),
        public.begin_privacy_deletion_revalidation(TEXT, TEXT),
        public.record_privacy_deletion_revalidation_result(
            TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
        )
        TO trpg_worker_service;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT ON
            public.privacy_deletion_revalidation_runs,
            public.privacy_deletion_revalidation_results
            TO trpg_api_service;
    END IF;
END;
$least_privilege$;
