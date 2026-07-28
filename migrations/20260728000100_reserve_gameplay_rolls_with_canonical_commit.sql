-- Reserve every formal Combat, Chase, and Growth roll in the same database
-- transaction as its canonical event. State projections remain rebuildable,
-- but a failed or cancelled projector can no longer make an already recorded
-- opaque server roll available to another aggregate.

CREATE FUNCTION core_domain.gameplay_roll_reservation_projection_id(
    projection JSONB
)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
    SELECT 'gameplay_roll_reservation_' || encode(
        sha256(convert_to(projection::TEXT, 'UTF8')),
        'hex'
    )
$$;

CREATE FUNCTION core_domain.reserve_gameplay_roll_consumptions(
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
    aggregate_kind TEXT;
    aggregate_id TEXT;
    consumption JSONB;
    inserted_rows BIGINT;
BEGIN
    IF target_commit_id IS NULL
       OR btrim(target_commit_id) = ''
       OR jsonb_typeof(projection) <> 'object'
       OR jsonb_typeof(projection -> 'consumptions') <> 'array'
       OR jsonb_array_length(projection -> 'consumptions') = 0 THEN
        RAISE EXCEPTION 'invalid gameplay roll reservation request';
    END IF;

    SELECT * INTO formal
      FROM public.formal_commits
     WHERE commit_id = target_commit_id
       AND status = 'committed'
     FOR SHARE;
    IF formal.commit_id IS NULL THEN
        RAISE EXCEPTION 'gameplay roll formal commit missing';
    END IF;

    SELECT * INTO audit
      FROM public.canonical_audit_log
     WHERE sequence = formal.audit_sequence;
    IF audit.sequence IS NULL
       OR audit.campaign_id IS DISTINCT FROM formal.campaign_id
       OR audit.action IS DISTINCT FROM 'write_official_state'
       OR audit.requested_role IS DISTINCT FROM 'workflow'
       OR audit.decision IS DISTINCT FROM 'PERMIT' THEN
        RAISE EXCEPTION 'gameplay roll policy evidence missing';
    END IF;

    SELECT count(*), min(sequence)
      INTO event_count, event_sequence
      FROM public.event_store
     WHERE sequence BETWEEN
           formal.first_event_sequence AND formal.last_event_sequence;
    IF event_count <> 1 OR event_sequence IS NULL THEN
        RAISE EXCEPTION 'gameplay roll event batch must contain one event';
    END IF;
    SELECT * INTO canonical
      FROM public.event_store
     WHERE sequence = event_sequence;

    aggregate_kind := projection ->> 'aggregate_kind';
    aggregate_id := projection ->> 'aggregate_id';
    IF aggregate_kind IS NULL
       OR projection ->> 'campaign_id' IS DISTINCT FROM formal.campaign_id
       OR formal.stream_id IS DISTINCT FROM aggregate_id
       OR canonical.campaign_id IS DISTINCT FROM formal.campaign_id
       OR canonical.stream_id IS DISTINCT FROM aggregate_id
       OR canonical.resource_id IS DISTINCT FROM aggregate_id
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
       OR (
            aggregate_kind = 'COMBAT'
            AND (
                canonical.event_type IS DISTINCT FROM 'CombatStateRecorded'
                OR canonical.resource_type IS DISTINCT FROM 'combat_state'
            )
       )
       OR (
            aggregate_kind = 'CHASE'
            AND (
                canonical.event_type IS DISTINCT FROM 'ChaseStateRecorded'
                OR canonical.resource_type IS DISTINCT FROM 'chase_state'
            )
       )
       OR (
            aggregate_kind = 'GROWTH'
            AND (
                canonical.event_type IS DISTINCT FROM
                    'CharacterGrowthApplied'
                OR canonical.resource_type IS DISTINCT FROM 'growth'
            )
       )
       OR aggregate_kind NOT IN ('COMBAT', 'CHASE', 'GROWTH') THEN
        RAISE EXCEPTION 'gameplay roll canonical event mismatch';
    END IF;

    projection_id :=
        core_domain.gameplay_roll_reservation_projection_id(projection);
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL
       OR btrim(projection_capability) = '' THEN
        RAISE EXCEPTION 'gameplay roll projection capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );
    IF NOT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(canonical.projection_targets) AS target
         WHERE target ->> 'relation' =
               'core_domain.gameplay_roll_reservation'
           AND target ->> 'row_id' = projection_id
           AND target ->> 'capability_hash' =
               projection_capability_hash
    ) OR NOT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(canonical.projection_targets) AS target
         WHERE target ->> 'relation' =
               'public.gameplay_roll_consumptions'
           AND target ->> 'row_id' = aggregate_id
           AND target ->> 'capability_hash' =
               projection_capability_hash
    ) THEN
        RAISE EXCEPTION 'gameplay roll reservation is not HMAC-bound';
    END IF;

    IF jsonb_array_length(projection -> 'consumptions') IS DISTINCT FROM (
        SELECT count(DISTINCT item ->> 'roll_id')
          FROM jsonb_array_elements(
               projection -> 'consumptions'
          ) AS item
    ) OR EXISTS (
        SELECT 1
          FROM jsonb_array_elements(
               projection -> 'consumptions'
          ) AS item
         WHERE jsonb_typeof(item) <> 'object'
            OR item ->> 'roll_id' IS NULL
            OR btrim(item ->> 'roll_id') = ''
            OR length(item ->> 'roll_id') > 128
            OR (
                aggregate_kind = 'COMBAT'
                AND item ->> 'roll_kind' NOT IN (
                    'ATTACKER_PERCENTILE',
                    'DEFENDER_PERCENTILE',
                    'DAMAGE',
                    'MEDICAL_PERCENTILE'
                )
            )
            OR (
                aggregate_kind = 'CHASE'
                AND item ->> 'roll_kind' IS DISTINCT FROM
                    'CHASE_PARTICIPANT_PERCENTILE'
            )
            OR (
                aggregate_kind = 'GROWTH'
                AND item ->> 'roll_kind' NOT IN (
                    'GROWTH_PERCENTILE',
                    'GROWTH_INCREASE_D10'
                )
            )
    ) THEN
        RAISE EXCEPTION 'invalid gameplay roll reservation shape';
    END IF;

    FOR consumption IN
        SELECT value
          FROM jsonb_array_elements(
               projection -> 'consumptions'
          )
    LOOP
        INSERT INTO public.gameplay_roll_consumptions (
            roll_id, campaign_id, aggregate_kind, aggregate_id, roll_kind,
            random_source, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            consumption ->> 'roll_id',
            formal.campaign_id,
            aggregate_kind,
            aggregate_id,
            consumption ->> 'roll_kind',
            'SERVER_OS_CSPRNG',
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            canonical.sequence
        )
        ON CONFLICT (roll_id) DO NOTHING;
        GET DIAGNOSTICS inserted_rows = ROW_COUNT;
        IF inserted_rows <> 1 THEN
            RAISE EXCEPTION 'gameplay roll already consumed';
        END IF;
    END LOOP;
END;
$$;

REVOKE ALL ON FUNCTION
    core_domain.gameplay_roll_reservation_projection_id(JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.reserve_gameplay_roll_consumptions(TEXT, JSONB)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_roles
         WHERE rolname = 'trpg_canonical_service'
    ) THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.reserve_gameplay_roll_consumptions(TEXT, JSONB)
            TO trpg_canonical_service;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_roles
         WHERE rolname = 'trpg_api_service'
    ) THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.gameplay_roll_reservation_projection_id(JSONB)
            TO trpg_api_service;
    END IF;
END;
$least_privilege$;
