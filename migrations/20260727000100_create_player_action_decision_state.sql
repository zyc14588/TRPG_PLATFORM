-- P07 forward-only migration: governed player-action decision state.
--
-- The canonical service receives EXECUTE on one SECURITY DEFINER projection
-- function, never direct table DML.  The function is called from the same
-- transaction that appends Event Store rows, outbox rows, audit evidence, and
-- the formal-commit receipt.

CREATE TABLE public.player_actions (
    action_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    character_id TEXT NOT NULL REFERENCES public.characters(character_id),
    scene_id TEXT NOT NULL REFERENCES public.scenes(scene_id),
    submitted_by TEXT NOT NULL REFERENCES public.users(user_id),
    action_kind TEXT NOT NULL CHECK (
        action_kind IN ('INVESTIGATION', 'SANITY_CHECK')
    ),
    intent_json JSONB NOT NULL CHECK (
        jsonb_typeof(intent_json) = 'object'
        AND NOT (
            intent_json ? 'roll'
            OR intent_json ? 'dice'
            OR intent_json ? 'dice_roll'
            OR intent_json ? 'random_value'
        )
    ),
    state TEXT NOT NULL CHECK (
        state IN ('AWAITING_HUMAN_CONFIRMATION', 'RESOLVED', 'REJECTED')
    ),
    version BIGINT NOT NULL CHECK (version > 0),
    submitted_at_unix_ms BIGINT NOT NULL CHECK (submitted_at_unix_ms > 0),
    confirmed_by TEXT REFERENCES public.users(user_id),
    resolved_at_unix_ms BIGINT,
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (
        state = 'AWAITING_HUMAN_CONFIRMATION'
        AND confirmed_by IS NULL
        AND resolved_at_unix_ms IS NULL
        OR
        state IN ('RESOLVED', 'REJECTED')
        AND confirmed_by IS NOT NULL
        AND resolved_at_unix_ms IS NOT NULL
    )
);

CREATE TABLE public.decision_records (
    decision_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    action_id TEXT NOT NULL UNIQUE REFERENCES public.player_actions(action_id),
    confirmed_by TEXT NOT NULL REFERENCES public.users(user_id),
    tool_execution_id TEXT NOT NULL UNIQUE CHECK (btrim(tool_execution_id) <> ''),
    outcome_json JSONB NOT NULL CHECK (jsonb_typeof(outcome_json) = 'object'),
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence)
);

CREATE TABLE public.dice_rolls (
    roll_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    action_id TEXT NOT NULL UNIQUE REFERENCES public.player_actions(action_id),
    decision_id TEXT NOT NULL REFERENCES public.decision_records(decision_id),
    target_value SMALLINT NOT NULL CHECK (target_value BETWEEN 1 AND 100),
    rolled_value SMALLINT NOT NULL CHECK (rolled_value BETWEEN 1 AND 100),
    success_level TEXT NOT NULL CHECK (
        success_level IN (
            'CRITICAL', 'EXTREME', 'HARD', 'REGULAR', 'FAILURE', 'FUMBLE'
        )
    ),
    selected_tens_digit SMALLINT NOT NULL CHECK (
        selected_tens_digit BETWEEN 0 AND 9
    ),
    ones_digit SMALLINT NOT NULL CHECK (ones_digit BETWEEN 0 AND 9),
    adjustment TEXT NOT NULL CHECK (
        adjustment IN ('NONE', 'BONUS', 'PENALTY')
    ),
    random_source TEXT NOT NULL CHECK (random_source = 'SERVER_OS_CSPRNG'),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence)
);

CREATE TABLE public.clues (
    clue_record_id TEXT PRIMARY KEY,
    clue_id TEXT NOT NULL CHECK (btrim(clue_id) <> ''),
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    action_id TEXT NOT NULL UNIQUE REFERENCES public.player_actions(action_id),
    decision_id TEXT NOT NULL REFERENCES public.decision_records(decision_id),
    importance TEXT NOT NULL CHECK (importance IN ('CORE', 'OPTIONAL')),
    outcome TEXT NOT NULL CHECK (
        outcome IN ('REVEALED', 'REVEALED_WITH_COST', 'NOT_FOUND')
    ),
    cost TEXT,
    revealed_to_party BOOLEAN NOT NULL,
    version BIGINT NOT NULL CHECK (version = 1),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    UNIQUE (campaign_id, clue_id)
);

