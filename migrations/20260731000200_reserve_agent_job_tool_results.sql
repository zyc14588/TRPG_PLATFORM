-- A tool side effect must have a durable, immutable receipt before an Agent
-- job may advance to canonical commit. This closes the crash window between a
-- server-generated COC7 roll and persistence of the workflow transition.

CREATE TABLE public.agent_job_tool_receipts (
    job_id TEXT NOT NULL
        REFERENCES public.agent_jobs(job_id) ON DELETE RESTRICT,
    idempotency_key TEXT NOT NULL,
    tool_name TEXT NOT NULL CHECK (tool_name = 'request_skill_check'),
    request_json JSONB NOT NULL CHECK (jsonb_typeof(request_json) = 'object'),
    execution_id TEXT NOT NULL UNIQUE CHECK (btrim(execution_id) <> ''),
    result_json JSONB NOT NULL CHECK (jsonb_typeof(result_json) = 'object'),
    result_hash TEXT NOT NULL CHECK (
        result_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (job_id, idempotency_key),
    CHECK (btrim(idempotency_key) <> '')
);

CREATE OR REPLACE FUNCTION public.validate_agent_job_tool_receipt()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    job public.agent_jobs%ROWTYPE;
    authoritative_target INTEGER;
    expected_roll INTEGER;
    expected_success_level TEXT;
BEGIN
    SELECT * INTO job
      FROM public.agent_jobs
     WHERE job_id = NEW.job_id;
    SELECT CASE
             WHEN jsonb_typeof(sheet.sheet_json -> 'skills'
                               -> (NEW.request_json ->> 'skill_name')) = 'number'
             THEN (
                 sheet.sheet_json -> 'skills'
                 ->> (NEW.request_json ->> 'skill_name')
             )::INTEGER
           END
      INTO authoritative_target
      FROM public.characters AS character
      JOIN public.character_sheet_versions AS sheet
        ON sheet.character_id = character.character_id
       AND sheet.version = character.current_sheet_version
     WHERE character.campaign_id = job.campaign_id
       AND character.character_id = NEW.request_json ->> 'character_id'
       AND character.state = 'APPROVED'
       AND sheet.locked;

    expected_roll := CASE
        WHEN (NEW.result_json ->> 'selected_tens_digit')::INTEGER = 0
         AND (NEW.result_json ->> 'ones_digit')::INTEGER = 0
        THEN 100
        ELSE (NEW.result_json ->> 'selected_tens_digit')::INTEGER * 10
             + (NEW.result_json ->> 'ones_digit')::INTEGER
    END;
    expected_success_level := CASE
        WHEN expected_roll = 1 THEN 'CRITICAL'
        WHEN (authoritative_target < 50 AND expected_roll >= 96)
          OR (authoritative_target >= 50 AND expected_roll = 100)
        THEN 'FUMBLE'
        WHEN expected_roll <= authoritative_target / 5 THEN 'EXTREME'
        WHEN expected_roll <= authoritative_target / 2 THEN 'HARD'
        WHEN expected_roll <= authoritative_target THEN 'REGULAR'
        ELSE 'FAILURE'
    END;

    IF job.job_id IS NULL
       OR job.authority_mode <> 'AI_KP'
       OR job.agent_kind <> 'ai_keeper_orchestrator'
       OR NEW.idempotency_key <> job.idempotency_key || ':tool'
       OR (
           SELECT count(*) FROM jsonb_object_keys(NEW.request_json)
       ) <> 3
       OR NEW.request_json ->> 'adjustment' <> 'NONE'
       OR btrim(COALESCE(NEW.request_json ->> 'character_id', '')) = ''
       OR btrim(COALESCE(NEW.request_json ->> 'skill_name', '')) = ''
       OR authoritative_target IS NULL
       OR authoritative_target NOT BETWEEN 0 AND 100
       OR (
           SELECT count(*) FROM jsonb_object_keys(NEW.result_json)
       ) <> 11
       OR NEW.result_json ->> 'adjustment' <> 'NONE'
       OR NEW.result_json ->> 'character_id'
          IS DISTINCT FROM NEW.request_json ->> 'character_id'
       OR NEW.result_json ->> 'skill_name'
          IS DISTINCT FROM NEW.request_json ->> 'skill_name'
       OR NEW.result_json ->> 'random_source' <> 'SERVER_OS_CSPRNG'
       OR NEW.result_json ->> 'schema_version' <> '1'
       OR NEW.result_json ->> 'roll_id' IS DISTINCT FROM NEW.execution_id
       OR (NEW.result_json ->> 'target')::INTEGER
          IS DISTINCT FROM authoritative_target
       OR (NEW.result_json ->> 'selected_tens_digit')::INTEGER NOT BETWEEN 0 AND 9
       OR (NEW.result_json ->> 'ones_digit')::INTEGER NOT BETWEEN 0 AND 9
       OR (NEW.result_json ->> 'roll')::INTEGER IS DISTINCT FROM expected_roll
       OR NEW.result_json ->> 'success_level'
          IS DISTINCT FROM expected_success_level THEN
        RAISE EXCEPTION 'agent tool receipt is not bound to an authorized server skill check';
    END IF;
    RETURN NEW;
EXCEPTION
    WHEN invalid_text_representation OR numeric_value_out_of_range THEN
        RAISE EXCEPTION 'agent tool receipt has invalid dice fields';
END;
$$;

CREATE TRIGGER agent_job_tool_receipts_validate
BEFORE INSERT ON public.agent_job_tool_receipts
FOR EACH ROW EXECUTE FUNCTION public.validate_agent_job_tool_receipt();

CREATE TRIGGER agent_job_tool_receipts_append_only
BEFORE UPDATE OR DELETE ON public.agent_job_tool_receipts
FOR EACH ROW EXECUTE FUNCTION public.reject_canonical_append_mutation();

REVOKE ALL ON public.agent_job_tool_receipts FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        REVOKE ALL ON public.agent_job_tool_receipts FROM trpg_api_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        REVOKE ALL ON public.agent_job_tool_receipts FROM trpg_canonical_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        REVOKE ALL ON public.agent_job_tool_receipts FROM trpg_worker_service;
        GRANT SELECT, INSERT ON public.agent_job_tool_receipts
            TO trpg_worker_service;
        -- The production worker also owns the rebuildable canonical
        -- projection loop. Keep Event Store append custody separate while
        -- granting only the materialization operations that loop executes.
        GRANT INSERT, DELETE ON public.canonical_event_projection
            TO trpg_worker_service;
        GRANT INSERT, DELETE ON public.projection_checkpoint
            TO trpg_worker_service;
        GRANT UPDATE (
            version, last_event_sequence, projection_hash, rebuilt_at
        ) ON public.projection_checkpoint TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
