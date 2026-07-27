-- P08 repair: production projection connections intentionally cannot DELETE
-- official read models. Rebuilds receive one capability-gated, target-scoped
-- cleanup operation instead of broad table privileges.

CREATE FUNCTION core_domain.clear_p08_rebuildable_projections(
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
    verified_targets JSONB;
BEGIN
    IF target_campaign_id IS NULL
       OR btrim(target_campaign_id) = ''
       OR authorizing_commit_id IS NULL
       OR btrim(authorizing_commit_id) = '' THEN
        RAISE EXCEPTION 'invalid P08 projection rebuild request';
    END IF;

    -- Serialize cleanup with every ordinary P08 projector. The Rust
    -- repository already holds this lock; advisory locks are transaction
    -- re-entrant, so direct function calls receive the same boundary.
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
        RAISE EXCEPTION 'P08 projection rebuild capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );

    -- The capability must be the preimage bound to the campaign's latest
    -- verified P08 event. It is derived from the canonical integrity secret
    -- and is never stored in the database.
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
           AND event.event_type IN (
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
        RAISE EXCEPTION 'P08 projection rebuild capability rejected';
    END IF;

    -- Collect only exact shared-table row identities named by committed,
    -- verified P08 events. Campaign-local P08-only tables are cleared in full
    -- below so corrupted or ghost read-model rows are also removed.
    SELECT COALESCE(
               jsonb_object_agg(grouped.target_relation, grouped.target_rows),
               '{}'::JSONB
           )
      INTO verified_targets
      FROM (
        SELECT exact_target.target_relation,
               jsonb_agg(
                   exact_target.target_row_id
                   ORDER BY exact_target.target_row_id
               ) AS target_rows
          FROM (
            SELECT DISTINCT
                   target ->> 'relation' AS target_relation,
                   target ->> 'row_id' AS target_row_id
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
               AND event.event_type IN (
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
               AND (
                    event.event_type = 'CampaignForkMaterialized'
                    AND target ->> 'relation' IN (
                        'public.scenarios',
                        'public.characters',
                        'public.character_sheet_versions',
                        'core_domain.sessions',
                        'public.scenes'
                    )
                    OR event.event_type = 'CharacterGrowthApplied'
                    AND target ->> 'relation' =
                        'public.character_sheet_versions'
               )
               AND btrim(COALESCE(target ->> 'row_id', '')) <> ''
          ) AS exact_target
         GROUP BY exact_target.target_relation
      ) AS grouped;

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

    DELETE FROM public.character_sheet_versions
     WHERE campaign_id = target_campaign_id
       AND sheet_version_id IN (
            SELECT jsonb_array_elements_text(
                COALESCE(
                    verified_targets ->
                        'public.character_sheet_versions',
                    '[]'::JSONB
                )
            )
       );
    DELETE FROM public.characters
     WHERE campaign_id = target_campaign_id
       AND character_id IN (
            SELECT jsonb_array_elements_text(
                COALESCE(
                    verified_targets -> 'public.characters',
                    '[]'::JSONB
                )
            )
       );
    DELETE FROM public.scenes
     WHERE campaign_id = target_campaign_id
       AND scene_id IN (
            SELECT jsonb_array_elements_text(
                COALESCE(
                    verified_targets -> 'public.scenes',
                    '[]'::JSONB
                )
            )
       );
    DELETE FROM core_domain.sessions
     WHERE campaign_id = target_campaign_id
       AND session_id IN (
            SELECT jsonb_array_elements_text(
                COALESCE(
                    verified_targets -> 'core_domain.sessions',
                    '[]'::JSONB
                )
            )
       );
    DELETE FROM public.scenarios
     WHERE campaign_id = target_campaign_id
       AND scenario_id IN (
            SELECT jsonb_array_elements_text(
                COALESCE(
                    verified_targets -> 'public.scenarios',
                    '[]'::JSONB
                )
            )
       );
    DELETE FROM public.campaign_forks
     WHERE campaign_id = target_campaign_id;
END;
$$;

REVOKE ALL ON FUNCTION
    core_domain.clear_p08_rebuildable_projections(TEXT, TEXT)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.clear_p08_rebuildable_projections(TEXT, TEXT)
            TO trpg_api_service;
    END IF;
END;
$least_privilege$;
