CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    login_normalized TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    global_role TEXT NOT NULL CHECK (
        global_role IN ('USER', 'MODERATOR', 'SERVER_OWNER')
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    disabled_at TIMESTAMPTZ,
    CHECK (login_normalized = lower(login_normalized)),
    CHECK (length(password_hash) > 0)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    token_hash BYTEA NOT NULL UNIQUE,
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    rotated_from_session_id TEXT REFERENCES sessions(session_id),
    CHECK (expires_at > issued_at)
);

CREATE INDEX IF NOT EXISTS sessions_user_id_idx ON sessions(user_id);
CREATE INDEX IF NOT EXISTS sessions_expires_at_idx ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS campaign_memberships (
    campaign_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    role TEXT NOT NULL CHECK (
        role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER', 'PLAYER', 'SPECTATOR')
    ),
    granted_by TEXT NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (campaign_id, user_id)
);

CREATE INDEX IF NOT EXISTS campaign_memberships_user_id_idx
    ON campaign_memberships(user_id);

CREATE TABLE IF NOT EXISTS authority_contracts (
    contract_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL UNIQUE,
    authority_mode TEXT NOT NULL CHECK (authority_mode IN ('HUMAN_KP', 'AI_KP')),
    authority_owner TEXT NOT NULL,
    contract_version BIGINT NOT NULL CHECK (contract_version > 0),
    ruleset_version TEXT NOT NULL,
    house_rules_version TEXT NOT NULL,
    scenario_version TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    agent_pack_version TEXT NOT NULL,
    tool_schema_version TEXT NOT NULL,
    safety_profile_version TEXT NOT NULL,
    ai_provider_snapshot TEXT NOT NULL,
    model_route_snapshot TEXT NOT NULL,
    character_sheet_template_version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    locked BOOLEAN NOT NULL DEFAULT TRUE CHECK (locked),
    change_policy TEXT NOT NULL DEFAULT 'FORK_ONLY' CHECK (change_policy = 'FORK_ONLY')
);

CREATE OR REPLACE FUNCTION reject_authority_contract_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'authority contracts are immutable; create a forked campaign';
END;
$$;

CREATE OR REPLACE FUNCTION enforce_authority_owner_membership()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.authority_mode = 'HUMAN_KP' THEN
        IF NOT EXISTS (
            SELECT 1
              FROM campaign_memberships
             WHERE campaign_id = NEW.campaign_id
               AND user_id = NEW.authority_owner
               AND role = 'HUMAN_KEEPER'
               AND revoked_at IS NULL
        ) OR EXISTS (
            SELECT 1
              FROM campaign_memberships
             WHERE campaign_id = NEW.campaign_id
               AND user_id <> NEW.authority_owner
               AND role = 'HUMAN_KEEPER'
               AND revoked_at IS NULL
        ) THEN
            RAISE EXCEPTION 'HUMAN_KP authority owner must be the unique active human keeper';
        END IF;
    ELSIF EXISTS (
        SELECT 1
          FROM campaign_memberships
         WHERE campaign_id = NEW.campaign_id
           AND role = 'HUMAN_KEEPER'
           AND revoked_at IS NULL
    ) THEN
        RAISE EXCEPTION 'AI_KP campaign cannot have an active human keeper authority';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION enforce_membership_authority_consistency()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    current_mode TEXT;
    current_owner TEXT;
