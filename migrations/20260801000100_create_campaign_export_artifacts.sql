-- AR12 forward-only migration: asynchronous, privacy-bound campaign exports.
--
-- `campaign_exports` remains the immutable projection of the canonical
-- CampaignExportRequested event.  The tables introduced here are rebuildable
-- worker/read-model state and short-lived authorization state; they are not a
-- second source of campaign truth.

ALTER TABLE public.campaign_exports
    DROP CONSTRAINT campaign_exports_audience_check,
    ADD CONSTRAINT campaign_exports_audience_check CHECK (
        audience IN ('PLAYER', 'KEEPER_PRIVATE', 'AUDIT', 'CAMPAIGN_ARCHIVE')
    );

CREATE TABLE public.campaign_export_jobs (
    export_id TEXT PRIMARY KEY
        REFERENCES public.campaign_exports(export_id) ON DELETE CASCADE,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    state TEXT NOT NULL CHECK (
        state IN ('REQUESTED', 'RUNNING', 'READY', 'FAILED', 'EXPIRED', 'DELETED')
    ),
    attempt_count SMALLINT NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 5),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    requested_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    first_event_sequence BIGINT,
    last_event_sequence BIGINT,
    event_count BIGINT CHECK (event_count IS NULL OR event_count >= 0),
    visibility_policy_version TEXT NOT NULL DEFAULT 'visibility-policy-v1'
        CHECK (btrim(visibility_policy_version) <> ''),
    artifact_schema TEXT NOT NULL DEFAULT 'trpg.campaign-export.v1'
        CHECK (btrim(artifact_schema) <> ''),
    artifact_key TEXT,
    artifact_hash TEXT CHECK (
        artifact_hash IS NULL OR artifact_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    manifest_hash TEXT CHECK (
        manifest_hash IS NULL OR manifest_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    artifact_size BIGINT CHECK (artifact_size IS NULL OR artifact_size > 0),
    ready_at TIMESTAMPTZ,
    retention_expires_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    failure_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (campaign_id, export_id),
    CHECK (
        (state = 'RUNNING' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (state <> 'RUNNING' AND lease_owner IS NULL AND lease_expires_at IS NULL)
    ),
    CHECK (
        state <> 'READY'
        OR
        artifact_key IS NOT NULL
        AND artifact_key !~ '(^|/)\.\.(/|$)'
        AND artifact_hash IS NOT NULL
        AND manifest_hash IS NOT NULL
        AND artifact_size IS NOT NULL
        AND ready_at IS NOT NULL
        AND retention_expires_at > ready_at
        AND first_event_sequence IS NOT NULL
        AND last_event_sequence IS NOT NULL
        AND event_count IS NOT NULL
    ),
    CHECK (
        state NOT IN ('EXPIRED', 'DELETED')
        OR artifact_key IS NULL AND deleted_at IS NOT NULL
    )
);

CREATE UNIQUE INDEX campaign_export_jobs_artifact_key_idx
    ON public.campaign_export_jobs(artifact_key)
    WHERE artifact_key IS NOT NULL;

CREATE INDEX campaign_export_jobs_claim_idx
    ON public.campaign_export_jobs(state, next_attempt_at, lease_expires_at, created_at);

INSERT INTO public.campaign_export_jobs (
    export_id, campaign_id, state, requested_event_sequence,
    next_attempt_at, created_at, updated_at
)
SELECT export_id, campaign_id, 'REQUESTED', last_event_sequence,
       requested_at, requested_at, requested_at
  FROM public.campaign_exports
ON CONFLICT (export_id) DO NOTHING;

CREATE TABLE public.campaign_export_subjects (
    export_id TEXT NOT NULL
        REFERENCES public.campaign_export_jobs(export_id) ON DELETE CASCADE,
    subject_id TEXT NOT NULL REFERENCES public.users(user_id),
    PRIMARY KEY (export_id, subject_id)
);

CREATE INDEX campaign_export_subjects_subject_idx
    ON public.campaign_export_subjects(subject_id, export_id);

CREATE TABLE public.campaign_export_download_tickets (
    token_hash TEXT PRIMARY KEY CHECK (token_hash ~ '^sha256:[0-9a-f]{64}$'),
    export_id TEXT NOT NULL
        REFERENCES public.campaign_export_jobs(export_id) ON DELETE CASCADE,
    actor_id TEXT NOT NULL REFERENCES public.users(user_id),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at)
);

CREATE INDEX campaign_export_download_tickets_expiry_idx
    ON public.campaign_export_download_tickets(expires_at);

DO $least_privilege$
BEGIN
    REVOKE ALL PRIVILEGES ON
        public.campaign_export_jobs,
        public.campaign_export_subjects,
        public.campaign_export_download_tickets
        FROM PUBLIC;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT, INSERT ON public.campaign_export_jobs
            TO trpg_api_service;
        REVOKE UPDATE, DELETE ON public.campaign_export_jobs
            FROM trpg_api_service;
        GRANT SELECT, INSERT, DELETE ON public.campaign_export_download_tickets
            TO trpg_api_service;
        REVOKE UPDATE ON public.campaign_export_download_tickets
            FROM trpg_api_service;
        GRANT SELECT ON public.campaign_export_subjects
            TO trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE ON public.campaign_export_subjects
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT, UPDATE ON public.campaign_export_jobs
            TO trpg_worker_service;
        REVOKE INSERT, DELETE ON public.campaign_export_jobs
            FROM trpg_worker_service;
        GRANT SELECT, INSERT, DELETE ON public.campaign_export_subjects
            TO trpg_worker_service;
        REVOKE UPDATE ON public.campaign_export_subjects
            FROM trpg_worker_service;
        GRANT SELECT, DELETE ON public.campaign_export_download_tickets
            TO trpg_worker_service;
        REVOKE INSERT, UPDATE ON public.campaign_export_download_tickets
            FROM trpg_worker_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        REVOKE ALL PRIVILEGES ON
            public.campaign_export_jobs,
            public.campaign_export_subjects,
            public.campaign_export_download_tickets
            FROM trpg_canonical_service;
    END IF;
END;
$least_privilege$;
