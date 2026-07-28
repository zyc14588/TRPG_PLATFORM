-- P08 forward-only migration: persistent combat/chase state, append-only
-- reconsideration outcomes, verified public fork snapshots, and conclusion
-- growth. Existing Event Store history remains immutable.

ALTER TABLE public.campaign_forks
    ADD COLUMN child_snapshot_hash TEXT,
    ADD COLUMN copy_scope_json JSONB,
    ADD COLUMN snapshot_json JSONB,
    ADD CONSTRAINT campaign_forks_verified_snapshot_copy CHECK (
        child_snapshot_hash IS NULL
        AND copy_scope_json IS NULL
        AND snapshot_json IS NULL
        OR
        child_snapshot_hash = source_snapshot_hash
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

ALTER TABLE public.reconsiderations
    ADD COLUMN review_workflow_version SMALLINT NOT NULL DEFAULT 1
        CHECK (review_workflow_version IN (1, 2)),
    ADD COLUMN review_summary TEXT,
    ADD COLUMN outcome TEXT CHECK (outcome IN ('UPHELD', 'CORRECTED')),
    ADD COLUMN corrected_event_type TEXT,
    ADD COLUMN corrected_payload JSONB,
    ADD CONSTRAINT reconsiderations_v2_append_only_shape CHECK (
        review_workflow_version = 1
        OR
        state = 'REQUESTED'
        AND review_summary IS NULL
        AND outcome IS NULL
        AND resolution IS NULL
        AND corrected_event_type IS NULL
        AND corrected_payload IS NULL
        OR
        state = 'REVIEWED'
        AND btrim(review_summary) <> ''
        AND outcome IS NULL
        AND resolution IS NULL
        AND corrected_event_type IS NULL
        AND corrected_payload IS NULL
        OR
        state = 'RESOLVED'
        AND btrim(review_summary) <> ''
        AND outcome = 'UPHELD'
        AND btrim(resolution) <> ''
        AND corrected_event_type IS NULL
        AND corrected_payload IS NULL
        OR
        state = 'RESOLVED'
        AND btrim(review_summary) <> ''
        AND outcome = 'CORRECTED'
        AND btrim(resolution) <> ''
        AND btrim(corrected_event_type) <> ''
        AND jsonb_typeof(corrected_payload) = 'object'
    );

CREATE TABLE public.combat_states (
    combat_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    status TEXT NOT NULL CHECK (status IN ('ONGOING', 'ENDED')),
    round BIGINT NOT NULL CHECK (round > 0),
    current_turn_index BIGINT NOT NULL CHECK (current_turn_index >= 0),
    state_json JSONB NOT NULL CHECK (jsonb_typeof(state_json) = 'object'),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, combat_id)
);

CREATE TABLE public.chase_states (
    chase_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    status TEXT NOT NULL CHECK (status IN ('ONGOING', 'ESCAPED', 'CAUGHT')),
    range_band SMALLINT NOT NULL CHECK (range_band BETWEEN 0 AND 5),
    segment BIGINT NOT NULL CHECK (segment > 0),
    state_json JSONB NOT NULL CHECK (jsonb_typeof(state_json) = 'object'),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, chase_id)
);

CREATE TABLE public.ending_events (
    ending_event_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    ending_id TEXT NOT NULL CHECK (btrim(ending_id) <> ''),
    summary TEXT NOT NULL CHECK (
        btrim(summary) <> '' AND length(summary) <= 1024
    ),
    ended_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (session_id)
);

