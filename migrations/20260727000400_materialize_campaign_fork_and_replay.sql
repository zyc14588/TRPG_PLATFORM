-- P08 repair: a fork is a child-owned, replayable initial-state
-- materialization, not merely a snapshot row attached to the parent.
--
-- Existing version-1 rows remain readable as historical lineage. Every new
-- application write uses materialization_version = 2 and must be owned by the
-- child campaign.

ALTER TABLE public.campaign_forks
    ADD COLUMN materialization_version SMALLINT NOT NULL DEFAULT 1;

ALTER TABLE public.campaign_forks
    ALTER COLUMN materialization_version SET DEFAULT 2,
    DROP CONSTRAINT campaign_forks_check,
    DROP CONSTRAINT campaign_forks_verified_snapshot_copy,
    ADD CONSTRAINT campaign_forks_materialization_shape CHECK (
        materialization_version = 1
        AND campaign_id = parent_campaign_id
        OR
        materialization_version = 2
        AND campaign_id = child_campaign_id
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
    );

CREATE TABLE public.campaign_fork_materializations (
    fork_id TEXT PRIMARY KEY REFERENCES public.campaign_forks(fork_id)
        DEFERRABLE INITIALLY DEFERRED,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    parent_campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    source_session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    child_session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    child_scenario_id TEXT NOT NULL REFERENCES public.scenarios(scenario_id)
        DEFERRABLE INITIALLY DEFERRED,
    source_snapshot_hash TEXT NOT NULL CHECK (
        source_snapshot_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    child_snapshot_hash TEXT NOT NULL CHECK (
        child_snapshot_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    child_state_json TEXT NOT NULL CHECK (
        jsonb_typeof(child_state_json::JSONB) = 'object'
    ),
    materialized_row_count BIGINT NOT NULL CHECK (materialized_row_count > 0),
    batch_count BIGINT NOT NULL CHECK (batch_count > 0),
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (campaign_id <> parent_campaign_id),
    CHECK (
        child_snapshot_hash = 'sha256:' || encode(
            sha256(convert_to(child_state_json, 'UTF8')),
            'hex'
        )
    )
);

CREATE INDEX campaign_fork_materializations_parent_idx
    ON public.campaign_fork_materializations(parent_campaign_id, source_session_id);

DROP TRIGGER scenarios_event_guard ON public.scenarios;
CREATE TRIGGER scenarios_event_guard
BEFORE INSERT OR UPDATE ON public.scenarios
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ScenarioImported,CampaignForkMaterialized',
    'public.scenarios', 'scenario_id'
);

DROP TRIGGER characters_event_guard ON public.characters;
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterSubmitted,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized',
    'public.characters', 'character_id'
);

DROP TRIGGER character_sheet_versions_event_guard
    ON public.character_sheet_versions;
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized',
    'public.character_sheet_versions', 'sheet_version_id'
);

DROP TRIGGER sessions_event_guard ON core_domain.sessions;
CREATE TRIGGER sessions_event_guard
BEFORE INSERT OR UPDATE ON core_domain.sessions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'SessionStarted,SessionStateChanged,SceneSwitched,CampaignForkMaterialized',
    'core_domain.sessions', 'session_id'
);

DROP TRIGGER scenes_event_guard ON public.scenes;
CREATE TRIGGER scenes_event_guard
BEFORE INSERT OR UPDATE ON public.scenes
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'SessionStarted,SessionStateChanged,SceneSwitched,CampaignForkMaterialized',
    'public.scenes', 'scene_id'
);

CREATE TRIGGER campaign_fork_materializations_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_fork_materializations
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignForkMaterializationRecorded',
    'public.campaign_fork_materializations', 'fork_id'
);

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT, INSERT ON public.campaign_fork_materializations
            TO trpg_api_service;
        REVOKE UPDATE, DELETE ON public.campaign_fork_materializations
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT ON public.campaign_fork_materializations
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
