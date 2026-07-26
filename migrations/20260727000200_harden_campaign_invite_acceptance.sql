-- P07 repair: make invite consumption and membership projection one
-- canonical transaction. A conflicting or revoked pre-existing membership
-- aborts the Event Store append instead of consuming the invite.

CREATE FUNCTION core_domain.campaign_invite_acceptance_projection_id(
    projection JSONB
)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
    SELECT 'projection_' || encode(
        sha256(convert_to(projection::TEXT, 'UTF8')),
        'hex'
    )
$$;

CREATE FUNCTION core_domain.apply_campaign_invite_acceptance(
    target_commit_id TEXT,
    projection JSONB
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public, core_domain
AS $$
DECLARE
    formal public.formal_commits%ROWTYPE;
    audit public.canonical_audit_log%ROWTYPE;
    projection_id TEXT;
    projection_capability TEXT;
    projection_capability_hash TEXT;
    event_count BIGINT;
    accepted_sequence BIGINT;
    granted_at_unix_ms BIGINT;
BEGIN
    IF target_commit_id IS NULL
       OR btrim(target_commit_id) = ''
       OR jsonb_typeof(projection) <> 'object'
       OR projection ->> 'kind' IS DISTINCT FROM 'ACCEPT'
       OR btrim(COALESCE(projection ->> 'invite_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'campaign_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'user_id', '')) = ''
       OR btrim(COALESCE(projection ->> 'granted_by', '')) = ''
       OR projection ->> 'role' NOT IN ('PLAYER', 'SPECTATOR') THEN
        RAISE EXCEPTION 'invalid campaign invite acceptance projection';
    END IF;

    BEGIN
        granted_at_unix_ms := (projection ->> 'granted_at_unix_ms')::BIGINT;
    EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN
        RAISE EXCEPTION 'invalid campaign invite acceptance timestamp';
    END;
    IF granted_at_unix_ms <= 0 THEN
        RAISE EXCEPTION 'invalid campaign invite acceptance timestamp';
    END IF;

    SELECT * INTO formal
      FROM public.formal_commits
     WHERE commit_id = target_commit_id
       AND status = 'committed'
     FOR SHARE;
    IF formal.commit_id IS NULL
       OR formal.campaign_id IS DISTINCT FROM projection ->> 'campaign_id'
       OR formal.stream_id IS DISTINCT FROM projection ->> 'invite_id'
       OR formal.idempotency_operation IS DISTINCT FROM 'canonical_commit' THEN
        RAISE EXCEPTION 'campaign invite formal commit mismatch';
    END IF;

    SELECT * INTO audit
      FROM public.canonical_audit_log
     WHERE sequence = formal.audit_sequence;
    IF audit.sequence IS NULL
       OR audit.campaign_id IS DISTINCT FROM formal.campaign_id
       OR audit.resource_type IS DISTINCT FROM 'campaign_invite'
       OR audit.resource_id IS DISTINCT FROM projection ->> 'invite_id'
       OR audit.action IS DISTINCT FROM 'write_official_state'
       OR audit.requested_role IS DISTINCT FROM 'workflow'
       OR audit.decision IS DISTINCT FROM 'PERMIT' THEN
        RAISE EXCEPTION 'campaign invite policy evidence missing';
    END IF;

    SELECT count(*),
           min(sequence) FILTER (
               WHERE event_type = 'CampaignInviteAccepted'
           )
      INTO event_count, accepted_sequence
      FROM public.event_store
     WHERE sequence BETWEEN
           formal.first_event_sequence AND formal.last_event_sequence;
    IF event_count <> 1 OR accepted_sequence IS NULL THEN
        RAISE EXCEPTION 'invalid campaign invite acceptance event batch';
    END IF;

    projection_id :=
        core_domain.campaign_invite_acceptance_projection_id(projection);
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL OR btrim(projection_capability) = '' THEN
        RAISE EXCEPTION 'campaign invite projection capability missing';
    END IF;
    projection_capability_hash := 'sha256:' || encode(
        sha256(convert_to(projection_capability, 'UTF8')),
        'hex'
    );
    IF NOT EXISTS (
        SELECT 1
          FROM public.event_store AS event
          CROSS JOIN LATERAL jsonb_array_elements(
              event.projection_targets
          ) AS target
         WHERE event.sequence = accepted_sequence
           AND event.campaign_id = formal.campaign_id
           AND event.stream_id = projection ->> 'invite_id'
           AND event.resource_type = 'campaign_invite'
           AND target ->> 'relation' =
               'core_domain.campaign_invite_acceptance'
           AND target ->> 'row_id' = projection_id
           AND target ->> 'capability_hash' = projection_capability_hash
    ) THEN
        RAISE EXCEPTION 'campaign invite projection is not HMAC-bound';
    END IF;

    INSERT INTO public.campaign_memberships (
        campaign_id, user_id, role, granted_by, granted_at
    ) VALUES (
        projection ->> 'campaign_id',
        projection ->> 'user_id',
        projection ->> 'role',
        projection ->> 'granted_by',
        to_timestamp(granted_at_unix_ms::DOUBLE PRECISION / 1000.0)
    );
END;
$$;

REVOKE ALL ON FUNCTION
    core_domain.campaign_invite_acceptance_projection_id(JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.apply_campaign_invite_acceptance(TEXT, JSONB)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.apply_campaign_invite_acceptance(TEXT, JSONB)
            TO trpg_canonical_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.campaign_invite_acceptance_projection_id(JSONB)
            TO trpg_api_service;
    END IF;
END;
$least_privilege$;