CREATE TABLE public.sanity_events (
    sanity_event_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES public.campaigns(campaign_id),
    action_id TEXT NOT NULL UNIQUE REFERENCES public.player_actions(action_id),
    decision_id TEXT NOT NULL REFERENCES public.decision_records(decision_id),
    character_id TEXT NOT NULL REFERENCES public.characters(character_id),
    sheet_version_id TEXT NOT NULL REFERENCES public.character_sheet_versions(sheet_version_id),
    day_key TEXT NOT NULL CHECK (btrim(day_key) <> ''),
    day_start_sanity SMALLINT NOT NULL CHECK (day_start_sanity BETWEEN 0 AND 99),
    sanity_before SMALLINT NOT NULL CHECK (sanity_before BETWEEN 0 AND 99),
    sanity_after SMALLINT NOT NULL CHECK (sanity_after BETWEEN 0 AND 99),
    sanity_loss SMALLINT NOT NULL CHECK (sanity_loss BETWEEN 0 AND 99),
    day_loss SMALLINT NOT NULL CHECK (day_loss BETWEEN 0 AND 255),
    indefinite_threshold SMALLINT NOT NULL CHECK (
        indefinite_threshold BETWEEN 1 AND 99
    ),
    madness_state TEXT NOT NULL CHECK (
        madness_state IN ('STABLE', 'TEMPORARY_INSANITY', 'INDEFINITE_INSANITY')
    ),
    visibility_label core_domain.visibility_label NOT NULL,
    visibility_subject TEXT NOT NULL CHECK (btrim(visibility_subject) <> ''),
    provenance_kind core_domain.provenance_kind NOT NULL,
    provenance_reference TEXT NOT NULL CHECK (btrim(provenance_reference) <> ''),
    provenance_recorded_by TEXT NOT NULL CHECK (btrim(provenance_recorded_by) <> ''),
    last_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    CHECK (sanity_after = GREATEST(sanity_before - sanity_loss, 0)),
    CHECK (indefinite_threshold = GREATEST(day_start_sanity / 5, 1))
);

CREATE INDEX player_actions_campaign_state_idx
    ON public.player_actions(campaign_id, state);
CREATE INDEX decision_records_campaign_idx
    ON public.decision_records(campaign_id);
CREATE INDEX dice_rolls_campaign_idx ON public.dice_rolls(campaign_id);
CREATE INDEX clues_campaign_idx ON public.clues(campaign_id);
CREATE INDEX sanity_events_character_day_idx
    ON public.sanity_events(character_id, day_key, last_event_sequence);

CREATE FUNCTION core_domain.player_action_projection_id(projection JSONB)
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

