-- Bind every cloud-bound context byte to the exact audited provider endpoint
-- and model. Historical rows receive explicit unavailable route markers; they
-- never mint runtime authorizations.

-- The preceding migration makes route snapshots and audit rows append-only.
-- This migration is the one authorised schema upgrade that adds route
-- identity to already-retained evidence. Drop only those two row guards inside
-- the migration transaction, perform the deterministic backfill, and restore
-- them before any new runtime row can be accepted. ACCESS EXCLUSIVE locks are
-- retained until commit, so another session cannot exploit the temporary
-- trigger absence or insert a row with nullable binding fields.
LOCK TABLE cloud_egress_route_snapshots, cloud_egress_audit
    IN ACCESS EXCLUSIVE MODE;

DROP TRIGGER IF EXISTS cloud_egress_route_snapshot_append_only
    ON cloud_egress_route_snapshots;
DROP TRIGGER IF EXISTS cloud_egress_audit_append_only
    ON cloud_egress_audit;

ALTER TABLE cloud_egress_route_snapshots
    ADD COLUMN source_endpoint TEXT,
    ADD COLUMN target_endpoint TEXT,
    ADD COLUMN model_id TEXT,
    ADD COLUMN source_credential_id TEXT,
    ADD COLUMN source_credential_version BIGINT,
    ADD COLUMN target_credential_id TEXT,
    ADD COLUMN target_credential_version BIGINT,
    ADD COLUMN fallback_policy TEXT,
    ADD COLUMN privacy_boundary TEXT,
    ADD COLUMN consent_expires_at_unix_ms BIGINT;

UPDATE cloud_egress_route_snapshots
   SET source_endpoint = 'http://127.0.0.1/historical-unavailable',
       target_endpoint = 'https://historical.invalid',
       model_id = 'historical_unavailable',
       source_credential_id = 'historical_unavailable',
       source_credential_version = 1,
       target_credential_id = 'historical_unavailable',
       target_credential_version = 1,
       fallback_policy = 'historical_unavailable',
       privacy_boundary = 'historical_unavailable',
       consent_expires_at_unix_ms = CASE
           WHEN consent_id IS NULL THEN NULL
           ELSE created_at_unix_ms + 1
       END;

ALTER TABLE cloud_egress_route_snapshots
    ALTER COLUMN source_endpoint SET NOT NULL,
    ALTER COLUMN target_endpoint SET NOT NULL,
    ALTER COLUMN model_id SET NOT NULL,
    ALTER COLUMN source_credential_id SET NOT NULL,
    ALTER COLUMN source_credential_version SET NOT NULL,
    ALTER COLUMN target_credential_id SET NOT NULL,
    ALTER COLUMN target_credential_version SET NOT NULL,
    ALTER COLUMN fallback_policy SET NOT NULL,
    ALTER COLUMN privacy_boundary SET NOT NULL,
    ADD CONSTRAINT cloud_egress_route_security_binding_valid CHECK (
        btrim(source_credential_id) <> ''
        AND source_credential_version > 0
        AND btrim(target_credential_id) <> ''
        AND target_credential_version > 0
        AND btrim(fallback_policy) <> ''
        AND btrim(privacy_boundary) <> ''
        AND (consent_id IS NULL OR consent_expires_at_unix_ms > created_at_unix_ms)
    ),
    ADD CONSTRAINT cloud_egress_route_endpoint_model_valid CHECK (
        source_endpoint ~ '^https?://(localhost|127\.0\.0\.1|\[::1\])([/:]|$)'
        AND target_endpoint ~ '^https://[^/@:[:space:]]+([/:]|$)'
        AND btrim(model_id) <> ''
    );

ALTER TABLE cloud_egress_audit
    ADD COLUMN source_provider TEXT,
    ADD COLUMN target_provider TEXT,
    ADD COLUMN source_endpoint TEXT,
    ADD COLUMN target_endpoint TEXT,
    ADD COLUMN model_id TEXT,
    ADD COLUMN source_credential_id TEXT,
    ADD COLUMN source_credential_version BIGINT,
    ADD COLUMN target_credential_id TEXT,
    ADD COLUMN target_credential_version BIGINT,
    ADD COLUMN fallback_policy TEXT,
    ADD COLUMN privacy_boundary TEXT;

UPDATE cloud_egress_audit AS audit
   SET source_provider = route.source_provider,
       target_provider = route.target_provider,
       source_endpoint = route.source_endpoint,
       target_endpoint = route.target_endpoint,
       model_id = route.model_id,
       source_credential_id = route.source_credential_id,
       source_credential_version = route.source_credential_version,
       target_credential_id = route.target_credential_id,
       target_credential_version = route.target_credential_version,
       fallback_policy = route.fallback_policy,
       privacy_boundary = route.privacy_boundary
  FROM cloud_egress_route_snapshots AS route
 WHERE route.snapshot_id = audit.snapshot_id;

