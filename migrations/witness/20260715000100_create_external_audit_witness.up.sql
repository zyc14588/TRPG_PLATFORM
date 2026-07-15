-- This migration is applied only to the independently operated audit-witness
-- PostgreSQL service. It must never be treated as a table in the primary
-- canonical-state database.

CREATE TABLE IF NOT EXISTS external_audit_witness (
    sequence BIGINT PRIMARY KEY,
    commit_id TEXT NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('PREPARED', 'COMMITTED', 'ABORTED')),
    primary_request_hash TEXT NOT NULL,
    primary_first_sequence BIGINT,
    primary_last_sequence BIGINT,
    reason TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    integrity_key_id TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    UNIQUE (commit_id, phase),
    CHECK (primary_request_hash ~ '^sha256:[0-9a-f]{64}$'),
    CHECK (previous_hash ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (record_hash ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (
        (phase = 'COMMITTED' AND primary_first_sequence IS NOT NULL AND primary_last_sequence IS NOT NULL)
        OR
        (phase <> 'COMMITTED' AND primary_first_sequence IS NULL AND primary_last_sequence IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS external_audit_witness_commit_idx
    ON external_audit_witness(commit_id, sequence);

CREATE OR REPLACE FUNCTION reject_external_witness_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'external audit witness is append-only';
END;
$$;

DROP TRIGGER IF EXISTS external_audit_witness_append_only ON external_audit_witness;
CREATE TRIGGER external_audit_witness_append_only
BEFORE UPDATE OR DELETE ON external_audit_witness
FOR EACH ROW EXECUTE FUNCTION reject_external_witness_mutation();

DROP TRIGGER IF EXISTS external_audit_witness_no_truncate ON external_audit_witness;
CREATE TRIGGER external_audit_witness_no_truncate
BEFORE TRUNCATE ON external_audit_witness
FOR EACH STATEMENT EXECUTE FUNCTION reject_external_witness_mutation();

CREATE OR REPLACE FUNCTION enforce_external_witness_chain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended('trpg.external_audit_witness.chain', 0));
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM external_audit_witness
     ORDER BY sequence DESC
     LIMIT 1;
    NEW.sequence := COALESCE(latest_sequence, 0) + 1;
    IF NEW.previous_hash <> COALESCE(
        latest_hash,
        'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
    ) THEN
        RAISE EXCEPTION 'external witness predecessor mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS external_audit_witness_chain_guard ON external_audit_witness;
CREATE TRIGGER external_audit_witness_chain_guard
BEFORE INSERT ON external_audit_witness
FOR EACH ROW EXECUTE FUNCTION enforce_external_witness_chain();
