-- P2 Batch 02: additive RAG storage contract hardening.

ALTER TABLE document_sources
    ADD COLUMN IF NOT EXISTS license_reason text,
    ADD COLUMN IF NOT EXISTS visibility_default text,
    ADD COLUMN IF NOT EXISTS metadata jsonb NOT NULL DEFAULT '{}'::jsonb;

UPDATE document_sources
SET visibility_default = visibility_scope
WHERE visibility_default IS NULL;

ALTER TABLE document_sources
    ALTER COLUMN visibility_default SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL;

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS normalized_hash text,
    ADD COLUMN IF NOT EXISTS visibility text,
    ADD COLUMN IF NOT EXISTS provider_metadata jsonb NOT NULL DEFAULT '{}'::jsonb;

UPDATE documents
SET normalized_hash = content_hash
WHERE normalized_hash IS NULL;

UPDATE documents
SET visibility = visibility_scope
WHERE visibility IS NULL;

ALTER TABLE documents
    ALTER COLUMN normalized_hash SET NOT NULL,
    ALTER COLUMN visibility SET NOT NULL,
    ALTER COLUMN provider_metadata SET NOT NULL;

ALTER TABLE chunks
    ADD COLUMN IF NOT EXISTS ordinal integer,
    ADD COLUMN IF NOT EXISTS heading_path text[],
    ADD COLUMN IF NOT EXISTS visibility text,
    ADD COLUMN IF NOT EXISTS token_estimate integer,
    ADD COLUMN IF NOT EXISTS citation jsonb NOT NULL DEFAULT '{}'::jsonb;

WITH numbered AS (
    SELECT
        id,
        row_number() OVER (PARTITION BY document_id ORDER BY created_at, id) - 1 AS next_ordinal
    FROM chunks
    WHERE ordinal IS NULL
)
UPDATE chunks c
SET ordinal = numbered.next_ordinal::integer
FROM numbered
WHERE c.id = numbered.id;

UPDATE chunks
SET heading_path = CASE
    WHEN jsonb_typeof(section_path) = 'array' THEN ARRAY(SELECT jsonb_array_elements_text(chunks.section_path))
    ELSE '{}'::text[]
END
WHERE heading_path IS NULL;

UPDATE chunks
SET visibility = visibility_scope
WHERE visibility IS NULL;

UPDATE chunks
SET token_estimate = GREATEST(1, CEIL(char_length(content)::numeric / 4)::integer)
WHERE token_estimate IS NULL;

UPDATE chunks
SET citation = jsonb_build_object(
        'heading_path', heading_path,
        'content_hash', content_hash,
        'location', concat('chunk ', ordinal + 1)
    )
WHERE citation = '{}'::jsonb;

ALTER TABLE chunks
    ALTER COLUMN ordinal SET NOT NULL,
    ALTER COLUMN heading_path SET DEFAULT '{}'::text[],
    ALTER COLUMN heading_path SET NOT NULL,
    ALTER COLUMN visibility SET NOT NULL,
    ALTER COLUMN token_estimate SET NOT NULL,
    ALTER COLUMN citation SET NOT NULL;

ALTER TABLE ingest_jobs
    ADD COLUMN IF NOT EXISTS error_code text,
    ADD COLUMN IF NOT EXISTS error_message text,
    ADD COLUMN IF NOT EXISTS chunk_count integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS provider_metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS response_json jsonb;

UPDATE ingest_jobs
SET error_message = last_error
WHERE error_message IS NULL AND last_error IS NOT NULL;

ALTER TABLE document_sources DROP CONSTRAINT IF EXISTS document_sources_source_kind_check;
ALTER TABLE document_sources
    ADD CONSTRAINT document_sources_source_kind_check
    CHECK (source_kind IN (
        'official_srd',
        'open_license',
        'open_text',
        'user_upload',
        'user_provided_text',
        'campaign_notes',
        'character_sheet',
        'module_private_notes',
        'commercial_adapter',
        'commercial_adapter_metadata',
        'unknown'
    ));

ALTER TABLE documents DROP CONSTRAINT IF EXISTS documents_source_kind_check;
ALTER TABLE documents
    ADD CONSTRAINT documents_source_kind_check
    CHECK (
        source_kind IS NULL
        OR source_kind IN (
            'official_srd',
            'open_license',
            'open_text',
            'user_upload',
            'user_provided_text',
            'campaign_notes',
            'character_sheet',
            'module_private_notes',
            'commercial_adapter',
            'commercial_adapter_metadata',
            'unknown'
        )
    );

ALTER TABLE ingest_jobs DROP CONSTRAINT IF EXISTS ingest_jobs_status_check;
ALTER TABLE ingest_jobs
    ADD CONSTRAINT ingest_jobs_status_check
    CHECK (status IN ('claimed', 'parsing', 'embedding', 'indexed', 'pending_review', 'denied', 'failed'));

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'document_sources_visibility_default_check'
    ) THEN
        ALTER TABLE document_sources
            ADD CONSTRAINT document_sources_visibility_default_check
            CHECK (visibility_default IN (
                'public_rule',
                'room_rule',
                'pl_visible_clue',
                'kp_only_module',
                'kp_secret',
                'character_private',
                'session_log',
                'memory_private',
                'system_internal'
            ));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'documents_visibility_check'
    ) THEN
        ALTER TABLE documents
            ADD CONSTRAINT documents_visibility_check
            CHECK (visibility IN (
                'public_rule',
                'room_rule',
                'pl_visible_clue',
                'kp_only_module',
                'kp_secret',
                'character_private',
                'session_log',
                'memory_private',
                'system_internal'
            ));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chunks_visibility_check'
    ) THEN
        ALTER TABLE chunks
            ADD CONSTRAINT chunks_visibility_check
            CHECK (visibility IN (
                'public_rule',
                'room_rule',
                'pl_visible_clue',
                'kp_only_module',
                'kp_secret',
                'character_private',
                'session_log',
                'memory_private',
                'system_internal'
            ));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS document_sources_room_license_idx ON document_sources(room_id, license_status);
CREATE INDEX IF NOT EXISTS documents_room_source_license_idx ON documents(room_id, source_id, license_status);
CREATE INDEX IF NOT EXISTS chunks_room_document_visibility_idx ON chunks(room_id, document_id, visibility);
CREATE INDEX IF NOT EXISTS chunks_room_source_idx ON chunks(room_id, source_id);
ALTER TABLE ingest_jobs DROP CONSTRAINT IF EXISTS ingest_jobs_room_id_idempotency_key_key;

CREATE UNIQUE INDEX IF NOT EXISTS ingest_jobs_room_created_by_idempotency_idx
    ON ingest_jobs(room_id, created_by, idempotency_key);
