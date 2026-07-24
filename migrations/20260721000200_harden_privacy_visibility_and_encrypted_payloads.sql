CREATE TABLE IF NOT EXISTS privacy_deletion_jobs (
    job_id TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    retention_policy TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'requested', 'blocked_legal_hold', 'running', 'verifying', 'completed', 'failed'
    )),
    failure_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS privacy_deletion_jobs_subject_idx
    ON privacy_deletion_jobs(subject_id, created_at DESC);

CREATE TABLE IF NOT EXISTS privacy_deletion_job_targets (
    job_id TEXT NOT NULL REFERENCES privacy_deletion_jobs(job_id) ON DELETE CASCADE,
    target TEXT NOT NULL CHECK (target IN (
        'database', 'rag_index', 'object_storage', 'cache', 'queue', 'export', 'backup_key'
    )),
    status TEXT NOT NULL CHECK (status IN ('pending', 'deleted', 'verified', 'failed')),
    error_code TEXT,
    deleted_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    PRIMARY KEY (job_id, target)
);

CREATE TABLE IF NOT EXISTS privacy_legal_holds (
    subject_id TEXT PRIMARY KEY,
    hold_reference TEXT NOT NULL,
    active BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS privacy_deletion_surface_records (
    surface TEXT NOT NULL CHECK (surface IN ('database', 'rag_index', 'cache', 'queue')),
    subject_id TEXT NOT NULL,
    record_key TEXT NOT NULL,
    protected_payload BYTEA NOT NULL,
    PRIMARY KEY (surface, subject_id, record_key)
);

CREATE TABLE IF NOT EXISTS privacy_subject_keys (
    subject_id TEXT PRIMARY KEY,
    key_reference TEXT NOT NULL,
    wrapped_key BYTEA,
    destroyed_at TIMESTAMPTZ,
    CHECK ((wrapped_key IS NOT NULL AND destroyed_at IS NULL)
        OR (wrapped_key IS NULL AND destroyed_at IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS cloud_egress_consents (
    consent_id TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    target_provider TEXT NOT NULL,
    purpose TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    visibility_scope TEXT NOT NULL CHECK (visibility_scope IN ('public_only', 'subject_private')),
    granted BOOLEAN NOT NULL,
    expires_at_unix_ms BIGINT NOT NULL CHECK (expires_at_unix_ms > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS cloud_egress_consents_lookup_idx
    ON cloud_egress_consents(
        subject_id, target_provider, purpose, policy_version, granted, expires_at_unix_ms
    );

CREATE TABLE IF NOT EXISTS cloud_egress_route_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    consent_id TEXT REFERENCES cloud_egress_consents(consent_id),
    source_provider TEXT NOT NULL,
    target_provider TEXT NOT NULL,
    purpose TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    notice_reference TEXT,
    context_manifest_hash TEXT NOT NULL CHECK (length(context_manifest_hash) = 64),
    allowed_fact_ids JSONB NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    denial_code TEXT,
    created_at_unix_ms BIGINT NOT NULL,
    CHECK ((decision = 'allow' AND consent_id IS NOT NULL AND notice_reference IS NOT NULL
            AND denial_code IS NULL)
        OR (decision = 'deny' AND denial_code IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS cloud_egress_audit (
    audit_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES cloud_egress_route_snapshots(snapshot_id),
    subject_id TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    denial_code TEXT,
    context_manifest_hash TEXT NOT NULL CHECK (length(context_manifest_hash) = 64),
    created_at_unix_ms BIGINT NOT NULL,
    CHECK ((decision = 'allow' AND denial_code IS NULL)
        OR (decision = 'deny' AND denial_code IS NOT NULL))
);

CREATE OR REPLACE FUNCTION enforce_cloud_egress_consent_transition()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'cloud egress consent history is immutable';
    END IF;
    IF NEW.consent_id IS DISTINCT FROM OLD.consent_id
       OR NEW.subject_id IS DISTINCT FROM OLD.subject_id
       OR NEW.target_provider IS DISTINCT FROM OLD.target_provider
       OR NEW.purpose IS DISTINCT FROM OLD.purpose
       OR NEW.policy_version IS DISTINCT FROM OLD.policy_version
       OR NEW.visibility_scope IS DISTINCT FROM OLD.visibility_scope
       OR NEW.expires_at_unix_ms IS DISTINCT FROM OLD.expires_at_unix_ms
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR (OLD.granted = false AND NEW.granted = true) THEN
        RAISE EXCEPTION 'cloud egress consent identity is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER cloud_egress_consent_transition_guard
BEFORE UPDATE OR DELETE ON cloud_egress_consents
FOR EACH ROW EXECUTE FUNCTION enforce_cloud_egress_consent_transition();

CREATE OR REPLACE FUNCTION reject_cloud_egress_evidence_mutation()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    RAISE EXCEPTION 'cloud egress route and audit evidence is append-only';
END;
$$;

CREATE TRIGGER cloud_egress_route_snapshot_append_only
BEFORE UPDATE OR DELETE ON cloud_egress_route_snapshots
FOR EACH ROW EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

CREATE TRIGGER cloud_egress_audit_append_only
BEFORE UPDATE OR DELETE ON cloud_egress_audit
FOR EACH ROW EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

ALTER TABLE IF EXISTS event_store
    ADD COLUMN IF NOT EXISTS visibility_subject TEXT,
    ADD COLUMN IF NOT EXISTS payload_ciphertext BYTEA,
    ADD COLUMN IF NOT EXISTS payload_key_reference TEXT,
    ADD COLUMN IF NOT EXISTS payload_nonce BYTEA;

ALTER TABLE IF EXISTS event_outbox
    ADD COLUMN IF NOT EXISTS visibility_subject TEXT,
    ADD COLUMN IF NOT EXISTS payload_ciphertext BYTEA,
    ADD COLUMN IF NOT EXISTS payload_key_reference TEXT,
    ADD COLUMN IF NOT EXISTS payload_nonce BYTEA;

-- Older schemas stored the visibility target only on the canonical event and
-- had no separate encrypted-payload columns. Align every existing outbox row
-- with its event before installing the stricter binding trigger. Historical
-- plaintext remains classified as historical and is handled fail-closed by
-- replay; this migration never invents key material or fabricates ciphertext.
UPDATE event_outbox AS outbox
   SET visibility_subject = event.visibility_subject,
       payload_ciphertext = event.payload_ciphertext,
       payload_key_reference = event.payload_key_reference,
       payload_nonce = event.payload_nonce
  FROM event_store AS event
 WHERE outbox.event_sequence = event.sequence
   AND ROW(
       outbox.visibility_subject, outbox.payload_ciphertext,
       outbox.payload_key_reference, outbox.payload_nonce
   ) IS DISTINCT FROM ROW(
       event.visibility_subject, event.payload_ciphertext,
       event.payload_key_reference, event.payload_nonce
   );

DO $$
BEGIN
    IF to_regclass('event_store') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM pg_constraint WHERE conname = 'event_store_verified_payload_encrypted'
       ) THEN
        ALTER TABLE event_store ADD CONSTRAINT event_store_verified_payload_encrypted
            CHECK (integrity_status <> 'verified_hmac'
                OR ((payload_json::jsonb ? 'protected_payload')
                    AND payload_ciphertext IS NOT NULL
                    AND octet_length(payload_ciphertext) >= 16
                    AND payload_key_reference IS NOT NULL
                    AND length(payload_key_reference) > 0
                    AND payload_nonce IS NOT NULL
                    AND octet_length(payload_nonce) = 12)) NOT VALID;
    END IF;
    IF to_regclass('event_outbox') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM pg_constraint WHERE conname = 'event_outbox_verified_payload_encrypted'
       ) THEN
        ALTER TABLE event_outbox ADD CONSTRAINT event_outbox_verified_payload_encrypted
            CHECK (integrity_status <> 'verified_hmac'
                OR ((payload_json::jsonb ? 'protected_payload')
                    AND payload_ciphertext IS NOT NULL
                    AND octet_length(payload_ciphertext) >= 16
                    AND payload_key_reference IS NOT NULL
                    AND length(payload_key_reference) > 0
                    AND payload_nonce IS NOT NULL
                    AND octet_length(payload_nonce) = 12)) NOT VALID;
    END IF;
END $$;

CREATE OR REPLACE FUNCTION enforce_event_outbox_binding()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
BEGIN
    IF TG_OP = 'UPDATE' AND ROW(
        NEW.outbox_id, NEW.event_id, NEW.event_sequence, NEW.nats_subject,
        NEW.idempotency_key, NEW.visibility_label, NEW.visibility_subject,
        NEW.correlation_id, NEW.causation_id, NEW.payload_json,
        NEW.payload_ciphertext, NEW.payload_key_reference, NEW.payload_nonce,
        NEW.commit_id, NEW.campaign_id, NEW.stream_id, NEW.event_schema_version,
        NEW.idempotency_operation, NEW.request_hash, NEW.request_hash_source,
        NEW.integrity_status
    ) IS DISTINCT FROM ROW(
        OLD.outbox_id, OLD.event_id, OLD.event_sequence, OLD.nats_subject,
        OLD.idempotency_key, OLD.visibility_label, OLD.visibility_subject,
        OLD.correlation_id, OLD.causation_id, OLD.payload_json,
        OLD.payload_ciphertext, OLD.payload_key_reference, OLD.payload_nonce,
        OLD.commit_id, OLD.campaign_id, OLD.stream_id, OLD.event_schema_version,
        OLD.idempotency_operation, OLD.request_hash, OLD.request_hash_source,
        OLD.integrity_status
    ) THEN
        RAISE EXCEPTION 'canonical outbox identity is immutable';
    END IF;

    SELECT * INTO stored_event
      FROM public.event_store
     WHERE sequence = NEW.event_sequence;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'outbox event does not exist';
    END IF;
    IF NEW.event_id IS DISTINCT FROM stored_event.sequence
       OR NEW.event_sequence IS DISTINCT FROM stored_event.sequence
       OR NEW.nats_subject IS DISTINCT FROM 'trpg.events.appended'
       OR NEW.campaign_id IS DISTINCT FROM stored_event.campaign_id
       OR NEW.stream_id IS DISTINCT FROM stored_event.stream_id
       OR NEW.event_schema_version IS DISTINCT FROM stored_event.event_schema_version
       OR NEW.idempotency_operation IS DISTINCT FROM stored_event.idempotency_operation
       OR NEW.visibility_label IS DISTINCT FROM stored_event.visibility_label
       OR NEW.visibility_subject IS DISTINCT FROM stored_event.visibility_subject
       OR NEW.correlation_id IS DISTINCT FROM stored_event.correlation_id
       OR NEW.causation_id IS DISTINCT FROM stored_event.causation_id
       OR NEW.payload_json IS DISTINCT FROM stored_event.payload_json
       OR NEW.payload_ciphertext IS DISTINCT FROM stored_event.payload_ciphertext
       OR NEW.payload_key_reference IS DISTINCT FROM stored_event.payload_key_reference
       OR NEW.payload_nonce IS DISTINCT FROM stored_event.payload_nonce
       OR NEW.request_hash IS DISTINCT FROM stored_event.request_hash
       OR NEW.request_hash_source IS DISTINCT FROM stored_event.request_hash_source
       OR NEW.integrity_status IS DISTINCT FROM stored_event.integrity_status THEN
        RAISE EXCEPTION 'outbox metadata does not match canonical event';
    END IF;
    RETURN NEW;
END;
$$;
