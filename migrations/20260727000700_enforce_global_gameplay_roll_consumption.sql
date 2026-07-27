-- P08 forward-only hardening: every opaque Combat/Chase server roll is
-- consumed once across all aggregates, while the consumption projection
-- remains rebuildable from verified canonical gameplay events.

CREATE TABLE public.gameplay_roll_consumptions (
    roll_id TEXT PRIMARY KEY CHECK (
        btrim(roll_id) <> '' AND length(roll_id) <= 128
    ),
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    aggregate_kind TEXT NOT NULL CHECK (
        aggregate_kind IN ('COMBAT', 'CHASE')
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
            'CHASE_PARTICIPANT_PERCENTILE'
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

CREATE TRIGGER gameplay_roll_consumptions_event_guard
BEFORE INSERT OR UPDATE ON public.gameplay_roll_consumptions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CombatStateRecorded,ChaseStateRecorded',
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