BEGIN
    SELECT authority_mode, authority_owner
      INTO current_mode, current_owner
      FROM authority_contracts
     WHERE campaign_id = NEW.campaign_id;

    IF FOUND AND NEW.revoked_at IS NULL THEN
        IF NEW.role = 'HUMAN_KEEPER'
           AND (current_mode <> 'HUMAN_KP' OR NEW.user_id <> current_owner) THEN
            RAISE EXCEPTION 'membership conflicts with canonical Authority Contract';
        END IF;
        IF current_mode = 'HUMAN_KP'
           AND NEW.user_id = current_owner
           AND NEW.role <> 'HUMAN_KEEPER' THEN
            RAISE EXCEPTION 'authority owner membership cannot be downgraded in place';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'authority_contract_owner_membership'
           AND tgrelid = 'authority_contracts'::regclass
    ) THEN
        CREATE TRIGGER authority_contract_owner_membership
        BEFORE INSERT ON authority_contracts
        FOR EACH ROW EXECUTE FUNCTION enforce_authority_owner_membership();
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'membership_authority_consistency'
           AND tgrelid = 'campaign_memberships'::regclass
    ) THEN
        CREATE TRIGGER membership_authority_consistency
        BEFORE INSERT OR UPDATE ON campaign_memberships
        FOR EACH ROW EXECUTE FUNCTION enforce_membership_authority_consistency();
    END IF;
END;
$migration$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'authority_contracts_immutable'
           AND tgrelid = 'authority_contracts'::regclass
    ) THEN
        CREATE TRIGGER authority_contracts_immutable
        BEFORE UPDATE OR DELETE ON authority_contracts
        FOR EACH ROW EXECUTE FUNCTION reject_authority_contract_mutation();
    END IF;
END;
$migration$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'authority_contracts_no_truncate'
           AND tgrelid = 'authority_contracts'::regclass
    ) THEN
        CREATE TRIGGER authority_contracts_no_truncate
        BEFORE TRUNCATE ON authority_contracts
        FOR EACH STATEMENT EXECUTE FUNCTION reject_authority_contract_mutation();
    END IF;
END;
$migration$;

CREATE TABLE IF NOT EXISTS audit_log (
    sequence BIGINT PRIMARY KEY,
    actor_id TEXT NOT NULL,
    actor_origin TEXT NOT NULL,
    authentication_reference TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    action TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('PERMIT', 'DENY', 'UNAVAILABLE')),
    openfga_decision_id TEXT NOT NULL,
    openfga_policy_revision TEXT NOT NULL,
    opa_decision_id TEXT NOT NULL,
    opa_policy_revision TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    integrity_key_id TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    CHECK (length(trim(integrity_key_id)) > 0),
    CHECK (previous_hash ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (record_hash ~ '^hmac-sha256:[0-9a-f]{64}$')
);

CREATE INDEX IF NOT EXISTS audit_log_campaign_sequence_idx
    ON audit_log(campaign_id, sequence);
CREATE INDEX IF NOT EXISTS audit_log_actor_sequence_idx
    ON audit_log(actor_id, sequence);

CREATE OR REPLACE FUNCTION reject_audit_log_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'audit log records are append-only';
END;
$$;

CREATE OR REPLACE FUNCTION enforce_audit_log_chain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended('trpg.audit_log.chain', 0));
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM audit_log
     ORDER BY sequence DESC
     LIMIT 1;

    NEW.sequence := COALESCE(latest_sequence, 0) + 1;
    IF NEW.previous_hash <> COALESCE(
        latest_hash,
        'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
    ) THEN
        RAISE EXCEPTION 'audit log chain predecessor mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'audit_log_append_only'
           AND tgrelid = 'audit_log'::regclass
    ) THEN
        CREATE TRIGGER audit_log_append_only
        BEFORE UPDATE OR DELETE ON audit_log
        FOR EACH ROW EXECUTE FUNCTION reject_audit_log_mutation();
    END IF;
END;
$migration$;

DO $migration$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'audit_log_chain_guard'
           AND tgrelid = 'audit_log'::regclass
    ) THEN
        CREATE TRIGGER audit_log_chain_guard
        BEFORE INSERT ON audit_log
        FOR EACH ROW EXECUTE FUNCTION enforce_audit_log_chain();
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'audit_log_no_truncate'
           AND tgrelid = 'audit_log'::regclass
    ) THEN
        CREATE TRIGGER audit_log_no_truncate
        BEFORE TRUNCATE ON audit_log
        FOR EACH STATEMENT EXECUTE FUNCTION reject_audit_log_mutation();
    END IF;
END;
$migration$;