CREATE FUNCTION core_domain.apply_player_action_projection(
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
    action_sequence BIGINT;
    decision_sequence BIGINT;
    dice_sequence BIGINT;
    clue_sequence BIGINT;
    sanity_sequence BIGINT;
    projection_kind TEXT;
    action_campaign_id TEXT;
    action_character_id TEXT;
    action_state TEXT;
BEGIN
    IF target_commit_id IS NULL
       OR btrim(target_commit_id) = ''
       OR jsonb_typeof(projection) <> 'object' THEN
        RAISE EXCEPTION 'invalid player action projection request';
    END IF;

    SELECT * INTO formal
      FROM public.formal_commits
     WHERE commit_id = target_commit_id
       AND status = 'committed'
     FOR SHARE;
    IF formal.commit_id IS NULL THEN
        RAISE EXCEPTION 'player action formal commit missing';
    END IF;

    SELECT * INTO audit
      FROM public.canonical_audit_log
     WHERE sequence = formal.audit_sequence;
    IF audit.sequence IS NULL
       OR audit.campaign_id IS DISTINCT FROM formal.campaign_id
       OR audit.action IS DISTINCT FROM 'write_official_state'
       OR audit.requested_role IS DISTINCT FROM 'workflow'
       OR audit.decision IS DISTINCT FROM 'PERMIT' THEN
        RAISE EXCEPTION 'player action policy evidence missing';
    END IF;

    projection_id := core_domain.player_action_projection_id(projection);
    projection_capability := current_setting(
        'trpg.projection_capability',
        TRUE
    );
    IF projection_capability IS NULL OR btrim(projection_capability) = '' THEN
        RAISE EXCEPTION 'player action projection capability missing';
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
         WHERE event.sequence BETWEEN
               formal.first_event_sequence AND formal.last_event_sequence
           AND target ->> 'relation' =
               'core_domain.player_action_projection'
           AND target ->> 'row_id' = projection_id
           AND target ->> 'capability_hash' = projection_capability_hash
    ) THEN
        RAISE EXCEPTION 'player action projection is not HMAC-bound';
    END IF;

    projection_kind := projection ->> 'kind';
    action_campaign_id := projection ->> 'campaign_id';
    IF action_campaign_id IS DISTINCT FROM formal.campaign_id THEN
        RAISE EXCEPTION 'player action campaign mismatch';
    END IF;

    SELECT count(*),
           min(sequence) FILTER (
               WHERE event_type = 'PlayerActionSubmitted'
           ),
           min(sequence) FILTER (
               WHERE event_type = 'DecisionCommitted'
           ),
           min(sequence) FILTER (
               WHERE event_type = 'DiceRolled'
           ),
           min(sequence) FILTER (
               WHERE event_type = 'ClueRevealed'
           ),
           min(sequence) FILTER (
               WHERE event_type = 'SanityLossApplied'
           )
      INTO event_count, action_sequence, decision_sequence, dice_sequence,
           clue_sequence, sanity_sequence
      FROM public.event_store
     WHERE sequence BETWEEN
           formal.first_event_sequence AND formal.last_event_sequence;

    IF projection_kind = 'SUBMIT' THEN
        IF event_count <> 1 OR action_sequence IS NULL THEN
            RAISE EXCEPTION 'invalid player action submission event batch';
        END IF;
        INSERT INTO public.player_actions (
            action_id, campaign_id, character_id, scene_id, submitted_by,
            action_kind, intent_json, state, version, submitted_at_unix_ms,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'action_id',
            action_campaign_id,
            projection ->> 'character_id',
            projection ->> 'scene_id',
            projection ->> 'submitted_by',
            projection ->> 'action_kind',
            projection -> 'intent',
            'AWAITING_HUMAN_CONFIRMATION',
            1,
            (projection ->> 'submitted_at_unix_ms')::BIGINT,
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            action_sequence
        );
        RETURN;
    END IF;

    SELECT campaign_id, character_id, state
      INTO action_campaign_id, action_character_id, action_state
      FROM public.player_actions
     WHERE action_id = projection ->> 'action_id'
     FOR UPDATE;
    IF action_campaign_id IS NULL
       OR action_campaign_id IS DISTINCT FROM formal.campaign_id
       OR action_character_id IS DISTINCT FROM projection ->> 'character_id'
       OR action_state IS DISTINCT FROM 'AWAITING_HUMAN_CONFIRMATION' THEN
        RAISE EXCEPTION 'pending player action mismatch';
    END IF;

    IF projection_kind = 'CONFIRM_INVESTIGATION' THEN
        IF event_count <> 4
           OR decision_sequence IS NULL
           OR dice_sequence IS NULL
           OR clue_sequence IS NULL THEN
            RAISE EXCEPTION 'invalid investigation decision event batch';
        END IF;

        INSERT INTO public.decision_records (
            decision_id, campaign_id, action_id, confirmed_by,
            tool_execution_id, outcome_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'decision_id', formal.campaign_id,
            projection ->> 'action_id', projection ->> 'confirmed_by',
            projection ->> 'tool_execution_id', projection -> 'outcome',
            1, projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            decision_sequence
        );

        INSERT INTO public.dice_rolls (
            roll_id, campaign_id, action_id, decision_id,
            target_value, rolled_value, success_level,
            selected_tens_digit, ones_digit, adjustment, random_source,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'roll_id', formal.campaign_id,
            projection ->> 'action_id', projection ->> 'decision_id',
            (projection ->> 'target_value')::SMALLINT,
            (projection ->> 'rolled_value')::SMALLINT,
            projection ->> 'success_level',
            (projection ->> 'selected_tens_digit')::SMALLINT,
            (projection ->> 'ones_digit')::SMALLINT,
            projection ->> 'adjustment', 'SERVER_OS_CSPRNG',
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            dice_sequence
        );

        INSERT INTO public.clues (
            clue_record_id, clue_id, campaign_id, action_id, decision_id,
            importance, outcome, cost, revealed_to_party, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'clue_record_id', projection ->> 'clue_id',
            formal.campaign_id, projection ->> 'action_id',
            projection ->> 'decision_id', projection ->> 'clue_importance',
            projection ->> 'clue_outcome', projection ->> 'clue_cost',
            (projection ->> 'revealed_to_party')::BOOLEAN, 1,
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            clue_sequence
        );
    ELSIF projection_kind = 'CONFIRM_SANITY' THEN
        IF event_count <> 3
           OR decision_sequence IS NULL
           OR dice_sequence IS NULL
           OR sanity_sequence IS NULL THEN
            RAISE EXCEPTION 'invalid SAN decision event batch';
        END IF;

        INSERT INTO public.decision_records (
            decision_id, campaign_id, action_id, confirmed_by,
            tool_execution_id, outcome_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'decision_id', formal.campaign_id,
            projection ->> 'action_id', projection ->> 'confirmed_by',
            projection ->> 'tool_execution_id', projection -> 'outcome',
            1, projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            decision_sequence
        );

        INSERT INTO public.dice_rolls (
            roll_id, campaign_id, action_id, decision_id,
            target_value, rolled_value, success_level,
            selected_tens_digit, ones_digit, adjustment, random_source,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'roll_id', formal.campaign_id,
            projection ->> 'action_id', projection ->> 'decision_id',
            (projection ->> 'target_value')::SMALLINT,
            (projection ->> 'rolled_value')::SMALLINT,
            projection ->> 'success_level',
            (projection ->> 'selected_tens_digit')::SMALLINT,
            (projection ->> 'ones_digit')::SMALLINT,
            projection ->> 'adjustment', 'SERVER_OS_CSPRNG',
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            dice_sequence
        );

        INSERT INTO public.character_sheet_versions (
            sheet_version_id, character_id, version, sheet_json, locked,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            campaign_id, last_event_sequence
        ) VALUES (
            projection ->> 'sheet_version_id',
            projection ->> 'character_id',
            (projection ->> 'sheet_version')::BIGINT,
            projection -> 'sheet_json', TRUE,
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            formal.campaign_id, sanity_sequence
        );

        UPDATE public.characters
           SET current_sheet_version =
                   (projection ->> 'sheet_version')::BIGINT,
               version = version + 1,
               visibility_label =
                   (projection ->> 'visibility_label')::core_domain.visibility_label,
               visibility_subject = projection ->> 'visibility_subject',
               provenance_kind =
                   (projection ->> 'provenance_kind')::core_domain.provenance_kind,
               provenance_reference = projection ->> 'provenance_reference',
               provenance_recorded_by = projection ->> 'provenance_recorded_by',
               last_event_sequence = sanity_sequence
         WHERE character_id = projection ->> 'character_id'
           AND campaign_id = formal.campaign_id
           AND current_sheet_version + 1 =
               (projection ->> 'sheet_version')::BIGINT;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'SAN character version conflict';
        END IF;

        INSERT INTO public.sanity_events (
            sanity_event_id, campaign_id, action_id, decision_id,
            character_id, sheet_version_id, day_key, day_start_sanity,
            sanity_before, sanity_after, sanity_loss, day_loss,
            indefinite_threshold, madness_state,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            projection ->> 'sanity_event_id', formal.campaign_id,
            projection ->> 'action_id', projection ->> 'decision_id',
            projection ->> 'character_id', projection ->> 'sheet_version_id',
            projection ->> 'day_key',
            (projection ->> 'day_start_sanity')::SMALLINT,
            (projection ->> 'sanity_before')::SMALLINT,
            (projection ->> 'sanity_after')::SMALLINT,
            (projection ->> 'sanity_loss')::SMALLINT,
            (projection ->> 'day_loss')::SMALLINT,
            (projection ->> 'indefinite_threshold')::SMALLINT,
            projection ->> 'madness_state',
            projection ->> 'visibility_label',
            projection ->> 'visibility_subject',
            projection ->> 'provenance_kind',
            projection ->> 'provenance_reference',
            projection ->> 'provenance_recorded_by',
            sanity_sequence
        );
    ELSE
        RAISE EXCEPTION 'unknown player action projection kind';
    END IF;

    UPDATE public.player_actions
       SET state = 'RESOLVED',
           version = version + 1,
           confirmed_by = projection ->> 'confirmed_by',
           resolved_at_unix_ms =
               (projection ->> 'resolved_at_unix_ms')::BIGINT,
           visibility_label =
               (projection ->> 'visibility_label')::core_domain.visibility_label,
           visibility_subject = projection ->> 'visibility_subject',
           provenance_kind =
               (projection ->> 'provenance_kind')::core_domain.provenance_kind,
           provenance_reference = projection ->> 'provenance_reference',
           provenance_recorded_by = projection ->> 'provenance_recorded_by',
           last_event_sequence = decision_sequence
     WHERE action_id = projection ->> 'action_id';
