-- Durable Agent jobs reuse workflow_instances for CAS versions and leases.
-- The companion tables bind authority/input/provider scope and retain only
-- bounded, visibility-labelled execution evidence.

ALTER TABLE public.workflow_instances
    ADD COLUMN IF NOT EXISTS claim_token TEXT,
    ADD COLUMN IF NOT EXISTS heartbeat_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS attempt INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ;

ALTER TABLE public.workflow_instances
    DROP CONSTRAINT IF EXISTS workflow_instances_state_check,
    ADD CONSTRAINT workflow_instances_state_check CHECK (
        state IN (
            'PENDING', 'RUNNING', 'WAITING', 'COMPLETED', 'FAILED', 'CANCELLED',
            'REQUESTED', 'CLAIMED', 'AGENT_RUNNING', 'AWAITING_TOOL',
            'COMMITTING', 'RETRYABLE_FAILED', 'TERMINAL_FAILED'
        )
    ) NOT VALID;

ALTER TABLE public.workflow_instances
    VALIDATE CONSTRAINT workflow_instances_state_check;

ALTER TABLE public.workflow_instances
    DROP CONSTRAINT IF EXISTS workflow_instances_agent_job_state_check,
    ADD CONSTRAINT workflow_instances_agent_job_state_check CHECK (
        workflow_type <> 'agent_job'
        OR state IN (
            'REQUESTED', 'CLAIMED', 'AGENT_RUNNING', 'AWAITING_TOOL',
            'COMMITTING', 'COMPLETED', 'RETRYABLE_FAILED',
            'TERMINAL_FAILED'
        )
    ) NOT VALID,
    DROP CONSTRAINT IF EXISTS workflow_instances_agent_lease_check,
    ADD CONSTRAINT workflow_instances_agent_lease_check CHECK (
        attempt >= 0
        AND (claim_token IS NULL OR btrim(claim_token) <> '')
        AND (
            workflow_type <> 'agent_job'
            OR state NOT IN (
                'CLAIMED', 'AGENT_RUNNING', 'AWAITING_TOOL', 'COMMITTING'
            )
            OR (
                attempt > 0
                AND lease_owner IS NOT NULL
                AND btrim(lease_owner) <> ''
                AND claim_token IS NOT NULL
                AND lease_expires_at IS NOT NULL
                AND heartbeat_at IS NOT NULL
            )
        )
    ) NOT VALID;

ALTER TABLE public.workflow_instances
    VALIDATE CONSTRAINT workflow_instances_agent_job_state_check;
ALTER TABLE public.workflow_instances
    VALIDATE CONSTRAINT workflow_instances_agent_lease_check;

CREATE TABLE IF NOT EXISTS public.agent_jobs (
    job_id TEXT PRIMARY KEY
        REFERENCES public.workflow_instances(workflow_id) ON DELETE RESTRICT,
    campaign_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    agent_kind TEXT NOT NULL CHECK (
        agent_kind IN ('ai_keeper_orchestrator', 'keeper_copilot')
    ),
    authority_contract_id TEXT NOT NULL
        REFERENCES public.authority_contracts(contract_id) ON DELETE RESTRICT,
    authority_mode TEXT NOT NULL CHECK (authority_mode IN ('AI_KP', 'HUMAN_KP')),
    authority_contract_version BIGINT NOT NULL
        CHECK (authority_contract_version > 0),
    input_event_sequence BIGINT NOT NULL
        REFERENCES public.event_store(sequence) ON DELETE RESTRICT,
    input_stream_version BIGINT NOT NULL CHECK (input_stream_version >= 0),
    visibility_scope JSONB NOT NULL,
    rag_snapshot_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_type TEXT NOT NULL CHECK (
        provider_type IN ('cloud', 'ollama', 'llama_cpp')
    ),
    model_id TEXT NOT NULL,
    model_artifact_sha256 TEXT NOT NULL CHECK (
        model_artifact_sha256 ~ '^sha256:[0-9a-f]{64}$'
    ),
    route_authorization_event_id TEXT NOT NULL,
    prompt_template_id TEXT NOT NULL,
    prompt_template_version TEXT NOT NULL,
    tool_schema_version TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    deadline_at TIMESTAMPTZ NOT NULL,
    resume_state TEXT,
    decision_json JSONB,
    tool_result_json JSONB,
    linked_event_sequences BIGINT[] NOT NULL DEFAULT '{}',
    cancellation_requested_at TIMESTAMPTZ,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (campaign_id, idempotency_key),
    CHECK (jsonb_typeof(visibility_scope) = 'object'),
    CHECK (btrim(actor_id) <> ''),
    CHECK (btrim(rag_snapshot_id) <> ''),
    CHECK (btrim(provider_id) <> ''),
    CHECK (btrim(model_id) <> ''),
    CHECK (btrim(route_authorization_event_id) <> ''),
    CHECK (btrim(prompt_template_id) <> ''),
    CHECK (btrim(prompt_template_version) <> ''),
    CHECK (btrim(tool_schema_version) <> ''),
    CHECK (btrim(idempotency_key) <> ''),
    CHECK (
        resume_state IS NULL OR resume_state IN (
            'REQUESTED', 'CLAIMED', 'AGENT_RUNNING', 'AWAITING_TOOL',
            'COMMITTING', 'RETRYABLE_FAILED'
        )
    )
);