CREATE TABLE public.growth_events (
    growth_event_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    ending_event_id TEXT NOT NULL REFERENCES public.ending_events(ending_event_id),
    character_id TEXT NOT NULL REFERENCES public.characters(character_id),
    source_sheet_version_id TEXT NOT NULL
        REFERENCES public.character_sheet_versions(sheet_version_id),
    new_sheet_version_id TEXT NOT NULL
        REFERENCES public.character_sheet_versions(sheet_version_id)
        DEFERRABLE INITIALLY DEFERRED,
    skill_name TEXT NOT NULL CHECK (
        btrim(skill_name) <> '' AND length(skill_name) <= 128
    ),
    skill_before SMALLINT NOT NULL CHECK (skill_before BETWEEN 0 AND 99),
    improvement_check_roll SMALLINT NOT NULL CHECK (
        improvement_check_roll BETWEEN 1 AND 100
    ),
    increase_roll SMALLINT CHECK (increase_roll BETWEEN 1 AND 10),
    skill_after SMALLINT NOT NULL CHECK (skill_after BETWEEN 0 AND 99),
    server_roll_id TEXT NOT NULL UNIQUE CHECK (btrim(server_roll_id) <> ''),
    increase_roll_id TEXT UNIQUE CHECK (
        increase_roll_id IS NULL OR btrim(increase_roll_id) <> ''
    ),
    random_source TEXT NOT NULL CHECK (random_source = 'SERVER_OS_CSPRNG'),
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (source_sheet_version_id <> new_sheet_version_id),
    CHECK (
        (increase_roll IS NULL AND increase_roll_id IS NULL)
        OR (
            increase_roll IS NOT NULL
            AND increase_roll_id IS NOT NULL
            AND increase_roll_id <> server_roll_id
        )
    ),
    CHECK (
        increase_roll IS NULL
        AND skill_after = skill_before
        AND (
            skill_before = 99
            OR improvement_check_roll <= skill_before
            AND improvement_check_roll < 96
        )
        OR
        increase_roll IS NOT NULL
        AND skill_before < 99
        AND (
            improvement_check_roll > skill_before
            OR improvement_check_roll >= 96
        )
        AND skill_after = LEAST(skill_before + increase_roll, 99)
    ),
    UNIQUE (ending_event_id, character_id, skill_name)
);

CREATE INDEX combat_states_session_status_idx
    ON public.combat_states(session_id, status);
CREATE INDEX chase_states_session_status_idx
    ON public.chase_states(session_id, status);
CREATE INDEX growth_events_character_idx
    ON public.growth_events(character_id, last_event_sequence);

DROP TRIGGER character_sheet_versions_event_guard
    ON public.character_sheet_versions;
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied',
    'public.character_sheet_versions', 'sheet_version_id'
);

DROP TRIGGER characters_event_guard ON public.characters;
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterSubmitted,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied',
    'public.characters', 'character_id'
);

DROP TRIGGER reconsiderations_event_guard ON public.reconsiderations;
CREATE TRIGGER reconsiderations_event_guard
BEFORE INSERT OR UPDATE ON public.reconsiderations
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ReconsiderationRequested,ReconsiderationReviewed,ReconsiderationUpheld,ReconsiderationCorrected',
    'public.reconsiderations', 'reconsideration_id'
);

CREATE TRIGGER combat_states_event_guard
BEFORE INSERT OR UPDATE ON public.combat_states
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CombatStateRecorded', 'public.combat_states', 'combat_id'
);

CREATE TRIGGER chase_states_event_guard
BEFORE INSERT OR UPDATE ON public.chase_states
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ChaseStateRecorded', 'public.chase_states', 'chase_id'
);

CREATE TRIGGER ending_events_event_guard
BEFORE INSERT OR UPDATE ON public.ending_events
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'EndingRecorded', 'public.ending_events', 'ending_event_id'
);

CREATE TRIGGER growth_events_event_guard
BEFORE INSERT OR UPDATE ON public.growth_events
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterGrowthApplied', 'public.growth_events', 'growth_event_id'
);

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT, INSERT, UPDATE ON
            public.combat_states, public.chase_states
            TO trpg_api_service;
        GRANT SELECT, INSERT ON
            public.ending_events, public.growth_events
            TO trpg_api_service;
        REVOKE DELETE ON
            public.combat_states, public.chase_states,
            public.ending_events, public.growth_events
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT ON
            public.combat_states, public.chase_states,
            public.ending_events, public.growth_events
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
