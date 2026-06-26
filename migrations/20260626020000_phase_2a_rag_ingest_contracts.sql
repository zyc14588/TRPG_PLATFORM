CREATE TABLE IF NOT EXISTS document_sources (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    source_kind text NOT NULL CHECK (source_kind IN ('official_srd', 'open_license', 'user_upload', 'commercial_adapter', 'unknown')),
    title text NOT NULL,
    source_url text,
    license_name text,
    license_url text,
    license_status text NOT NULL DEFAULT 'pending_review' CHECK (license_status IN ('allowed', 'pending_review', 'denied')),
    visibility_scope text NOT NULL,
    content_hash text NOT NULL,
    declared_has_rights boolean NOT NULL DEFAULT false,
    commercial_adapter_only boolean NOT NULL DEFAULT false,
    created_by uuid REFERENCES users(id),
    audit_log_id uuid REFERENCES audit_logs(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (room_id, content_hash)
);

CREATE TABLE IF NOT EXISTS ingest_jobs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id uuid REFERENCES rooms(id) ON DELETE CASCADE,
    idempotency_key text NOT NULL,
    request_hash text NOT NULL,
    source_id uuid REFERENCES document_sources(id) ON DELETE SET NULL,
    document_id uuid REFERENCES documents(id) ON DELETE SET NULL,
    status text NOT NULL DEFAULT 'claimed' CHECK (status IN ('claimed', 'parsing', 'embedding', 'indexed', 'pending_review', 'failed')),
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    lease_until timestamptz,
    created_by uuid REFERENCES users(id),
    audit_log_id uuid REFERENCES audit_logs(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    UNIQUE (room_id, idempotency_key)
);

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS source_id uuid REFERENCES document_sources(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS source_kind text,
    ADD COLUMN IF NOT EXISTS license_status text,
    ADD COLUMN IF NOT EXISTS audit_log_id uuid REFERENCES audit_logs(id),
    ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now();

UPDATE documents
SET license_status = status
WHERE license_status IS NULL;

ALTER TABLE documents
    ALTER COLUMN license_status SET DEFAULT 'pending_review';

ALTER TABLE chunks
    ADD COLUMN IF NOT EXISTS source_id uuid REFERENCES document_sources(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS content_hash text,
    ADD COLUMN IF NOT EXISTS license_status text DEFAULT 'pending_review',
    ADD COLUMN IF NOT EXISTS license_name text,
    ADD COLUMN IF NOT EXISTS source_url text,
    ADD COLUMN IF NOT EXISTS audit_log_id uuid REFERENCES audit_logs(id);

UPDATE chunks
SET content_hash = 'sha256:' || encode(digest(content, 'sha256'), 'hex')
WHERE content_hash IS NULL;

UPDATE chunks
SET license_status = 'pending_review'
WHERE license_status IS NULL;

ALTER TABLE chunks
    ALTER COLUMN content_hash SET NOT NULL,
    ALTER COLUMN license_status SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'documents_source_kind_check'
    ) THEN
        ALTER TABLE documents
            ADD CONSTRAINT documents_source_kind_check
            CHECK (source_kind IS NULL OR source_kind IN ('official_srd', 'open_license', 'user_upload', 'commercial_adapter', 'unknown'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'documents_license_status_check'
    ) THEN
        ALTER TABLE documents
            ADD CONSTRAINT documents_license_status_check
            CHECK (license_status IN ('allowed', 'pending_review', 'denied'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chunks_license_status_check'
    ) THEN
        ALTER TABLE chunks
            ADD CONSTRAINT chunks_license_status_check
            CHECK (license_status IN ('allowed', 'pending_review', 'denied'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS document_sources_room_visibility_idx ON document_sources(room_id, visibility_scope, license_status);
CREATE INDEX IF NOT EXISTS document_sources_content_hash_idx ON document_sources(content_hash);
CREATE INDEX IF NOT EXISTS documents_source_id_idx ON documents(source_id);
CREATE INDEX IF NOT EXISTS documents_license_status_idx ON documents(room_id, license_status, visibility_scope);
CREATE INDEX IF NOT EXISTS chunks_source_id_idx ON chunks(source_id);
CREATE INDEX IF NOT EXISTS chunks_content_hash_idx ON chunks(document_id, content_hash);
CREATE INDEX IF NOT EXISTS chunks_license_status_idx ON chunks(room_id, license_status, visibility_scope);
CREATE INDEX IF NOT EXISTS ingest_jobs_status_idx ON ingest_jobs(room_id, status, updated_at);
CREATE INDEX IF NOT EXISTS ingest_jobs_source_document_idx ON ingest_jobs(source_id, document_id);

ALTER TABLE document_sources ENABLE ROW LEVEL SECURITY;
ALTER TABLE ingest_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE document_sources FORCE ROW LEVEL SECURITY;
ALTER TABLE ingest_jobs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS document_sources_visibility_select ON document_sources;
CREATE POLICY document_sources_visibility_select ON document_sources
    FOR SELECT
    USING (
        (
            room_id IS NULL
            AND visibility_scope = 'public_rule'
            AND license_status = 'allowed'
        )
        OR (
            room_id = app.current_room_id()
            AND app.has_room_role(room_id)
            AND app.can_view_visibility(visibility_scope, created_by)
        )
    );

DROP POLICY IF EXISTS document_sources_kp_write ON document_sources;
CREATE POLICY document_sources_kp_write ON document_sources
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );

DROP POLICY IF EXISTS ingest_jobs_kp_access ON ingest_jobs;
CREATE POLICY ingest_jobs_kp_access ON ingest_jobs
    FOR ALL
    USING (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    )
    WITH CHECK (
        room_id = app.current_room_id()
        AND app.has_room_role(room_id)
        AND app.is_kp_role(app.current_room_role())
    );
