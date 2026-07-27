-- Keep Character sheets immutable while projecting canonical Combat health
-- changes into new locked versions. Combat is party-visible, but a Character
-- sheet can remain owner-private; the narrow guard exception below preserves
-- the existing Character visibility envelope while still requiring the exact
-- HMAC-bound projection target and canonical Combat provenance.

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
    combat_private_projection BOOLEAN := FALSE;
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

    IF canonical.event_type = 'CombatStateRecorded'
       AND projection_relation = 'public.characters'
       AND TG_OP = 'UPDATE'
       AND NEW.visibility_label IS NOT DISTINCT FROM OLD.visibility_label
       AND NEW.visibility_subject IS NOT DISTINCT FROM
           OLD.visibility_subject THEN
        combat_private_projection := TRUE;
    ELSIF canonical.event_type = 'CombatStateRecorded'
       AND projection_relation = 'public.character_sheet_versions'
       AND TG_OP = 'INSERT' THEN
        SELECT EXISTS (
            SELECT 1
              FROM public.characters AS character
             WHERE character.character_id = NEW.character_id
               AND character.campaign_id = NEW.campaign_id
               AND character.visibility_label =
                   NEW.visibility_label
               AND character.visibility_subject =
                   NEW.visibility_subject
        ) INTO combat_private_projection;
    END IF;

    IF canonical.sequence IS NULL
       OR canonical.campaign_id IS DISTINCT FROM NEW.campaign_id
       OR canonical.event_type <> ALL(permitted_event_types)
       OR canonical.event_integrity_version IS DISTINCT FROM 3
       OR canonical.authenticated_actor_role IS DISTINCT FROM 'workflow'
       OR canonical.authenticated_actor_origin IS DISTINCT FROM
          '{"kind":"workload","role":"workflow_engine"}'::JSONB
       OR (
            (
                canonical.visibility_label IS DISTINCT FROM
                    NEW.visibility_label::TEXT
                OR canonical.visibility_subject IS DISTINCT FROM
                    NEW.visibility_subject
            )
            AND NOT combat_private_projection
       )
       OR canonical.fact_provenance_kind IS DISTINCT FROM
          NEW.provenance_kind::TEXT
       OR canonical.fact_provenance_reference IS DISTINCT FROM
          NEW.provenance_reference
       OR canonical.fact_recorded_by IS DISTINCT FROM
          NEW.provenance_recorded_by
       OR canonical.integrity_status IS DISTINCT FROM 'verified_hmac'
       OR canonical.request_hash_source IS DISTINCT FROM 'formal_commit'
       OR NOT COALESCE(exact_projection_target, FALSE)
       OR NOT COALESCE(formal_workflow_decision, FALSE) THEN
        RAISE EXCEPTION
            'core projection does not match a verified canonical event';
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

DROP TRIGGER characters_event_guard ON public.characters;
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterSubmitted,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized,CombatStateRecorded',
    'public.characters', 'character_id'
);

DROP TRIGGER character_sheet_versions_event_guard
    ON public.character_sheet_versions;
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized,CombatStateRecorded',
    'public.character_sheet_versions', 'sheet_version_id'
);

-- The ordinary P08 cleanup intentionally leaves shared Character rows in
-- place. Delete only sheet versions owned by verified Combat targets; replay
-- recreates them in event order and either advances a Character at the source
-- version or preserves one already advanced by later canonical state.
CREATE FUNCTION core_domain.clear_combat_health_sheet_projections(
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
       OR btrim(authorizing_commit_id) = '' THEN
        RAISE EXCEPTION 'invalid Combat health rebuild request';
    END IF;

    PERFORM pg_advisory_xact_lock(
        hashtextextended(
            'p08-projection-rebuild:' || target_campaign_id,
            0
        )
    );
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL
       OR btrim(projection_capability) = '' THEN
        RAISE EXCEPTION 'Combat health rebuild capability missing';
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
           AND (
                event.data_subject_id = 'not_applicable'
                OR EXISTS (
                    SELECT 1
                      FROM public.privacy_subject_keys AS subject_key
                     WHERE subject_key.subject_id = event.data_subject_id
                       AND subject_key.key_reference =
                           event.payload_key_reference
                       AND subject_key.wrapped_key IS NOT NULL
                       AND subject_key.destroyed_at IS NULL
                )
           )
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
                   AND latest.event_type IN (
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
                   AND latest.event_integrity_version = 3
                   AND latest.integrity_status = 'verified_hmac'
                   AND latest.request_hash_source = 'formal_commit'
                   AND latest.event_integrity_hash IS NOT NULL
                   AND latest.payload_json ? 'protected_payload'
                   AND (
                        latest.data_subject_id = 'not_applicable'
                        OR EXISTS (
                            SELECT 1
                              FROM public.privacy_subject_keys AS subject_key
                             WHERE subject_key.subject_id =
                                   latest.data_subject_id
                               AND subject_key.key_reference =
                                   latest.payload_key_reference
                               AND subject_key.wrapped_key IS NOT NULL
                               AND subject_key.destroyed_at IS NULL
                        )
                   )
           )
    ) INTO capability_is_current;
    IF NOT COALESCE(capability_is_current, FALSE) THEN
        RAISE EXCEPTION 'Combat health rebuild capability rejected';
    END IF;

    DELETE FROM public.character_sheet_versions AS sheet
     WHERE sheet.campaign_id = target_campaign_id
       AND EXISTS (
            SELECT 1
              FROM public.event_store AS event
              JOIN public.formal_commits AS formal
                ON formal.campaign_id = event.campaign_id
               AND event.sequence BETWEEN
                   formal.first_event_sequence
                   AND formal.last_event_sequence
               AND formal.status = 'committed'
              JOIN public.canonical_audit_log AS audit
                ON audit.sequence = formal.audit_sequence
              CROSS JOIN LATERAL jsonb_array_elements(
                  event.projection_targets
              ) AS target
             WHERE event.campaign_id = target_campaign_id
               AND event.event_type = 'CombatStateRecorded'
               AND event.event_integrity_version = 3
               AND event.integrity_status = 'verified_hmac'
               AND event.request_hash_source = 'formal_commit'
               AND event.event_integrity_hash IS NOT NULL
               AND event.payload_json ? 'protected_payload'
               AND (
                    event.data_subject_id = 'not_applicable'
                    OR EXISTS (
                        SELECT 1
                          FROM public.privacy_subject_keys AS subject_key
                         WHERE subject_key.subject_id = event.data_subject_id
                           AND subject_key.key_reference =
                               event.payload_key_reference
                           AND subject_key.wrapped_key IS NOT NULL
                           AND subject_key.destroyed_at IS NULL
                    )
               )
               AND event.authenticated_actor_role = 'workflow'
               AND event.authenticated_actor_origin =
                   '{"kind":"workload","role":"workflow_engine"}'::JSONB
               AND audit.campaign_id = event.campaign_id
               AND audit.resource_type = event.resource_type
               AND audit.resource_id = event.resource_id
               AND audit.action = 'write_official_state'
               AND audit.requested_role = 'workflow'
               AND audit.decision = 'PERMIT'
               AND target ->> 'relation' =
                   'public.character_sheet_versions'
               AND target ->> 'row_id' = sheet.sheet_version_id
               AND sheet.last_event_sequence = event.sequence
       );
END;
$$;

REVOKE ALL ON FUNCTION
    core_domain.clear_combat_health_sheet_projections(TEXT, TEXT)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.clear_combat_health_sheet_projections(TEXT, TEXT)
            TO trpg_api_service;
    END IF;
END;
$least_privilege$;
