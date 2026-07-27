-- P08 repair: PostgreSQL CHECK constraints accept UNKNOWN, so nullable v2
-- evidence fields must be rejected explicitly. Keep legacy version-1 fork
-- rows readable while making every current fork and reconsideration shape
-- complete.

ALTER TABLE public.campaign_forks
    DROP CONSTRAINT campaign_forks_materialization_shape,
    ADD CONSTRAINT campaign_forks_materialization_shape CHECK (
        (
            materialization_version = 1
            AND campaign_id = parent_campaign_id
        )
        OR (
            materialization_version = 2
            AND campaign_id = child_campaign_id
            AND child_snapshot_hash IS NOT NULL
            AND copy_scope_json IS NOT NULL
            AND snapshot_json IS NOT NULL
            AND child_snapshot_hash ~ '^sha256:[0-9a-f]{64}$'
            AND jsonb_typeof(copy_scope_json) = 'array'
            AND copy_scope_json = '[
                "CHARACTER_STATE",
                "PUBLIC_EVENTS",
                "DISCOVERED_CLUES",
                "WORLD_STATE",
                "NPC_STATE",
                "SCENE_STATE",
                "COMBAT_STATE",
                "CHASE_STATE",
                "CONCLUSION_STATE"
            ]'::JSONB
            AND jsonb_typeof(snapshot_json) = 'object'
            AND snapshot_json ->> 'schema_version' = '1'
            AND snapshot_json -> 'excluded_private_scopes' @> '[
                "KEEPER_NOTES",
                "HIDDEN_CLUES",
                "PRIVATE_MESSAGES",
                "AI_INTERNAL_MEMORY"
            ]'::JSONB
        )
    );

ALTER TABLE public.reconsiderations
    DROP CONSTRAINT reconsiderations_v2_append_only_shape,
    ADD CONSTRAINT reconsiderations_v2_append_only_shape CHECK (
        review_workflow_version = 1
        OR (
            review_workflow_version = 2
            AND (
                (
                    state = 'REQUESTED'
                    AND review_summary IS NULL
                    AND outcome IS NULL
                    AND resolution IS NULL
                    AND corrected_event_type IS NULL
                    AND corrected_payload IS NULL
                )
                OR (
                    state = 'REVIEWED'
                    AND review_summary IS NOT NULL
                    AND btrim(review_summary) <> ''
                    AND outcome IS NULL
                    AND resolution IS NULL
                    AND corrected_event_type IS NULL
                    AND corrected_payload IS NULL
                )
                OR (
                    state = 'RESOLVED'
                    AND review_summary IS NOT NULL
                    AND btrim(review_summary) <> ''
                    AND outcome IS NOT NULL
                    AND outcome = 'UPHELD'
                    AND resolution IS NOT NULL
                    AND btrim(resolution) <> ''
                    AND corrected_event_type IS NULL
                    AND corrected_payload IS NULL
                )
                OR (
                    state = 'RESOLVED'
                    AND review_summary IS NOT NULL
                    AND btrim(review_summary) <> ''
                    AND outcome IS NOT NULL
                    AND outcome = 'CORRECTED'
                    AND resolution IS NOT NULL
                    AND btrim(resolution) <> ''
                    AND corrected_event_type IS NOT NULL
                    AND btrim(corrected_event_type) <> ''
                    AND corrected_payload IS NOT NULL
                    AND jsonb_typeof(corrected_payload) = 'object'
                )
            )
        )
    );