END;
$$;

CREATE TRIGGER player_actions_event_guard
BEFORE INSERT OR UPDATE ON public.player_actions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'PlayerActionSubmitted,DecisionCommitted',
    'public.player_actions',
    'action_id'
);
CREATE TRIGGER decision_records_event_guard
BEFORE INSERT OR UPDATE ON public.decision_records
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'DecisionCommitted',
    'public.decision_records',
    'decision_id'
);
CREATE TRIGGER dice_rolls_event_guard
BEFORE INSERT OR UPDATE ON public.dice_rolls
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'DiceRolled',
    'public.dice_rolls',
    'roll_id'
);
CREATE TRIGGER clues_event_guard
BEFORE INSERT OR UPDATE ON public.clues
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'ClueRevealed',
    'public.clues',
    'clue_record_id'
);
CREATE TRIGGER sanity_events_event_guard
BEFORE INSERT OR UPDATE ON public.sanity_events
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'SanityLossApplied',
    'public.sanity_events',
    'sanity_event_id'
);

DROP TRIGGER characters_event_guard ON public.characters;
CREATE TRIGGER characters_event_guard
BEFORE INSERT OR UPDATE ON public.characters
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterSubmitted,CharacterInitialVersionApproved,SanityLossApplied',
    'public.characters',
    'character_id'
);
DROP TRIGGER character_sheet_versions_event_guard
    ON public.character_sheet_versions;
