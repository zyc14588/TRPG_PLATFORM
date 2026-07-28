-- P08 forward-only repair:
-- 1. reserve one canonical ending per Session in the Event Store transaction;
-- 2. permit capability-gated cleanup when canonical P08 history is empty.

CREATE TABLE core_domain.session_ending_reservations (
    session_id TEXT PRIMARY KEY
        REFERENCES core_domain.sessions(session_id),
    campaign_id TEXT NOT NULL
        REFERENCES public.campaigns(campaign_id),
    ending_event_id TEXT NOT NULL UNIQUE CHECK (
        btrim(ending_event_id) <> '' AND length(ending_event_id) <= 128
    ),
    commit_id TEXT NOT NULL UNIQUE
        REFERENCES public.formal_commits(commit_id),
    event_sequence BIGINT NOT NULL UNIQUE
        REFERENCES public.event_store(sequence),
    reserved_at TIMESTAMPTZ NOT NULL
);

COMMENT ON TABLE core_domain.session_ending_reservations IS
    'Append-only canonical uniqueness guard; ending_events remains rebuildable';

-- Preserve the uniqueness already represented by pre-repair projections.
INSERT INTO core_domain.session_ending_reservations (
    session_id, campaign_id, ending_event_id, commit_id,
    event_sequence, reserved_at
)
SELECT ending.session_id, ending.campaign_id, ending.ending_event_id,
       formal.commit_id, event.sequence, event.recorded_at
  FROM public.ending_events AS ending
  JOIN public.event_store AS event
    ON event.sequence = ending.last_event_sequence
   AND event.campaign_id = ending.campaign_id
   AND event.event_type = 'EndingRecorded'
   AND event.integrity_status = 'verified_hmac'
   AND event.request_hash_source = 'formal_commit'
  JOIN public.formal_commits AS formal
    ON event.sequence BETWEEN
       formal.first_event_sequence AND formal.last_event_sequence
   AND formal.campaign_id = event.campaign_id
   AND formal.status = 'committed';

CREATE TRIGGER session_ending_reservations_append_only
BEFORE UPDATE OR DELETE ON core_domain.session_ending_reservations
FOR EACH ROW EXECUTE FUNCTION public.reject_canonical_append_mutation();

CREATE TRIGGER session_ending_reservations_no_truncate
BEFORE TRUNCATE ON core_domain.session_ending_reservations
FOR EACH STATEMENT EXECUTE FUNCTION public.reject_canonical_append_mutation();

CREATE FUNCTION core_domain.session_ending_reservation_projection_id(
    projection JSONB
)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
    SELECT 'session_ending_reservation_' || encode(
        sha256(convert_to(projection::TEXT, 'UTF8')),
        'hex'
    )
$$;

