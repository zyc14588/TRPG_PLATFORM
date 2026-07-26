-- P06 forward-only migration: canonical core-domain projections.
--
-- `public.sessions` is the immutable P02 identity/login session table. Gameplay
-- sessions therefore live in `core_domain.sessions`, preserving both exact
-- aggregate naming and the established identity boundary.

CREATE SCHEMA core_domain;
REVOKE ALL PRIVILEGES ON SCHEMA core_domain FROM PUBLIC;

CREATE DOMAIN core_domain.visibility_label AS TEXT
CHECK (
    VALUE IN (
        'public', 'party_visible', 'private_to_player',
        'private_to_group', 'keeper_only', 'ai_internal', 'system_only',
        'spectator_visible', 'spectator_hidden',
        'investigator_private', 'system_private'
    )
);

CREATE DOMAIN core_domain.provenance_kind AS TEXT
CHECK (
    VALUE IN (
        'user_statement', 'human_keeper_statement', 'rules_engine_decision',
        'tool_result', 'agent_proposal', 'imported_source', 'system_fixture'
    )
);

-- Projection target identities are intentionally separate from the encrypted
-- business payload. They reveal no more than the projection primary keys,
-- while event-integrity version 3 HMAC-binds the complete list. Each target
-- also carries a one-way verifier for a commit-scoped capability known only to
-- the trusted canonical repository, so a database role cannot consume either
-- another row's target or the intended row's target with forged contents.
ALTER TABLE public.event_store
    ADD COLUMN projection_targets JSONB NOT NULL DEFAULT '[]'::JSONB
    CHECK (jsonb_typeof(projection_targets) = 'array');

ALTER TABLE public.event_store
    ALTER COLUMN event_integrity_version SET DEFAULT 3,
    DROP CONSTRAINT event_store_integrity_status_valid,
    ADD CONSTRAINT event_store_integrity_status_valid CHECK (
        integrity_status = 'verified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_version IN (2, 3)
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unverified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_version = 1
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unsigned'
        AND request_hash_source = 'historical_unavailable'
        AND event_integrity_version = 0
        AND event_integrity_hash IS NULL
    );

COMMENT ON COLUMN public.event_store.projection_targets IS
    'HMAC-bound version-3 allow-list of relation/row_id projection targets';

CREATE TABLE public.campaigns (
    campaign_id TEXT PRIMARY KEY,
    owner_user_id TEXT NOT NULL REFERENCES public.users(user_id),
    authority_contract_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL CHECK (btrim(title) <> '' AND length(title) <= 512),
    state TEXT NOT NULL CHECK (
        state IN ('DRAFT', 'READY', 'ACTIVE', 'ENDED', 'ARCHIVED')
    ),
    version BIGINT NOT NULL CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL,
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence)
);

ALTER TABLE public.authority_contracts
    ADD CONSTRAINT authority_contracts_contract_campaign_key
    UNIQUE (contract_id, campaign_id);

-- Identity owns memberships and Authority Contracts and may establish either
-- before a core-domain projection exists. Keep that dependency one-way:
-- Campaign creation must bind the exact contract/campaign pair, while P06
-- never makes the P02 identity tables depend on a rebuildable projection.
ALTER TABLE public.campaigns
    ADD CONSTRAINT campaigns_authority_contract_fkey
    FOREIGN KEY (authority_contract_id, campaign_id)
    REFERENCES public.authority_contracts(contract_id, campaign_id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE public.rooms (
    room_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    name TEXT NOT NULL CHECK (btrim(name) <> '' AND length(name) <= 512),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, name)
);

CREATE TABLE public.scenarios (
    scenario_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    ruleset_id TEXT NOT NULL CHECK (btrim(ruleset_id) <> ''),
    format_version TEXT NOT NULL CHECK (btrim(format_version) <> ''),
    content_hash TEXT NOT NULL CHECK (
        content_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    document_json JSONB NOT NULL CHECK (jsonb_typeof(document_json) = 'object'),
    validated BOOLEAN NOT NULL CHECK (validated),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, content_hash)
);

