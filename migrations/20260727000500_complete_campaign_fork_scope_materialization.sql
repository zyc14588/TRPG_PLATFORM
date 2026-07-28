-- P08 review repair: every declared public fork copy scope must have a
-- child-owned, guarded read model. The canonical CampaignForkMaterialized
-- event remains the sole authority for these projections.

CREATE TABLE public.campaign_fork_public_events (
    fork_event_id TEXT PRIMARY KEY,
    fork_id TEXT NOT NULL REFERENCES public.campaign_forks(fork_id)
        DEFERRABLE INITIALLY DEFERRED,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    source_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    source_event_type TEXT NOT NULL CHECK (btrim(source_event_type) <> ''),
    source_resource_type TEXT NOT NULL CHECK (btrim(source_resource_type) <> ''),
    source_resource_id TEXT NOT NULL CHECK (btrim(source_resource_id) <> ''),
    source_payload_json JSONB NOT NULL CHECK (
        jsonb_typeof(source_payload_json) = 'object'
    ),
    source_event_integrity_hash TEXT NOT NULL CHECK (
        source_event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
    ),
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL CHECK (
        visibility_label IN ('public', 'party_visible')
    ),
    visibility_subject TEXT NOT NULL CHECK (
        visibility_subject = 'not_applicable'
    ),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (fork_id, source_event_sequence)
);

CREATE TABLE public.campaign_fork_clues (
    fork_clue_id TEXT PRIMARY KEY,
    fork_id TEXT NOT NULL REFERENCES public.campaign_forks(fork_id)
        DEFERRABLE INITIALLY DEFERRED,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    source_clue_id TEXT NOT NULL CHECK (btrim(source_clue_id) <> ''),
    importance TEXT NOT NULL CHECK (importance IN ('CORE', 'OPTIONAL')),
    outcome TEXT NOT NULL CHECK (
        outcome IN ('REVEALED', 'REVEALED_WITH_COST')
    ),
    cost TEXT,
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL CHECK (
        visibility_label IN ('public', 'party_visible')
    ),
    visibility_subject TEXT NOT NULL CHECK (
        visibility_subject = 'not_applicable'
    ),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (fork_id, source_clue_id)
);

CREATE TABLE public.campaign_fork_npc_states (
    npc_state_id TEXT PRIMARY KEY,
    fork_id TEXT NOT NULL REFERENCES public.campaign_forks(fork_id)
        DEFERRABLE INITIALLY DEFERRED,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    source_npc_id TEXT NOT NULL CHECK (btrim(source_npc_id) <> ''),
    state_json JSONB NOT NULL CHECK (jsonb_typeof(state_json) = 'object'),
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL CHECK (
        visibility_label IN ('public', 'party_visible')
    ),
    visibility_subject TEXT NOT NULL CHECK (
        visibility_subject = 'not_applicable'
    ),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (fork_id, source_npc_id)
);

CREATE INDEX campaign_fork_public_events_campaign_idx
    ON public.campaign_fork_public_events(campaign_id, source_event_sequence);
CREATE INDEX campaign_fork_clues_campaign_idx
    ON public.campaign_fork_clues(campaign_id, source_clue_id);
CREATE INDEX campaign_fork_npc_states_campaign_idx
    ON public.campaign_fork_npc_states(campaign_id, source_npc_id);

CREATE TRIGGER campaign_fork_public_events_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_fork_public_events
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignForkMaterialized',
    'public.campaign_fork_public_events',
    'fork_event_id'
);

CREATE TRIGGER campaign_fork_clues_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_fork_clues
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignForkMaterialized',
    'public.campaign_fork_clues',
    'fork_clue_id'
);

CREATE TRIGGER campaign_fork_npc_states_event_guard
BEFORE INSERT OR UPDATE ON public.campaign_fork_npc_states
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CampaignForkMaterialized',
    'public.campaign_fork_npc_states',
    'npc_state_id'
);

DROP TRIGGER combat_states_event_guard ON public.combat_states;
CREATE TRIGGER combat_states_event_guard
BEFORE INSERT OR UPDATE ON public.combat_states
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CombatStateRecorded,CampaignForkMaterialized',
    'public.combat_states',
    'combat_id'
);

DROP TRIGGER chase_states_event_guard ON public.chase_states;
CREATE TRIGGER chase_states_event_guard
BEFORE INSERT OR UPDATE ON public.chase_states
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ChaseStateRecorded,CampaignForkMaterialized',
    'public.chase_states',
    'chase_id'
);

DROP TRIGGER ending_events_event_guard ON public.ending_events;
CREATE TRIGGER ending_events_event_guard
BEFORE INSERT OR UPDATE ON public.ending_events
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'EndingRecorded,CampaignForkMaterialized',
    'public.ending_events',
    'ending_event_id'
);

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT SELECT, INSERT ON
            public.campaign_fork_public_events,
            public.campaign_fork_clues,
            public.campaign_fork_npc_states
            TO trpg_api_service;
        REVOKE UPDATE, DELETE ON
            public.campaign_fork_public_events,
            public.campaign_fork_clues,
            public.campaign_fork_npc_states
            FROM trpg_api_service;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT SELECT ON
            public.campaign_fork_public_events,
            public.campaign_fork_clues,
            public.campaign_fork_npc_states
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