CREATE FUNCTION core_domain.reserve_session_ending(
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
    canonical public.event_store%ROWTYPE;
    projection_id TEXT;
    projection_capability TEXT;
    projection_capability_hash TEXT;
    event_count BIGINT;
    event_sequence BIGINT;
    inserted_rows BIGINT;
BEGIN
    IF target_commit_id IS NULL
       OR btrim(target_commit_id) = ''
       OR jsonb_typeof(projection) <> 'object'
       OR btrim(COALESCE(projection ->> 'campaign_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'session_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'ending_event_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'ending_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'summary', '')) = ''
       OR length(projection ->> 'summary') > 1024
       OR jsonb_typeof(projection -> 'ended_at_unix_ms') <> 'number'
       OR btrim(COALESCE(projection ->> 'visibility_label', '')) = ''
       OR btrim(COALESCE(projection ->> 'visibility_subject', '')) = ''
       OR btrim(COALESCE(projection ->> 'provenance_kind', '')) = ''
       OR btrim(COALESCE(projection ->> 'provenance_reference', '')) = ''
       OR btrim(COALESCE(projection ->> 'provenance_recorded_by', '')) = ''
    THEN
        RAISE EXCEPTION 'invalid Session ending reservation request';
    END IF;

    SELECT * INTO formal
      FROM public.formal_commits
     WHERE commit_id = target_commit_id
       AND status = 'committed'
     FOR SHARE;
    IF formal.commit_id IS NULL THEN
        RAISE EXCEPTION 'Session ending formal commit missing';
    END IF;

    SELECT * INTO audit
      FROM public.canonical_audit_log
     WHERE sequence = formal.audit_sequence;
    IF audit.sequence IS NULL
       OR audit.campaign_id IS DISTINCT FROM formal.campaign_id
       OR audit.action IS DISTINCT FROM 'write_official_state'
       OR audit.requested_role IS DISTINCT FROM 'workflow'
       OR audit.decision IS DISTINCT FROM 'PERMIT'
    THEN
        RAISE EXCEPTION 'Session ending policy evidence missing';
    END IF;

    SELECT count(*), min(sequence)
      INTO event_count, event_sequence
      FROM public.event_store
     WHERE sequence BETWEEN
           formal.first_event_sequence AND formal.last_event_sequence;
    IF event_count <> 1 OR event_sequence IS NULL THEN
        RAISE EXCEPTION 'Session ending batch must contain one event';
    END IF;
    SELECT * INTO canonical
      FROM public.event_store
     WHERE sequence = event_sequence;

    IF formal.expected_version IS DISTINCT FROM 0
       OR projection ->> 'campaign_id' IS DISTINCT FROM formal.campaign_id
       OR projection ->> 'ending_event_id' IS DISTINCT FROM formal.stream_id
       OR canonical.campaign_id IS DISTINCT FROM formal.campaign_id
       OR canonical.stream_id IS DISTINCT FROM formal.stream_id
       OR canonical.resource_id IS DISTINCT FROM formal.stream_id
       OR canonical.resource_type IS DISTINCT FROM 'ending'
       OR canonical.event_type IS DISTINCT FROM 'EndingRecorded'
       OR canonical.integrity_status IS DISTINCT FROM 'verified_hmac'
       OR canonical.request_hash_source IS DISTINCT FROM 'formal_commit'
       OR canonical.event_integrity_version IS DISTINCT FROM 3
       OR canonical.authenticated_actor_role IS DISTINCT FROM 'workflow'
       OR canonical.visibility_label IS DISTINCT FROM
          projection ->> 'visibility_label'
       OR canonical.visibility_subject IS DISTINCT FROM
          projection ->> 'visibility_subject'
       OR canonical.fact_provenance_kind IS DISTINCT FROM
          projection ->> 'provenance_kind'
       OR canonical.fact_provenance_reference IS DISTINCT FROM
          projection ->> 'provenance_reference'
       OR canonical.fact_recorded_by IS DISTINCT FROM
          projection ->> 'provenance_recorded_by'
    THEN
        RAISE EXCEPTION 'Session ending canonical event mismatch';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM core_domain.sessions AS session_projection
          JOIN public.scenarios AS scenario
            ON scenario.scenario_id = session_projection.scenario_id
           AND scenario.campaign_id = session_projection.campaign_id
          CROSS JOIN LATERAL jsonb_array_elements(
              scenario.document_json -> 'endings'
          ) AS defined_ending
         WHERE session_projection.session_id =
               projection ->> 'session_id'
           AND session_projection.campaign_id =
               projection ->> 'campaign_id'
           AND session_projection.state = 'ENDED'
           AND defined_ending ->> 'id' = projection ->> 'ending_id'
    ) THEN
        RAISE EXCEPTION 'Session ending authority mismatch';
    END IF;

    projection_id :=
        core_domain.session_ending_reservation_projection_id(projection);
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL
       OR btrim(projection_capability) = ''
    THEN
        RAISE EXCEPTION 'Session ending projection capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );
    IF NOT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(canonical.projection_targets) AS target
         WHERE target ->> 'relation' =
               'core_domain.session_ending_reservation'
           AND target ->> 'row_id' = projection_id
           AND target ->> 'capability_hash' =
               projection_capability_hash
    ) OR NOT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(canonical.projection_targets) AS target
         WHERE target ->> 'relation' = 'public.ending_events'
           AND target ->> 'row_id' = projection ->> 'ending_event_id'
           AND target ->> 'capability_hash' =
               projection_capability_hash
    ) THEN
        RAISE EXCEPTION 'Session ending reservation is not HMAC-bound';
    END IF;

    INSERT INTO core_domain.session_ending_reservations (
        session_id, campaign_id, ending_event_id, commit_id,
        event_sequence, reserved_at
    ) VALUES (
        projection ->> 'session_id',
        projection ->> 'campaign_id',
        projection ->> 'ending_event_id',
        formal.commit_id,
        canonical.sequence,
        canonical.recorded_at
    )
    ON CONFLICT DO NOTHING;
    GET DIAGNOSTICS inserted_rows = ROW_COUNT;
    IF inserted_rows <> 1 THEN
        RAISE EXCEPTION 'Session ending already reserved';
    END IF;
END;
$$;

