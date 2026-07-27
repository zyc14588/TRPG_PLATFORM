-- P08 forward-only hardening: every opaque Combat/Chase server roll is
-- consumed once across all aggregates, while the consumption projection
-- remains rebuildable from verified canonical gameplay events.

CREATE TABLE public.gameplay_roll_consumptions (
    roll_id TEXT PRIMARY KEY CHECK (
        btrim(roll_id) <> '' AND length(roll_id) <= 128
    ),
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    aggregate_kind TEXT NOT NULL CHECK (
        aggregate_kind IN ('COMBAT', 'CHASE', 'GROWTH')
    ),
    aggregate_id TEXT NOT NULL CHECK (
        btrim(aggregate_id) <> '' AND length(aggregate_id) <= 128
    ),
    roll_kind TEXT NOT NULL CHECK (
        roll_kind IN (
            'ATTACKER_PERCENTILE',
            'DEFENDER_PERCENTILE',
            'DAMAGE',
            'MEDICAL_PERCENTILE',
            'CHASE_PARTICIPANT_PERCENTILE',
            'GROWTH_PERCENTILE',
            'GROWTH_INCREASE_D10'
        )
    ),
    random_source TEXT NOT NULL CHECK (
        random_source = 'SERVER_OS_CSPRNG'
    ),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (
        btrim(visibility_subject) <> ''
    ),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (
        btrim(provenance_reference) <> ''
    ),
    provenance_recorded_by TEXT NOT NULL CHECK (
        btrim(provenance_recorded_by) <> ''
    ),
    last_event_sequence BIGINT NOT NULL
        REFERENCES public.event_store(sequence)
);

CREATE INDEX gameplay_roll_consumptions_aggregate_idx
    ON public.gameplay_roll_consumptions(
        campaign_id, aggregate_kind, aggregate_id, last_event_sequence
    );

-- The fork command deliberately releases its projection transaction before
-- the canonical commit so concurrent forks cannot reserve every connection
-- and deadlock the pool. Canonical lineage uniqueness, rather than a
-- long-lived advisory-lock connection, now serializes child initialization.
CREATE UNIQUE INDEX event_store_one_fork_lineage_per_child_idx
    ON public.event_store(campaign_id)
    WHERE event_type = 'CampaignForkRecorded'
      AND integrity_status = 'verified_hmac'
      AND request_hash_source = 'formal_commit';

-- Growth replay must temporarily return a character to the last non-growth
-- projection before deleting and recreating growth-owned sheets. Preserve the
-- default monotonic guard for every ordinary write. The narrow exception
-- still requires the secret commit capability, an exact canonical character
-- target, an all-growth suffix, and values derived from canonical targets and
-- pre-growth sheets.
CREATE OR REPLACE FUNCTION public.enforce_core_projection_event()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public, core_domain
AS $$
DECLARE
    canonical public.event_store%ROWTYPE;
    permitted_event_types TEXT[];
    formal_workflow_decision BOOLEAN;
    projection_relation TEXT;
    projection_row_id TEXT;
    projection_capability TEXT;
    projection_capability_hash TEXT;
    exact_projection_target BOOLEAN;
    projection_rebuild_scope TEXT;
    growth_rewind_allowed BOOLEAN := FALSE;