CREATE TABLE public.characters (
    character_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    owner_user_id TEXT NOT NULL REFERENCES public.users(user_id),
    display_name TEXT NOT NULL CHECK (
        btrim(display_name) <> '' AND length(display_name) <= 512
    ),
    state TEXT NOT NULL CHECK (state IN ('DRAFT', 'SUBMITTED', 'APPROVED')),
    current_sheet_version BIGINT NOT NULL CHECK (current_sheet_version > 0),
    initial_version_locked BOOLEAN NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, owner_user_id, display_name)
);

CREATE TABLE public.character_sheet_versions (
    sheet_version_id TEXT PRIMARY KEY,
    character_id TEXT NOT NULL REFERENCES public.characters(character_id),
    version BIGINT NOT NULL CHECK (version > 0),
    sheet_json JSONB NOT NULL CHECK (jsonb_typeof(sheet_json) = 'object'),
    locked BOOLEAN NOT NULL,
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (character_id, version)
);

CREATE TABLE core_domain.sessions (
    session_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    room_id TEXT NOT NULL REFERENCES public.rooms(room_id),
    scenario_id TEXT NOT NULL REFERENCES public.scenarios(scenario_id),
    state TEXT NOT NULL CHECK (state IN ('SCHEDULED', 'ACTIVE', 'PAUSED', 'ENDED')),
    active_scene_id TEXT,
    started_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (
        (state = 'SCHEDULED' AND started_at IS NULL AND ended_at IS NULL)
        OR
        (state IN ('ACTIVE', 'PAUSED') AND started_at IS NOT NULL AND ended_at IS NULL)
        OR
        (state = 'ENDED' AND started_at IS NOT NULL AND ended_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX sessions_one_live_per_room_idx
    ON core_domain.sessions(campaign_id, room_id)
    WHERE state IN ('ACTIVE', 'PAUSED');

CREATE TABLE public.scenes (
    scene_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    scenario_id TEXT NOT NULL REFERENCES public.scenarios(scenario_id),
    room_id TEXT NOT NULL REFERENCES public.rooms(room_id),
    scene_key TEXT NOT NULL CHECK (btrim(scene_key) <> ''),
    name TEXT NOT NULL CHECK (btrim(name) <> '' AND length(name) <= 512),
    state TEXT NOT NULL CHECK (state IN ('READY', 'ACTIVE', 'CLOSED')),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (session_id, scene_key)
);

CREATE UNIQUE INDEX scenes_one_active_per_session_room_idx
    ON public.scenes(session_id, room_id)
    WHERE state = 'ACTIVE';

ALTER TABLE core_domain.sessions
    ADD CONSTRAINT sessions_active_scene_fkey
    FOREIGN KEY (active_scene_id)
    REFERENCES public.scenes(scene_id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE public.campaign_forks (
    fork_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    parent_campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    child_campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id)
        DEFERRABLE INITIALLY DEFERRED,
    source_session_id TEXT NOT NULL REFERENCES core_domain.sessions(session_id)
        DEFERRABLE INITIALLY DEFERRED,
    source_snapshot_hash TEXT NOT NULL CHECK (
        source_snapshot_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    reason TEXT NOT NULL CHECK (btrim(reason) <> '' AND length(reason) <= 512),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (campaign_id = parent_campaign_id),
    CHECK (parent_campaign_id <> child_campaign_id),
    UNIQUE (parent_campaign_id, child_campaign_id)
);

CREATE TABLE public.reconsiderations (
    reconsideration_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    original_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    requested_by TEXT NOT NULL REFERENCES public.users(user_id),
    reason TEXT NOT NULL CHECK (btrim(reason) <> '' AND length(reason) <= 512),
    state TEXT NOT NULL CHECK (state IN ('REQUESTED', 'REVIEWED', 'RESOLVED')),
    resolution TEXT,
    event_chain JSONB NOT NULL CHECK (
        jsonb_typeof(event_chain) = 'array'
        AND jsonb_array_length(event_chain) > 0
    ),
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (
        (state <> 'RESOLVED' AND resolution IS NULL)
        OR
        (state = 'RESOLVED' AND btrim(resolution) <> '')
    )
);

CREATE INDEX rooms_campaign_idx ON public.rooms(campaign_id);
CREATE INDEX scenarios_campaign_idx ON public.scenarios(campaign_id);
CREATE INDEX characters_campaign_owner_idx
    ON public.characters(campaign_id, owner_user_id);
CREATE INDEX character_sheet_versions_character_idx
    ON public.character_sheet_versions(character_id, version DESC);
CREATE INDEX sessions_campaign_state_idx
    ON core_domain.sessions(campaign_id, state);
CREATE INDEX scenes_session_state_idx ON public.scenes(session_id, state);
CREATE INDEX campaign_forks_parent_idx ON public.campaign_forks(parent_campaign_id);
CREATE INDEX reconsiderations_campaign_state_idx
    ON public.reconsiderations(campaign_id, state);

CREATE FUNCTION public.enforce_core_projection_event()
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
BEGIN
    permitted_event_types := string_to_array(TG_ARGV[0], ',');
    projection_relation := TG_ARGV[1];
    projection_row_id := to_jsonb(NEW) ->> TG_ARGV[2];
    projection_capability := current_setting(
        'trpg.projection_capability',
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
        RAISE EXCEPTION 'core projection event sequence must advance';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER campaigns_event_guard
BEFORE INSERT OR UPDATE ON public.campaigns
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignCreated', 'public.campaigns', 'campaign_id'
);
CREATE TRIGGER rooms_event_guard
BEFORE INSERT OR UPDATE ON public.rooms
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignCreated', 'public.rooms', 'room_id'
);
CREATE TRIGGER scenarios_event_guard
BEFORE INSERT OR UPDATE ON public.scenarios
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ScenarioImported', 'public.scenarios', 'scenario_id'
);
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterSubmitted,CharacterInitialVersionApproved',
    'public.characters', 'character_id'
);
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterInitialVersionApproved',
    'public.character_sheet_versions', 'sheet_version_id'
);
CREATE TRIGGER sessions_event_guard
BEFORE INSERT OR UPDATE ON core_domain.sessions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'SessionStarted,SessionStateChanged,SceneSwitched',
    'core_domain.sessions', 'session_id'
);
CREATE TRIGGER scenes_event_guard
BEFORE INSERT OR UPDATE ON public.scenes
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'SessionStarted,SessionStateChanged,SceneSwitched',
    'public.scenes', 'scene_id'
);
CREATE TRIGGER campaign_forks_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_forks
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignForkRecorded', 'public.campaign_forks', 'fork_id'
);
CREATE TRIGGER reconsiderations_event_guard
BEFORE INSERT OR UPDATE ON public.reconsiderations
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ReconsiderationRequested,ReconsiderationReviewed',
    'public.reconsiderations', 'reconsideration_id'
);

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA core_domain
            FROM trpg_canonical_service;
        REVOKE ALL PRIVILEGES ON SCHEMA core_domain
            FROM trpg_canonical_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT SELECT, INSERT, UPDATE ON
            public.campaigns, public.rooms, public.scenarios,
            public.characters, public.character_sheet_versions,
            public.scenes, public.campaign_forks, public.reconsiderations,
            core_domain.sessions
            TO trpg_api_service;
        REVOKE DELETE ON
            public.campaigns, public.rooms, public.scenarios,
            public.characters, public.character_sheet_versions,
            public.scenes, public.campaign_forks, public.reconsiderations,
            core_domain.sessions
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_worker_service;
        GRANT SELECT ON
            public.campaigns, public.rooms, public.scenarios,
            public.characters, public.character_sheet_versions,
            public.scenes, public.campaign_forks, public.reconsiderations,
            core_domain.sessions
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
