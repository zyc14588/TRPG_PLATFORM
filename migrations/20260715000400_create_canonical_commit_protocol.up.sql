-- Durable formal-state commit protocol. Canonical events, policy audit evidence,
-- and outbox rows are inserted in one PostgreSQL transaction. The external
-- witness is prepared before this transaction and finalized/reconciled after it.

ALTER TABLE event_store
    ADD COLUMN IF NOT EXISTS campaign_id TEXT NOT NULL DEFAULT 'historical_unscoped',
    ADD COLUMN IF NOT EXISTS stream_version BIGINT,
    ADD COLUMN IF NOT EXISTS authenticated_actor_id TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS resource_type TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS resource_id TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS authority_contract_id TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS authority_owner TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS visibility_subject TEXT NOT NULL DEFAULT 'not_applicable',
    ADD COLUMN IF NOT EXISTS trace_id TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN IF NOT EXISTS event_integrity_hash TEXT;

UPDATE event_store SET stream_version = sequence WHERE stream_version IS NULL;
ALTER TABLE event_store ALTER COLUMN stream_version SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS event_store_campaign_stream_version_idx
    ON event_store(campaign_id, stream_version);
CREATE INDEX IF NOT EXISTS event_store_campaign_sequence_idx
    ON event_store(campaign_id, sequence);

ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_integrity_hash_format;
ALTER TABLE event_store
    ADD CONSTRAINT event_store_integrity_hash_format
    CHECK (
        event_integrity_hash IS NULL
        OR event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
    );

ALTER TABLE event_outbox
    ADD COLUMN IF NOT EXISTS commit_id TEXT,
    ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS claim_owner TEXT,
    ADD COLUMN IF NOT EXISTS last_error TEXT,
    ADD COLUMN IF NOT EXISTS dead_lettered_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS event_outbox_commit_idx
    ON event_outbox(commit_id, event_sequence);

CREATE TABLE IF NOT EXISTS canonical_audit_log (
    sequence BIGINT PRIMARY KEY,
    commit_id TEXT NOT NULL UNIQUE,
    campaign_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_origin TEXT NOT NULL,
    authentication_reference TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    action TEXT NOT NULL,
    requested_role TEXT NOT NULL,
    visibility_label TEXT NOT NULL,
    visibility_subject TEXT NOT NULL,
    provenance_kind TEXT NOT NULL,
    provenance_reference TEXT NOT NULL,
    provenance_recorded_by TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision = 'PERMIT'),
    openfga_decision_id TEXT NOT NULL,
    openfga_policy_revision TEXT NOT NULL,
    opa_decision_id TEXT NOT NULL,
    opa_policy_revision TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    event_batch_hash TEXT NOT NULL,
    witness_prepare_sequence BIGINT NOT NULL,
    witness_prepare_hash TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    integrity_key_id TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    CHECK (event_batch_hash ~ '^sha256:[0-9a-f]{64}$'),
    CHECK (witness_prepare_hash ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (previous_hash ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (record_hash ~ '^hmac-sha256:[0-9a-f]{64}$')
);

CREATE TABLE IF NOT EXISTS formal_commits (
    commit_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    request_hash TEXT NOT NULL,
    expected_version BIGINT NOT NULL,
    first_event_sequence BIGINT NOT NULL REFERENCES event_store(sequence),
    last_event_sequence BIGINT NOT NULL REFERENCES event_store(sequence),
    first_stream_version BIGINT NOT NULL,
    last_stream_version BIGINT NOT NULL,
    audit_sequence BIGINT NOT NULL REFERENCES canonical_audit_log(sequence),
    witness_prepare_sequence BIGINT NOT NULL,
    witness_prepare_hash TEXT NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (request_hash ~ '^sha256:[0-9a-f]{64}$'),
    CHECK (witness_prepare_hash ~ '^hmac-sha256:[0-9a-f]{64}$')
);

CREATE OR REPLACE FUNCTION reject_canonical_append_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'canonical commit records are append-only';
END;
$$;

DROP TRIGGER IF EXISTS event_store_append_only ON event_store;
CREATE TRIGGER event_store_append_only
BEFORE UPDATE OR DELETE ON event_store
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS event_store_no_truncate ON event_store;
CREATE TRIGGER event_store_no_truncate
BEFORE TRUNCATE ON event_store
FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS canonical_audit_log_append_only ON canonical_audit_log;
CREATE TRIGGER canonical_audit_log_append_only
BEFORE UPDATE OR DELETE ON canonical_audit_log
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS canonical_audit_log_no_truncate ON canonical_audit_log;
CREATE TRIGGER canonical_audit_log_no_truncate
BEFORE TRUNCATE ON canonical_audit_log
FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS formal_commits_append_only ON formal_commits;
CREATE TRIGGER formal_commits_append_only
BEFORE UPDATE OR DELETE ON formal_commits
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS formal_commits_no_truncate ON formal_commits;
CREATE TRIGGER formal_commits_no_truncate
BEFORE TRUNCATE ON formal_commits
FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation();

CREATE OR REPLACE FUNCTION enforce_canonical_audit_chain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended('trpg.canonical_audit_log.chain', 0));
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM canonical_audit_log
     ORDER BY sequence DESC
     LIMIT 1;
    NEW.sequence := COALESCE(latest_sequence, 0) + 1;
    IF NEW.previous_hash <> COALESCE(
        latest_hash,
        'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
    ) THEN
        RAISE EXCEPTION 'canonical audit predecessor mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS canonical_audit_log_chain_guard ON canonical_audit_log;
CREATE TRIGGER canonical_audit_log_chain_guard
BEFORE INSERT ON canonical_audit_log
FOR EACH ROW EXECUTE FUNCTION enforce_canonical_audit_chain();

CREATE TABLE IF NOT EXISTS workflow_instances (
    workflow_id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL,
    workflow_type TEXT NOT NULL,
    state TEXT NOT NULL,
    version BIGINT NOT NULL,
    input_json TEXT NOT NULL,
    output_json TEXT,
    wake_at TIMESTAMPTZ,
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS workflow_transitions (
    transition_id BIGSERIAL PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflow_instances(workflow_id),
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    workflow_version BIGINT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    correlation_id TEXT NOT NULL,
    causation_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workflow_id, workflow_version)
);

DROP TRIGGER IF EXISTS workflow_transitions_append_only ON workflow_transitions;
CREATE TRIGGER workflow_transitions_append_only
BEFORE UPDATE OR DELETE ON workflow_transitions
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

DROP TRIGGER IF EXISTS workflow_transitions_no_truncate ON workflow_transitions;
CREATE TRIGGER workflow_transitions_no_truncate
BEFORE TRUNCATE ON workflow_transitions
FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation();