CREATE TRIGGER character_sheet_versions_event_guard
BEFORE INSERT OR UPDATE ON public.character_sheet_versions
FOR EACH ROW EXECUTE FUNCTION public.enforce_core_projection_event(
    'CharacterCreated,CharacterInitialVersionApproved,SanityLossApplied',
    'public.character_sheet_versions',
    'sheet_version_id'
);

REVOKE ALL ON FUNCTION
    core_domain.player_action_projection_id(JSONB)
    FROM PUBLIC;
REVOKE ALL ON FUNCTION
    core_domain.apply_player_action_projection(TEXT, JSONB)
    FROM PUBLIC;

DO $least_privilege$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_canonical_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.apply_player_action_projection(TEXT, JSONB)
            TO trpg_canonical_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_api_service;
        GRANT EXECUTE ON FUNCTION
            core_domain.player_action_projection_id(JSONB)
            TO trpg_api_service;
        GRANT SELECT ON
            public.player_actions, public.decision_records,
            public.dice_rolls, public.clues, public.sanity_events
            TO trpg_api_service;
        REVOKE INSERT, UPDATE, DELETE ON
            public.player_actions, public.decision_records,
            public.dice_rolls, public.clues, public.sanity_events
            FROM trpg_api_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        GRANT USAGE ON SCHEMA core_domain TO trpg_worker_service;
        GRANT SELECT ON
            public.player_actions, public.decision_records,
            public.dice_rolls, public.clues, public.sanity_events
            TO trpg_worker_service;
    END IF;
END;
$least_privilege$;
