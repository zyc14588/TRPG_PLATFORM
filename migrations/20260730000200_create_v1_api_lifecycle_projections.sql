-- AR06 forward-only migration: missing V1 API lifecycle projections.
--
-- These tables remain rebuildable read models. Every INSERT/UPDATE is guarded
-- by the canonical event projection capability established by
-- CoreDomainRepository after a committed formal event.

ALTER TABLE public.characters
    ADD CONSTRAINT characters_campaign_identity_key
    UNIQUE (campaign_id, character_id);

ALTER TABLE core_domain.sessions
    ADD CONSTRAINT sessions_campaign_identity_key
    UNIQUE (campaign_id, session_id);

CREATE TABLE core_domain.session_characters (
    join_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES public.users(user_id),
    joined_by TEXT NOT NULL REFERENCES public.users(user_id),
    joined_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    FOREIGN KEY (campaign_id, session_id)
        REFERENCES core_domain.sessions(campaign_id, session_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (campaign_id, character_id)
        REFERENCES public.characters(campaign_id, character_id)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK (joined_by = owner_user_id),
    UNIQUE (session_id, character_id),
    UNIQUE (session_id, owner_user_id)
);

CREATE INDEX session_characters_campaign_idx
    ON core_domain.session_characters(campaign_id, session_id);

CREATE TABLE public.campaign_exports (
    export_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    requested_by TEXT NOT NULL REFERENCES public.users(user_id),
    audience TEXT NOT NULL CHECK (audience = 'CAMPAIGN_ARCHIVE'),
    state TEXT NOT NULL CHECK (state IN ('REQUESTED', 'READY', 'FAILED')),
    requested_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, export_id)
);

CREATE INDEX campaign_exports_requester_idx
    ON public.campaign_exports(campaign_id, requested_by, requested_at DESC);

DROP TRIGGER characters_event_guard ON public.characters;
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterUpdated,CharacterSubmitted,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized,CombatStateRecorded',
    'public.characters',
    'character_id'
);

DROP TRIGGER character_sheet_versions_event_guard
    ON public.character_sheet_versions;
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterUpdated,CharacterInitialVersionApproved,SanityLossApplied,CharacterGrowthApplied,CampaignForkMaterialized,CombatStateRecorded',
    'public.character_sheet_versions',
    'sheet_version_id'
);

CREATE TRIGGER session_characters_event_guard
BEFORE INSERT OR UPDATE ON core_domain.session_characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterJoinedSession',
    'core_domain.session_characters',
    'join_id'
);

CREATE TRIGGER campaign_exports_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_exports
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignExportRequested',
    'public.campaign_exports',
    'export_id'
);

DO $least_privilege$
BEGIN
    REVOKE ALL PRIVILEGES ON core_domain.session_characters,
        public.campaign_exports FROM PUBLIC;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        REVOKE ALL PRIVILEGES ON core_domain.session_characters,
            public.campaign_exports FROM trpg_canonical_service;
        GRANT SELECT, INSERT ON public.privacy_subject_keys
            TO trpg_canonical_service;
        GRANT SELECT ON public.privacy_subject_deletion_fences
            TO trpg_canonical_service;
        REVOKE UPDATE, DELETE ON public.privacy_subject_keys
            FROM trpg_canonical_service;
        REVOKE INSERT, UPDATE, DELETE ON public.privacy_subject_deletion_fences
            FROM trpg_canonical_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT SELECT, INSERT ON core_domain.session_characters,
            public.campaign_exports TO trpg_api_service;
        GRANT SELECT ON public.player_actions TO trpg_api_service;
        REVOKE UPDATE, DELETE ON core_domain.session_characters,
            public.campaign_exports FROM trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE ON public.player_actions
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_worker_service;
        GRANT SELECT ON core_domain.session_characters,
            public.campaign_exports TO trpg_worker_service;
        REVOKE INSERT, UPDATE, DELETE ON core_domain.session_characters,
            public.campaign_exports FROM trpg_worker_service;
    END IF;
END;
$least_privilege$;
