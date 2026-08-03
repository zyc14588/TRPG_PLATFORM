-- Bind AgentJobRequested and its durable workflow to one formal canonical
-- transaction. A workflow/job conflict now aborts the Event Store append;
-- workers can no longer observe a canonical request without a claimable row.

CREATE FUNCTION core_domain.agent_job_request_projection_id(
    projection JSONB
)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
    SELECT 'projection_' || encode(
        sha256(convert_to(projection::TEXT, 'UTF8')),
        'hex'
    )
$$;

CREATE FUNCTION core_domain.apply_agent_job_request(
    target_commit_id TEXT,
    projection JSONB
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public, core_domain
AS $$
DECLARE
    formal public.formal_commits%ROWTYPE;
    audit public.canonical_audit_log%ROWTYPE;
    source_event public.event_store%ROWTYPE;
    projection_id TEXT;
    projection_capability TEXT;
    projection_capability_hash TEXT;
    event_count BIGINT;
    source_event_sequence BIGINT;
    contract_version BIGINT;
    deadline_unix_ms BIGINT;
BEGIN
    IF target_commit_id IS NULL
       OR btrim(target_commit_id) = ''
       OR jsonb_typeof(projection) <> 'object'
       OR projection ->> 'kind' IS DISTINCT FROM 'AGENT_JOB_REQUEST'
       OR btrim(COALESCE(projection ->> 'job_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'campaign_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'actor_id', '')) = ''
       OR projection ->> 'agent_kind' NOT IN (
           'ai_keeper_orchestrator', 'keeper_copilot'
       )
       OR btrim(COALESCE(projection ->> 'authority_contract_id', '')) = ''
       OR projection ->> 'authority_mode' NOT IN ('AI_KP', 'HUMAN_KP')
       OR jsonb_typeof(projection -> 'visibility_scope') <> 'object'
       OR btrim(COALESCE(projection ->> 'rag_snapshot_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'provider_id', '')) = ''
       OR projection ->> 'provider_type' NOT IN ('cloud', 'ollama', 'llama_cpp')
       OR btrim(COALESCE(projection ->> 'model_id', '')) = ''
       OR COALESCE(projection ->> 'model_artifact_sha256', '')
          !~ '^sha256:[0-9a-f]{64}$'
       OR btrim(COALESCE(projection ->> 'route_authorization_event_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'prompt_template_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'prompt_template_version', '')) = ''
       OR btrim(COALESCE(projection ->> 'tool_schema_version', '')) = ''
       OR btrim(COALESCE(projection ->> 'idempotency_key', '')) = ''
       OR NOT (projection ? 'input') THEN
        RAISE EXCEPTION 'invalid agent job request projection';
    END IF;

    BEGIN
        contract_version :=
            (projection ->> 'authority_contract_version')::BIGINT;
        deadline_unix_ms := (projection ->> 'deadline_unix_ms')::BIGINT;
    EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN
        RAISE EXCEPTION 'invalid agent job request numeric binding';
    END;
    IF contract_version <= 0 OR deadline_unix_ms <= 0 THEN
        RAISE EXCEPTION 'invalid agent job request numeric binding';
    END IF;

    SELECT * INTO formal
      FROM public.formal_commits
     WHERE commit_id = target_commit_id
       AND status = 'committed'
     FOR SHARE;
    IF formal.commit_id IS NULL
       OR formal.campaign_id IS DISTINCT FROM projection ->> 'campaign_id'
       OR formal.stream_id IS DISTINCT FROM projection ->> 'job_id'
       OR formal.idempotency_key IS DISTINCT FROM projection ->> 'idempotency_key'
       OR formal.idempotency_operation IS DISTINCT FROM 'canonical_commit' THEN
        RAISE EXCEPTION 'agent job formal commit mismatch';
    END IF;

    SELECT * INTO audit
      FROM public.canonical_audit_log
     WHERE sequence = formal.audit_sequence;
    IF audit.sequence IS NULL
       OR audit.campaign_id IS DISTINCT FROM formal.campaign_id
       OR audit.resource_type IS DISTINCT FROM 'agent_job'
       OR audit.resource_id IS DISTINCT FROM projection ->> 'job_id'
       OR audit.action IS DISTINCT FROM 'write_official_state'
       OR audit.requested_role IS DISTINCT FROM 'workflow'
       OR audit.decision IS DISTINCT FROM 'PERMIT' THEN
        RAISE EXCEPTION 'agent job policy evidence missing';
    END IF;

    SELECT count(*), min(event.sequence)
      INTO event_count, source_event_sequence
      FROM public.event_store AS event
     WHERE event.sequence BETWEEN
           formal.first_event_sequence AND formal.last_event_sequence
       AND event.event_type = 'AgentJobRequested';
    IF event_count <> 1 OR source_event_sequence IS NULL THEN
        RAISE EXCEPTION 'invalid agent job canonical event batch';
    END IF;
    SELECT * INTO source_event
      FROM public.event_store
     WHERE sequence = source_event_sequence;
    IF source_event.campaign_id IS DISTINCT FROM projection ->> 'campaign_id'
       OR source_event.stream_id IS DISTINCT FROM projection ->> 'job_id'
       OR source_event.resource_type IS DISTINCT FROM 'agent_job'
       OR source_event.resource_id IS DISTINCT FROM projection ->> 'job_id'
       OR source_event.authority_contract_id
          IS DISTINCT FROM projection ->> 'authority_contract_id'
       OR source_event.authority_contract_version IS DISTINCT FROM contract_version
       OR source_event.authority_mode IS DISTINCT FROM
          lower(projection ->> 'authority_mode')
       OR source_event.visibility_label IS DISTINCT FROM
          projection -> 'visibility_scope' ->> 'output_label'
       OR source_event.integrity_status IS DISTINCT FROM 'verified_hmac'
       OR source_event.request_hash_source IS DISTINCT FROM 'formal_commit' THEN
        RAISE EXCEPTION 'agent job canonical event binding mismatch';
    END IF;

    projection_id := core_domain.agent_job_request_projection_id(projection);
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL OR btrim(projection_capability) = '' THEN
        RAISE EXCEPTION 'agent job projection capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );
    IF NOT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(source_event.projection_targets) AS target
         WHERE target ->> 'relation' = 'core_domain.agent_job_request'
           AND target ->> 'row_id' = projection_id
           AND target ->> 'capability_hash' = projection_capability_hash
    ) THEN
        RAISE EXCEPTION 'agent job projection is not HMAC-bound';
    END IF;

    INSERT INTO public.workflow_instances (
        workflow_id, campaign_id, workflow_type, state, version,
        input_json, wake_at, next_attempt_at
    ) VALUES (
        projection ->> 'job_id',
        projection ->> 'campaign_id',
        'agent_job',
        'REQUESTED',
        0,
        jsonb_build_object(
            'input_event_sequence', source_event.sequence,
            'input_stream_version', source_event.stream_version
        )::TEXT,
        now(),
        now()
    );

    INSERT INTO public.agent_jobs (
        job_id, campaign_id, actor_id, agent_kind,
        authority_contract_id, authority_mode, authority_contract_version,
        input_event_sequence, input_stream_version, visibility_scope,
        rag_snapshot_id, provider_id, provider_type, model_id,
        model_artifact_sha256, route_authorization_event_id,
        prompt_template_id, prompt_template_version, tool_schema_version,
        idempotency_key, deadline_at
    ) VALUES (
        projection ->> 'job_id',
        projection ->> 'campaign_id',
        projection ->> 'actor_id',
        projection ->> 'agent_kind',
        projection ->> 'authority_contract_id',
        projection ->> 'authority_mode',
        contract_version,
        source_event.sequence,
        source_event.stream_version,
        projection -> 'visibility_scope',
        projection ->> 'rag_snapshot_id',
        projection ->> 'provider_id',
        projection ->> 'provider_type',
        projection ->> 'model_id',
        projection ->> 'model_artifact_sha256',
        projection ->> 'route_authorization_event_id',
        projection ->> 'prompt_template_id',
        projection ->> 'prompt_template_version',
        projection ->> 'tool_schema_version',
        projection ->> 'idempotency_key',
        to_timestamp(deadline_unix_ms::DOUBLE PRECISION / 1000.0)
    );
END;
$$;

REVOKE ALL ON FUNCTION
    core_domain.agent_job_request_projection_id(JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.apply_agent_job_request(TEXT, JSONB)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.agent_job_request_projection_id(JSONB)
            TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.apply_agent_job_request(TEXT, JSONB)
            TO trpg_canonical_service;
    END IF;
END;
$least_privilege$;
