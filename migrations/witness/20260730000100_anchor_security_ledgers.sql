-- Security ledger checkpoints live in the independently operated witness
-- database so a filesystem snapshot cannot roll them back with the ledger.

CREATE TABLE IF NOT EXISTS security_ledger_checkpoints (
    ledger_id TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    previous_chain_head TEXT NOT NULL,
    chain_head TEXT NOT NULL,
    integrity_key_id TEXT NOT NULL,
    checkpoint_mac TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (ledger_id, sequence),
    UNIQUE (ledger_id, chain_head),
    CHECK (ledger_id ~ '^[a-z][a-z0-9-]{2,63}:sha256:[0-9a-f]{64}$'),
    CHECK (sequence > 0),
    CHECK (previous_chain_head ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (chain_head ~ '^hmac-sha256:[0-9a-f]{64}$'),
    CHECK (length(btrim(integrity_key_id)) BETWEEN 1 AND 128),
    CHECK (checkpoint_mac ~ '^hmac-sha256:[0-9a-f]{64}$')
);

CREATE OR REPLACE FUNCTION reject_security_ledger_checkpoint_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'security ledger checkpoints are append-only';
END;
$$;

DROP TRIGGER IF EXISTS security_ledger_checkpoint_append_only
    ON security_ledger_checkpoints;
CREATE TRIGGER security_ledger_checkpoint_append_only
BEFORE UPDATE OR DELETE ON security_ledger_checkpoints
FOR EACH ROW EXECUTE FUNCTION reject_security_ledger_checkpoint_mutation();

DROP TRIGGER IF EXISTS security_ledger_checkpoint_no_truncate
    ON security_ledger_checkpoints;
CREATE TRIGGER security_ledger_checkpoint_no_truncate
BEFORE TRUNCATE ON security_ledger_checkpoints
FOR EACH STATEMENT EXECUTE FUNCTION reject_security_ledger_checkpoint_mutation();

CREATE OR REPLACE FUNCTION enforce_security_ledger_checkpoint_chain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    latest public.security_ledger_checkpoints%ROWTYPE;
BEGIN
    PERFORM pg_advisory_xact_lock(
        hashtextextended('trpg.security-ledger.' || NEW.ledger_id, 0)
    );
    SELECT *
      INTO latest
      FROM public.security_ledger_checkpoints
     WHERE ledger_id = NEW.ledger_id
     ORDER BY sequence DESC
     LIMIT 1;

    IF latest.sequence IS NULL THEN
        IF NEW.sequence <> 1
           OR NEW.previous_chain_head <>
              'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
        THEN
            RAISE EXCEPTION 'security ledger checkpoint genesis mismatch';
        END IF;
    ELSIF NEW.sequence = latest.sequence
          AND NEW.previous_chain_head = latest.previous_chain_head
          AND NEW.chain_head = latest.chain_head
          AND NEW.integrity_key_id = latest.integrity_key_id
          AND NEW.checkpoint_mac = latest.checkpoint_mac
    THEN
        RETURN NULL;
    ELSIF NEW.sequence <> latest.sequence + 1
          OR NEW.previous_chain_head <> latest.chain_head
    THEN
        RAISE EXCEPTION 'security ledger checkpoint predecessor mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS security_ledger_checkpoint_chain_guard
    ON security_ledger_checkpoints;
CREATE TRIGGER security_ledger_checkpoint_chain_guard
BEFORE INSERT ON security_ledger_checkpoints
FOR EACH ROW EXECUTE FUNCTION enforce_security_ledger_checkpoint_chain();

-- Runtime roles call these fixed-shape capabilities instead of receiving
-- table privileges. The witness role bootstrap intentionally revokes all
-- table grants on every restart, while these narrowly scoped function grants
-- remain stable.
CREATE OR REPLACE FUNCTION latest_security_ledger_checkpoint(
    requested_ledger_id TEXT
)
RETURNS TABLE (
    sequence BIGINT,
    previous_chain_head TEXT,
    chain_head TEXT,
    integrity_key_id TEXT,
    checkpoint_mac TEXT
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
    SELECT checkpoint.sequence,
           checkpoint.previous_chain_head,
           checkpoint.chain_head,
           checkpoint.integrity_key_id,
           checkpoint.checkpoint_mac
      FROM public.security_ledger_checkpoints AS checkpoint
     WHERE checkpoint.ledger_id = requested_ledger_id
     ORDER BY checkpoint.sequence DESC
     LIMIT 1
$$;

CREATE OR REPLACE FUNCTION append_security_ledger_checkpoint(
    requested_ledger_id TEXT,
    requested_sequence BIGINT,
    requested_previous_chain_head TEXT,
    requested_chain_head TEXT,
    requested_integrity_key_id TEXT,
    requested_checkpoint_mac TEXT
)
RETURNS void
LANGUAGE sql
VOLATILE
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
    INSERT INTO public.security_ledger_checkpoints (
        ledger_id,
        sequence,
        previous_chain_head,
        chain_head,
        integrity_key_id,
        checkpoint_mac
    )
    VALUES (
        requested_ledger_id,
        requested_sequence,
        requested_previous_chain_head,
        requested_chain_head,
        requested_integrity_key_id,
        requested_checkpoint_mac
    )
$$;

REVOKE ALL ON TABLE security_ledger_checkpoints FROM PUBLIC;
REVOKE ALL ON TABLE security_ledger_checkpoints
    FROM trpg_witness_append_service, trpg_witness_read_service;
REVOKE ALL ON FUNCTION public.reject_security_ledger_checkpoint_mutation() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.enforce_security_ledger_checkpoint_chain() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.latest_security_ledger_checkpoint(TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.append_security_ledger_checkpoint(
    TEXT, BIGINT, TEXT, TEXT, TEXT, TEXT
) FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_append_service') THEN
        GRANT EXECUTE ON FUNCTION public.latest_security_ledger_checkpoint(TEXT)
            TO trpg_witness_append_service;
        GRANT EXECUTE ON FUNCTION public.append_security_ledger_checkpoint(
            TEXT, BIGINT, TEXT, TEXT, TEXT, TEXT
        ) TO trpg_witness_append_service;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_read_service') THEN
        GRANT EXECUTE ON FUNCTION public.latest_security_ledger_checkpoint(TEXT)
            TO trpg_witness_read_service;
        GRANT EXECUTE ON FUNCTION public.append_security_ledger_checkpoint(
            TEXT, BIGINT, TEXT, TEXT, TEXT, TEXT
        ) TO trpg_witness_read_service;
    END IF;
END;
$$;