CREATE FUNCTION core_domain.clear_empty_p08_rebuildable_projections(
    target_campaign_id TEXT,
    authorizing_commit_id TEXT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public, core_domain
AS $$
DECLARE
    projection_capability TEXT;
    projection_capability_hash TEXT;
    capability_is_current BOOLEAN;
BEGIN
    IF target_campaign_id IS NULL
       OR btrim(target_campaign_id) = ''
       OR authorizing_commit_id IS NULL
       OR btrim(authorizing_commit_id) = ''
    THEN
        RAISE EXCEPTION 'invalid empty P08 projection rebuild request';
    END IF;

    PERFORM pg_advisory_xact_lock(
        hashtextextended(
            'p08-projection-rebuild:' || target_campaign_id,
            0
        )
    );

    -- Never use the empty-history path when any canonical P08 event exists,
    -- including malformed history that the replay reader must surface.
    IF EXISTS (
        SELECT 1
          FROM public.event_store
         WHERE campaign_id = target_campaign_id
           AND event_type IN (
                'CombatStateRecorded',
                'ChaseStateRecorded',
                'ReconsiderationRequested',
                'ReconsiderationReviewed',
                'ReconsiderationUpheld',
                'ReconsiderationCorrected',
                'CampaignForkRecorded',
                'CampaignForkMaterializationRecorded',
                'CampaignForkMaterialized',
                'EndingRecorded',
                'CharacterGrowthApplied'
           )
    ) THEN
        RAISE EXCEPTION 'canonical P08 history is not empty';
    END IF;

    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL
       OR btrim(projection_capability) = ''
    THEN
        RAISE EXCEPTION 'empty P08 rebuild capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );

    SELECT EXISTS (
        SELECT 1
          FROM public.formal_commits AS formal
          JOIN public.canonical_audit_log AS audit
            ON audit.sequence = formal.audit_sequence
          JOIN public.event_store AS event
            ON event.sequence BETWEEN
               formal.first_event_sequence AND formal.last_event_sequence
          CROSS JOIN LATERAL jsonb_array_elements(
              event.projection_targets
          ) AS target
         WHERE formal.commit_id = authorizing_commit_id
           AND formal.campaign_id = target_campaign_id
           AND formal.status = 'committed'
           AND event.campaign_id = target_campaign_id
           AND event.event_integrity_version = 3
           AND event.integrity_status = 'verified_hmac'
           AND event.request_hash_source = 'formal_commit'
           AND event.event_integrity_hash IS NOT NULL
           AND event.payload_json ? 'protected_payload'
           AND event.authenticated_actor_role = 'workflow'
           AND event.authenticated_actor_origin =
               '{"kind":"workload","role":"workflow_engine"}'::JSONB
           AND audit.campaign_id = event.campaign_id
           AND audit.resource_type = event.resource_type
           AND audit.resource_id = event.resource_id
           AND audit.action = 'write_official_state'
           AND audit.requested_role = 'workflow'
           AND audit.decision = 'PERMIT'
           AND target ->> 'capability_hash' =
               projection_capability_hash
           AND event.sequence = (
                SELECT max(latest.sequence)
                  FROM public.event_store AS latest
                 WHERE latest.campaign_id = target_campaign_id
                   AND latest.event_integrity_version = 3
                   AND latest.integrity_status = 'verified_hmac'
                   AND latest.request_hash_source = 'formal_commit'
                   AND latest.event_integrity_hash IS NOT NULL
                   AND jsonb_array_length(latest.projection_targets) > 0
           )
    ) INTO capability_is_current;
    IF NOT COALESCE(capability_is_current, FALSE) THEN
        RAISE EXCEPTION 'empty P08 rebuild capability rejected';
    END IF;

    SET CONSTRAINTS ALL DEFERRED;
    DELETE FROM public.growth_events
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.gameplay_roll_consumptions
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.reconsiderations
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.ending_events
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.combat_states
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.chase_states
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.campaign_fork_npc_states
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.campaign_fork_clues
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.campaign_fork_public_events
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.campaign_fork_materializations
     WHERE campaign_id = target_campaign_id;
    DELETE FROM public.campaign_forks
     WHERE campaign_id = target_campaign_id;
END;
$$;

REVOKE ALL ON TABLE core_domain.session_ending_reservations
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.session_ending_reservation_projection_id(JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.reserve_session_ending(TEXT, JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.clear_empty_p08_rebuildable_projections(TEXT, TEXT)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_roles
         WHERE rolname = 'trpg_canonical_service'
    ) THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.reserve_session_ending(TEXT, JSONB)
            TO trpg_canonical_service;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_roles
         WHERE rolname = 'trpg_api_service'
    ) THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT SELECT ON core_domain.session_ending_reservations
            TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.session_ending_reservation_projection_id(JSONB)
            TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.clear_empty_p08_rebuildable_projections(TEXT, TEXT)
            TO trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE, TRUNCATE
            ON core_domain.session_ending_reservations
            FROM trpg_api_service;
    END IF;
END;
$least_privilege$;