CREATE INDEX IF NOT EXISTS agent_jobs_campaign_state_idx
    ON public.agent_jobs (campaign_id, updated_at, job_id);

CREATE TABLE IF NOT EXISTS public.agent_job_evidence (
    evidence_id BIGSERIAL PRIMARY KEY,
    job_id TEXT NOT NULL
        REFERENCES public.agent_jobs(job_id) ON DELETE RESTRICT,
    attempt INTEGER NOT NULL CHECK (attempt > 0),
    phase TEXT NOT NULL CHECK (
        phase IN (
            'authority', 'context', 'provider', 'tool', 'canonical_commit',
            'completed', 'failed'
        )
    ),
    model_id TEXT NOT NULL,
    runtime_version TEXT NOT NULL,
    prompt_template_hash TEXT NOT NULL CHECK (
        prompt_template_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    tool_schema_hash TEXT NOT NULL CHECK (
        tool_schema_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    retrieval_hash TEXT NOT NULL CHECK (
        retrieval_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    input_hash TEXT NOT NULL CHECK (
        input_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    output_hash TEXT NOT NULL CHECK (
        output_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    input_tokens BIGINT NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens BIGINT NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    latency_ms BIGINT NOT NULL DEFAULT 0 CHECK (latency_ms >= 0),
    tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0),
    linked_event_sequences BIGINT[] NOT NULL DEFAULT '{}',
    visibility_label TEXT NOT NULL,
    retention_until TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (job_id, attempt, phase)
);

ALTER TABLE public.agent_job_evidence
    DROP CONSTRAINT IF EXISTS agent_job_evidence_visibility_label_check,
    ADD CONSTRAINT agent_job_evidence_visibility_label_check CHECK (
        visibility_label IN (
            'public', 'party_visible', 'private_to_player',
            'private_to_group', 'keeper_only', 'ai_internal',
            'system_only', 'spectator_visible', 'spectator_hidden',
            'investigator_private', 'system_private'
        )
    ) NOT VALID,
    DROP CONSTRAINT IF EXISTS agent_job_evidence_retention_check,
    ADD CONSTRAINT agent_job_evidence_retention_check CHECK (
        retention_until > recorded_at
    ) NOT VALID;

ALTER TABLE public.agent_job_evidence
    VALIDATE CONSTRAINT agent_job_evidence_visibility_label_check;
ALTER TABLE public.agent_job_evidence
    VALIDATE CONSTRAINT agent_job_evidence_retention_check;

CREATE TABLE IF NOT EXISTS public.agent_job_approvals (
    approval_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL UNIQUE
        REFERENCES public.agent_jobs(job_id) ON DELETE RESTRICT,
    approval_event_sequence BIGINT NOT NULL UNIQUE
        REFERENCES public.event_store(sequence) ON DELETE RESTRICT,
    approved_by TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (btrim(approved_by) <> ''),
    CHECK (btrim(idempotency_key) <> '')
);

CREATE OR REPLACE FUNCTION public.validate_agent_job_binding()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    workflow public.workflow_instances%ROWTYPE;
    contract public.authority_contracts%ROWTYPE;
    source_event public.event_store%ROWTYPE;
BEGIN
    SELECT * INTO workflow
      FROM public.workflow_instances
     WHERE workflow_id = NEW.job_id;
    IF NOT FOUND
       OR workflow.workflow_type <> 'agent_job'
       OR workflow.campaign_id IS DISTINCT FROM NEW.campaign_id
       OR workflow.state <> 'REQUESTED' THEN
        RAISE EXCEPTION 'agent job must bind a requested durable workflow';
    END IF;

    SELECT * INTO contract
      FROM public.authority_contracts
     WHERE contract_id = NEW.authority_contract_id;
    IF NOT FOUND
       OR contract.campaign_id IS DISTINCT FROM NEW.campaign_id
       OR contract.authority_mode IS DISTINCT FROM NEW.authority_mode
       OR contract.contract_version IS DISTINCT FROM NEW.authority_contract_version
       OR NOT contract.locked
       OR (
           NEW.authority_mode = 'AI_KP'
           AND (
               NEW.agent_kind <> 'ai_keeper_orchestrator'
               OR NEW.actor_id <> contract.authority_owner
           )
       )
       OR (
           NEW.authority_mode = 'HUMAN_KP'
           AND NEW.agent_kind <> 'keeper_copilot'
       ) THEN
        RAISE EXCEPTION 'agent job authority snapshot is not current and immutable';
    END IF;

    SELECT * INTO source_event
      FROM public.event_store
     WHERE sequence = NEW.input_event_sequence;
    IF NOT FOUND
       OR source_event.campaign_id IS DISTINCT FROM NEW.campaign_id
       OR source_event.stream_version IS DISTINCT FROM NEW.input_stream_version
       OR source_event.integrity_status IS DISTINCT FROM 'verified_hmac' THEN
        RAISE EXCEPTION 'agent job input event is not a verified campaign event';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS agent_jobs_validate_binding ON public.agent_jobs;
CREATE TRIGGER agent_jobs_validate_binding
BEFORE INSERT ON public.agent_jobs
FOR EACH ROW EXECUTE FUNCTION public.validate_agent_job_binding();

CREATE OR REPLACE FUNCTION public.protect_agent_job_binding()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.job_id IS DISTINCT FROM OLD.job_id
       OR NEW.campaign_id IS DISTINCT FROM OLD.campaign_id
       OR NEW.actor_id IS DISTINCT FROM OLD.actor_id
       OR NEW.agent_kind IS DISTINCT FROM OLD.agent_kind
       OR NEW.authority_contract_id IS DISTINCT FROM OLD.authority_contract_id
       OR NEW.authority_mode IS DISTINCT FROM OLD.authority_mode
       OR NEW.authority_contract_version IS DISTINCT FROM OLD.authority_contract_version
       OR NEW.input_event_sequence IS DISTINCT FROM OLD.input_event_sequence
       OR NEW.input_stream_version IS DISTINCT FROM OLD.input_stream_version
       OR NEW.visibility_scope IS DISTINCT FROM OLD.visibility_scope
       OR NEW.rag_snapshot_id IS DISTINCT FROM OLD.rag_snapshot_id
       OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
       OR NEW.provider_type IS DISTINCT FROM OLD.provider_type
       OR NEW.model_id IS DISTINCT FROM OLD.model_id
       OR NEW.model_artifact_sha256 IS DISTINCT FROM OLD.model_artifact_sha256
       OR NEW.route_authorization_event_id
          IS DISTINCT FROM OLD.route_authorization_event_id
       OR NEW.prompt_template_id IS DISTINCT FROM OLD.prompt_template_id
       OR NEW.prompt_template_version IS DISTINCT FROM OLD.prompt_template_version
       OR NEW.tool_schema_version IS DISTINCT FROM OLD.tool_schema_version
       OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
       OR NEW.deadline_at IS DISTINCT FROM OLD.deadline_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'agent job authority, input, visibility, and route binding is immutable';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS agent_jobs_protect_binding ON public.agent_jobs;
CREATE TRIGGER agent_jobs_protect_binding
BEFORE UPDATE ON public.agent_jobs
FOR EACH ROW EXECUTE FUNCTION public.protect_agent_job_binding();

CREATE OR REPLACE FUNCTION public.validate_agent_job_approval()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    job public.agent_jobs%ROWTYPE;
    workflow public.workflow_instances%ROWTYPE;
    approval_event public.event_store%ROWTYPE;
BEGIN
    SELECT * INTO job
      FROM public.agent_jobs
     WHERE job_id = NEW.job_id;
    SELECT * INTO workflow
      FROM public.workflow_instances
     WHERE workflow_id = NEW.job_id;
    SELECT * INTO approval_event
      FROM public.event_store
     WHERE sequence = NEW.approval_event_sequence;
    IF job.job_id IS NULL
       OR workflow.workflow_id IS NULL
       OR workflow.state <> 'AWAITING_TOOL'
       OR approval_event.sequence IS NULL
       OR job.authority_mode <> 'HUMAN_KP'
       OR approval_event.campaign_id IS DISTINCT FROM job.campaign_id
       OR approval_event.event_type <> 'AgentDraftApproved'
       OR approval_event.integrity_status <> 'verified_hmac'
       OR approval_event.authority_mode <> 'human_kp'
       OR approval_event.authority_contract_id
          IS DISTINCT FROM job.authority_contract_id
       OR approval_event.authority_contract_version
          IS DISTINCT FROM job.authority_contract_version
       OR approval_event.resource_type <> 'agent_job'
       OR approval_event.resource_id IS DISTINCT FROM job.job_id
       OR approval_event.authenticated_actor_role <> 'human_keeper'
       OR approval_event.authenticated_actor_id IS DISTINCT FROM NEW.approved_by
       OR approval_event.authority_owner IS DISTINCT FROM NEW.approved_by
       OR approval_event.idempotency_key
          IS DISTINCT FROM NEW.idempotency_key THEN
        RAISE EXCEPTION 'agent draft approval is not a bound human keeper command';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS agent_job_approvals_validate
    ON public.agent_job_approvals;
CREATE TRIGGER agent_job_approvals_validate
BEFORE INSERT ON public.agent_job_approvals
FOR EACH ROW EXECUTE FUNCTION public.validate_agent_job_approval();

DROP TRIGGER IF EXISTS agent_job_evidence_append_only
    ON public.agent_job_evidence;
CREATE TRIGGER agent_job_evidence_append_only
BEFORE UPDATE OR DELETE ON public.agent_job_evidence
FOR EACH ROW EXECUTE FUNCTION public.reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS agent_job_approvals_append_only
    ON public.agent_job_approvals;
CREATE TRIGGER agent_job_approvals_append_only
BEFORE UPDATE OR DELETE ON public.agent_job_approvals
FOR EACH ROW EXECUTE FUNCTION public.reject_canonical_append_mutation();

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        REVOKE UPDATE ON public.workflow_instances FROM trpg_api_service;
        REVOKE ALL ON public.workflow_transitions FROM trpg_api_service;
        REVOKE USAGE, SELECT
            ON SEQUENCE public.workflow_transitions_transition_id_seq
            FROM trpg_api_service;
        GRANT SELECT, INSERT ON public.workflow_instances
            TO trpg_api_service;
        GRANT SELECT, INSERT ON public.agent_jobs TO trpg_api_service;
        GRANT UPDATE (cancellation_requested_at)
            ON public.agent_jobs TO trpg_api_service;
        GRANT SELECT, INSERT ON public.agent_job_approvals TO trpg_api_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT, UPDATE ON public.agent_jobs TO trpg_worker_service;
        GRANT SELECT, INSERT ON public.agent_job_evidence TO trpg_worker_service;
        GRANT SELECT ON public.agent_job_approvals TO trpg_worker_service;
        GRANT USAGE, SELECT ON SEQUENCE public.agent_job_evidence_evidence_id_seq
            TO trpg_worker_service;
    END IF;
END;
$$;