ALTER TABLE cloud_egress_audit
    ALTER COLUMN source_provider SET NOT NULL,
    ALTER COLUMN target_provider SET NOT NULL,
    ALTER COLUMN source_endpoint SET NOT NULL,
    ALTER COLUMN target_endpoint SET NOT NULL,
    ALTER COLUMN model_id SET NOT NULL,
    ALTER COLUMN source_credential_id SET NOT NULL,
    ALTER COLUMN source_credential_version SET NOT NULL,
    ALTER COLUMN target_credential_id SET NOT NULL,
    ALTER COLUMN target_credential_version SET NOT NULL,
    ALTER COLUMN fallback_policy SET NOT NULL,
    ALTER COLUMN privacy_boundary SET NOT NULL,
    ADD CONSTRAINT cloud_egress_audit_security_binding_valid CHECK (
        btrim(source_credential_id) <> ''
        AND source_credential_version > 0
        AND btrim(target_credential_id) <> ''
        AND target_credential_version > 0
        AND btrim(fallback_policy) <> ''
        AND btrim(privacy_boundary) <> ''
    ),
    ADD CONSTRAINT cloud_egress_audit_endpoint_model_valid CHECK (
        source_endpoint ~ '^https?://(localhost|127\.0\.0\.1|\[::1\])([/:]|$)'
        AND target_endpoint ~ '^https://[^/@:[:space:]]+([/:]|$)'
        AND btrim(source_provider) <> ''
        AND btrim(target_provider) <> ''
        AND btrim(model_id) <> ''
    );

CREATE TRIGGER cloud_egress_route_snapshot_append_only
BEFORE UPDATE OR DELETE ON cloud_egress_route_snapshots
FOR EACH ROW EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

CREATE TRIGGER cloud_egress_audit_append_only
BEFORE UPDATE OR DELETE ON cloud_egress_audit
FOR EACH ROW EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

CREATE OR REPLACE FUNCTION enforce_cloud_egress_route_audit_binding()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    route public.cloud_egress_route_snapshots%ROWTYPE;
BEGIN
    SELECT * INTO route
      FROM public.cloud_egress_route_snapshots
     WHERE snapshot_id = NEW.snapshot_id;
    IF NOT FOUND
       OR NEW.subject_id IS DISTINCT FROM route.subject_id
       OR NEW.source_provider IS DISTINCT FROM route.source_provider
       OR NEW.target_provider IS DISTINCT FROM route.target_provider
       OR NEW.source_endpoint IS DISTINCT FROM route.source_endpoint
       OR NEW.target_endpoint IS DISTINCT FROM route.target_endpoint
       OR NEW.model_id IS DISTINCT FROM route.model_id
       OR NEW.source_credential_id IS DISTINCT FROM route.source_credential_id
       OR NEW.source_credential_version IS DISTINCT FROM route.source_credential_version
       OR NEW.target_credential_id IS DISTINCT FROM route.target_credential_id
       OR NEW.target_credential_version IS DISTINCT FROM route.target_credential_version
       OR NEW.fallback_policy IS DISTINCT FROM route.fallback_policy
       OR NEW.privacy_boundary IS DISTINCT FROM route.privacy_boundary
       OR NEW.decision IS DISTINCT FROM route.decision
       OR NEW.denial_code IS DISTINCT FROM route.denial_code
       OR NEW.context_manifest_hash IS DISTINCT FROM route.context_manifest_hash
       OR NEW.created_at_unix_ms IS DISTINCT FROM route.created_at_unix_ms THEN
        RAISE EXCEPTION 'cloud egress audit does not match route snapshot';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS cloud_egress_route_audit_binding ON cloud_egress_audit;
CREATE TRIGGER cloud_egress_route_audit_binding
BEFORE INSERT ON cloud_egress_audit
FOR EACH ROW EXECUTE FUNCTION enforce_cloud_egress_route_audit_binding();

DROP TRIGGER IF EXISTS cloud_egress_consents_no_truncate ON cloud_egress_consents;
CREATE TRIGGER cloud_egress_consents_no_truncate
BEFORE TRUNCATE ON cloud_egress_consents
FOR EACH STATEMENT EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

DROP TRIGGER IF EXISTS cloud_egress_route_snapshots_no_truncate ON cloud_egress_route_snapshots;
CREATE TRIGGER cloud_egress_route_snapshots_no_truncate
BEFORE TRUNCATE ON cloud_egress_route_snapshots
FOR EACH STATEMENT EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

DROP TRIGGER IF EXISTS cloud_egress_audit_no_truncate ON cloud_egress_audit;
CREATE TRIGGER cloud_egress_audit_no_truncate
BEFORE TRUNCATE ON cloud_egress_audit
FOR EACH STATEMENT EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

CREATE TABLE cloud_egress_notices (
    notice_reference TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    notice_digest TEXT NOT NULL
        CHECK (notice_digest ~ '^sha256:[0-9a-f]{64}$'),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER cloud_egress_notices_append_only
BEFORE UPDATE OR DELETE ON cloud_egress_notices
FOR EACH ROW EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();

CREATE TRIGGER cloud_egress_notices_no_truncate
BEFORE TRUNCATE ON cloud_egress_notices
FOR EACH STATEMENT EXECUTE FUNCTION reject_cloud_egress_evidence_mutation();