BEGIN
    permitted_event_types := string_to_array(TG_ARGV[0], ',');
    projection_relation := TG_ARGV[1];
    projection_row_id := to_jsonb(NEW) ->> TG_ARGV[2];
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    projection_rebuild_scope := current_setting(
        'trpg.p08_projection_rebuild',
        TRUE
    );
    IF projection_capability IS NOT NULL
       AND btrim(projection_capability) <> '' THEN
        projection_capability_hash := 'sha256:' || encode(
            sha256(convert_to(projection_capability, 'UTF8')),
            'hex'
        );
    END IF;
    SELECT * INTO canonical
      FROM public.event_store
     WHERE sequence = NEW.last_event_sequence;

    SELECT EXISTS (
        SELECT 1
          FROM public.formal_commits AS formal
          JOIN public.canonical_audit_log AS audit
            ON audit.sequence = formal.audit_sequence
         WHERE NEW.last_event_sequence BETWEEN
               formal.first_event_sequence AND formal.last_event_sequence
           AND formal.campaign_id = canonical.campaign_id
           AND audit.campaign_id = canonical.campaign_id
           AND audit.resource_type = canonical.resource_type
           AND audit.resource_id = canonical.resource_id
           AND audit.action = 'write_official_state'
           AND audit.requested_role = 'workflow'
           AND audit.decision = 'PERMIT'
    ) INTO formal_workflow_decision;

    SELECT EXISTS (
        SELECT 1
          FROM jsonb_array_elements(
                   COALESCE(canonical.projection_targets, '[]'::JSONB)
               ) AS target
         WHERE target ->> 'relation' = projection_relation
           AND target ->> 'row_id' = projection_row_id
           AND target ->> 'capability_hash' = projection_capability_hash
    ) INTO exact_projection_target;

    IF canonical.sequence IS NULL
       OR canonical.campaign_id IS DISTINCT FROM NEW.campaign_id
       OR canonical.event_type <> ALL(permitted_event_types)
       OR canonical.event_integrity_version IS DISTINCT FROM 3
       OR canonical.authenticated_actor_role IS DISTINCT FROM 'workflow'
       OR canonical.authenticated_actor_origin IS DISTINCT FROM
          '{"kind":"workload","role":"workflow_engine"}'::JSONB
       OR canonical.visibility_label IS DISTINCT FROM NEW.visibility_label::TEXT
       OR canonical.visibility_subject IS DISTINCT FROM NEW.visibility_subject
       OR canonical.fact_provenance_kind IS DISTINCT FROM NEW.provenance_kind::TEXT
       OR canonical.fact_provenance_reference IS DISTINCT FROM NEW.provenance_reference
       OR canonical.fact_recorded_by IS DISTINCT FROM NEW.provenance_recorded_by
       OR canonical.integrity_status IS DISTINCT FROM 'verified_hmac'
       OR canonical.request_hash_source IS DISTINCT FROM 'formal_commit'
       OR NOT COALESCE(exact_projection_target, FALSE)
       OR NOT COALESCE(formal_workflow_decision, FALSE) THEN
        RAISE EXCEPTION 'core projection does not match a verified canonical event';
    END IF;

    IF TG_OP = 'UPDATE'
       AND NEW.last_event_sequence <= OLD.last_event_sequence THEN
        SELECT
            projection_rebuild_scope = 'character_growth'
            AND projection_relation = 'public.characters'
            AND EXISTS (
                SELECT 1
                  FROM public.event_store AS old_event
                 WHERE old_event.sequence = OLD.last_event_sequence
                   AND old_event.campaign_id = NEW.campaign_id
                   AND old_event.event_type = 'CharacterGrowthApplied'
                   AND old_event.integrity_status = 'verified_hmac'
                   AND old_event.request_hash_source = 'formal_commit'
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               old_event.projection_targets
                          ) AS old_target
                         WHERE old_target ->> 'relation' =
                               'public.characters'
                           AND old_target ->> 'row_id' = projection_row_id
                   )
            )
            AND NOT EXISTS (
                SELECT 1
                  FROM public.event_store AS suffix_event
                 WHERE suffix_event.campaign_id = NEW.campaign_id
                   AND suffix_event.sequence > NEW.last_event_sequence
                   AND suffix_event.sequence <= OLD.last_event_sequence
                   AND suffix_event.integrity_status = 'verified_hmac'
                   AND suffix_event.request_hash_source = 'formal_commit'
                   AND suffix_event.event_type <> 'CharacterGrowthApplied'
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               suffix_event.projection_targets
                          ) AS suffix_target
                         WHERE suffix_target ->> 'relation' =
                               'public.characters'
                           AND suffix_target ->> 'row_id' = projection_row_id
                   )
            )
            AND (to_jsonb(NEW) ->> 'version')::BIGINT = (
                SELECT count(*)
                  FROM public.event_store AS version_event
                 WHERE version_event.campaign_id = NEW.campaign_id
                   AND version_event.sequence <= NEW.last_event_sequence
                   AND version_event.integrity_status = 'verified_hmac'
                   AND version_event.request_hash_source = 'formal_commit'
                   AND EXISTS (
                        SELECT 1
                          FROM jsonb_array_elements(
                               version_event.projection_targets
                          ) AS version_target
                         WHERE version_target ->> 'relation' =
                               'public.characters'
                           AND version_target ->> 'row_id' = projection_row_id
                   )
            )
            AND (to_jsonb(NEW) ->> 'current_sheet_version')::BIGINT = (
                SELECT max(sheet.version)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.campaign_id = NEW.campaign_id
                   AND sheet.character_id = projection_row_id
                   AND sheet.last_event_sequence <= NEW.last_event_sequence
            )
          INTO growth_rewind_allowed;
        IF NOT COALESCE(growth_rewind_allowed, FALSE) THEN
            RAISE EXCEPTION 'core projection event sequence must advance';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER gameplay_roll_consumptions_event_guard
BEFORE INSERT OR UPDATE ON public.gameplay_roll_consumptions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CombatStateRecorded,ChaseStateRecorded,CharacterGrowthApplied',
    'public.gameplay_roll_consumptions',
    'aggregate_id'
);

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT, INSERT ON public.gameplay_roll_consumptions
            TO trpg_api_service;
        REVOKE UPDATE, DELETE ON public.gameplay_roll_consumptions
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT ON public.gameplay_roll_consumptions
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
