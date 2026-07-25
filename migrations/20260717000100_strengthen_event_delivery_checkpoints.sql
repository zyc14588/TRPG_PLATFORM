-- Forward-only event delivery and projection recovery hardening.
--
-- Canonical events remain append-only. Application rollback may leave these
-- columns and guards in place; it must never drop Event Store history.

-- Actor identity is one authenticated-principal tuple. Historical rows did
-- not persist role/origin, so they receive an explicit unknown provenance;
-- no audit approver metadata is ever spliced into that tuple.
ALTER TABLE public.event_store
    ADD COLUMN authenticated_actor_role TEXT NOT NULL
        DEFAULT 'historical_unknown',
    ADD COLUMN authenticated_actor_origin JSONB NOT NULL
        DEFAULT '{"kind":"workload","role":"historical_unknown"}'::jsonb;

ALTER TABLE public.event_store
    ALTER COLUMN authenticated_actor_role DROP DEFAULT,
    ALTER COLUMN authenticated_actor_origin DROP DEFAULT,
    ADD CONSTRAINT event_store_actor_role_valid CHECK (
        authenticated_actor_role IN (
            'server_owner', 'campaign_owner', 'human_keeper', 'ai_keeper',
            'investigator', 'moderator', 'spectator', 'workflow',
            'rules_engine', 'system', 'historical_unknown'
        )
    ),
    ADD CONSTRAINT event_store_actor_origin_valid CHECK (
        jsonb_typeof(authenticated_actor_origin) = 'object'
        AND (
            authenticated_actor_role = 'historical_unknown'
            AND authenticated_actor_origin =
                '{"kind":"workload","role":"historical_unknown"}'::jsonb
            OR authenticated_actor_origin->>'kind' = 'user_session'
            AND authenticated_actor_origin ?& ARRAY['kind', 'session_id']
            AND authenticated_actor_origin - ARRAY['kind', 'session_id'] = '{}'::jsonb
            AND btrim(authenticated_actor_origin->>'session_id') <> ''
            AND authenticated_actor_role IN (
                'server_owner', 'campaign_owner', 'human_keeper',
                'investigator', 'moderator', 'spectator'
            )
            OR authenticated_actor_origin->>'kind' = 'workload'
            AND authenticated_actor_origin ?& ARRAY['kind', 'role']
            AND authenticated_actor_origin - ARRAY['kind', 'role'] = '{}'::jsonb
            AND (
                authenticated_actor_role = 'workflow'
                AND authenticated_actor_origin->>'role' = 'workflow_engine'
                OR authenticated_actor_role = 'rules_engine'
                AND authenticated_actor_origin->>'role' = 'rules_engine'
                OR authenticated_actor_role = 'system'
                AND authenticated_actor_origin->>'role' IN (
                    'api_server', 'realtime_server', 'agent_worker',
                    'audit_writer'
                )
            )
            OR authenticated_actor_origin->>'kind' = 'agent_run'
            AND authenticated_actor_origin ?& ARRAY[
                'kind', 'run_id', 'class', 'campaign_id'
            ]
            AND authenticated_actor_origin - ARRAY[
                'kind', 'run_id', 'class', 'campaign_id'
            ] = '{}'::jsonb
            AND btrim(authenticated_actor_origin->>'run_id') <> ''
            AND authenticated_actor_origin->>'campaign_id' = campaign_id
            AND (
                authenticated_actor_role = 'ai_keeper'
                AND authenticated_actor_origin->>'class' =
                    'ai_keeper_orchestrator'
                OR authenticated_actor_role = 'investigator'
                AND authenticated_actor_origin->>'class' IN (
                    'keeper_copilot', 'atmosphere_writer', 'memory_curator'
                )
            )
        )
    );

-- Correlation and causation are part of the durable policy decision evidence,
-- not transient log decoration. Existing v1/v2 records retain their original
-- HMAC input and receive an explicit historical value; new v3 records bind both
-- identifiers together with the database timestamp.
ALTER TABLE public.canonical_audit_log
    ADD COLUMN correlation_id TEXT NOT NULL DEFAULT 'historical_unknown',
    ADD COLUMN causation_id TEXT NOT NULL DEFAULT 'historical_unknown';

ALTER TABLE public.canonical_audit_log
    ALTER COLUMN correlation_id DROP DEFAULT,
    ALTER COLUMN causation_id DROP DEFAULT,
    DROP CONSTRAINT canonical_audit_log_integrity_version_valid,
    ALTER COLUMN integrity_version SET DEFAULT 3,
    ADD CONSTRAINT canonical_audit_log_integrity_version_valid
        CHECK (integrity_version IN (1, 2, 3)),
    ADD CONSTRAINT canonical_audit_log_context_valid CHECK (
        btrim(correlation_id) <> ''
        AND btrim(causation_id) <> ''
        AND (
            integrity_version IN (1, 2)
            OR correlation_id <> 'historical_unknown'
            AND causation_id <> 'historical_unknown'
        )
    );

CREATE OR REPLACE FUNCTION public.enforce_canonical_audit_chain()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    IF NEW.integrity_version <> 3 THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = 'historical audit integrity version is migration-only';
    END IF;
    PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended('trpg.canonical_audit_log.chain', 0)
    );
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM public.canonical_audit_log
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

ALTER TABLE public.event_outbox
    ADD COLUMN delivery_status TEXT,
    ADD COLUMN available_at TIMESTAMPTZ,
    ADD COLUMN locked_until TIMESTAMPTZ,
    ADD COLUMN claim_token TEXT;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM public.event_outbox
         WHERE published_at IS NOT NULL AND dead_lettered_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'unsupported outbox delivery data: published and dead-lettered';
    END IF;
END;
$$;

UPDATE public.event_outbox
   SET delivery_status = CASE
           WHEN published_at IS NOT NULL THEN 'published'
           WHEN dead_lettered_at IS NOT NULL THEN 'dead_lettered'
           WHEN claimed_at IS NOT NULL
            AND btrim(COALESCE(claim_owner, '')) <> '' THEN 'claimed'
           WHEN retry_count > 0 THEN 'retrying'
           ELSE 'pending'
       END,
       available_at = COALESCE(published_at, dead_lettered_at, claimed_at, now()),
       claimed_at = CASE
           WHEN published_at IS NULL
            AND dead_lettered_at IS NULL
            AND claimed_at IS NOT NULL
            AND btrim(COALESCE(claim_owner, '')) <> '' THEN claimed_at
           ELSE NULL
       END,
       claim_owner = CASE
           WHEN published_at IS NULL
            AND dead_lettered_at IS NULL
            AND claimed_at IS NOT NULL
            AND btrim(COALESCE(claim_owner, '')) <> '' THEN claim_owner
           ELSE NULL
       END,
       claim_token = CASE
           WHEN published_at IS NULL
            AND dead_lettered_at IS NULL
            AND claimed_at IS NOT NULL
            AND btrim(COALESCE(claim_owner, '')) <> ''
               THEN 'migration-claim-' || outbox_id::TEXT
           ELSE NULL
       END,
       locked_until = CASE
           WHEN published_at IS NULL
            AND dead_lettered_at IS NULL
            AND claimed_at IS NOT NULL
            AND btrim(COALESCE(claim_owner, '')) <> ''
               THEN claimed_at + interval '60 seconds'
           ELSE NULL
       END;

ALTER TABLE public.event_outbox
    ALTER COLUMN delivery_status SET NOT NULL,
    ALTER COLUMN delivery_status SET DEFAULT 'pending',
    ALTER COLUMN available_at SET NOT NULL,
    ALTER COLUMN available_at SET DEFAULT now(),
    ADD CONSTRAINT event_outbox_delivery_status_valid CHECK (
        delivery_status IN (
            'pending', 'claimed', 'retrying', 'published', 'dead_lettered'
        )
    ),
    ADD CONSTRAINT event_outbox_delivery_state_consistent CHECK (
        delivery_status = 'pending'
        AND published_at IS NULL
        AND dead_lettered_at IS NULL
        AND claimed_at IS NULL
        AND claim_owner IS NULL
        AND claim_token IS NULL
        AND locked_until IS NULL
        OR delivery_status = 'retrying'
        AND retry_count > 0
        AND published_at IS NULL
        AND dead_lettered_at IS NULL
        AND claimed_at IS NULL
        AND claim_owner IS NULL
        AND claim_token IS NULL
        AND locked_until IS NULL
        OR delivery_status = 'claimed'
        AND published_at IS NULL
        AND dead_lettered_at IS NULL
        AND claimed_at IS NOT NULL
        AND claim_owner IS NOT NULL
        AND btrim(claim_owner) <> ''
        AND claim_token IS NOT NULL
        AND btrim(claim_token) <> ''
        AND length(claim_token) <= 160
        AND locked_until IS NOT NULL
        AND locked_until > claimed_at
        OR delivery_status = 'published'
        AND published_at IS NOT NULL
        AND dead_lettered_at IS NULL
        AND claimed_at IS NULL
        AND claim_owner IS NULL
        AND claim_token IS NULL
        AND locked_until IS NULL
        OR delivery_status = 'dead_lettered'
        AND published_at IS NULL
        AND dead_lettered_at IS NOT NULL
        AND claimed_at IS NULL
        AND claim_owner IS NULL
        AND claim_token IS NULL
        AND locked_until IS NULL
    );

CREATE INDEX event_outbox_claim_ready_idx
    ON public.event_outbox (available_at, locked_until, outbox_id)
    WHERE published_at IS NULL AND dead_lettered_at IS NULL;

-- P04 introduces a new complete projection hash domain and a concrete read
-- model table. A pre-P04 cursor cannot prove that this new read model contains
-- the rows it claims to cover, even when its legacy hash happens to look like
-- SHA-256. Preserve the checkpoint identity, but deliberately reset the
-- rebuildable cursor to genesis so startup replay cannot silently skip Event
-- Store history. Pre-P03 records were normalized into the one canonical
-- historical stream by the preceding migration.
UPDATE public.projection_checkpoint
   SET stream_id = CASE
           WHEN campaign_id = 'historical_unscoped'
               THEN 'historical_unscoped'
           ELSE stream_id
       END,
       version = 0,
       last_event_sequence = 0,
       projection_hash =
           'sha256:0000000000000000000000000000000000000000000000000000000000000000',
       rebuilt_at = now();

ALTER TABLE public.projection_checkpoint
    ADD CONSTRAINT projection_checkpoint_cursor_consistent CHECK (
        version = 0 AND last_event_sequence = 0
        OR version > 0 AND last_event_sequence > 0
    ),
    ADD CONSTRAINT projection_checkpoint_hash_valid CHECK (
        projection_hash ~ '^sha256:[0-9a-f]{64}$'
    );

ALTER TABLE public.event_store
    ADD CONSTRAINT event_store_projection_reference_uq UNIQUE (
        campaign_id, stream_id, stream_version, sequence
    );

-- Align every durable visibility boundary with the authoritative top-level
-- design. These constraints were created by the preceding forward migration,
-- so this unpublished P04 migration replaces them in place.
ALTER TABLE public.event_store
    DROP CONSTRAINT event_store_visibility_label_valid,
    ADD CONSTRAINT event_store_visibility_label_valid CHECK (
        visibility_label IN (
            'public', 'party_visible', 'private_to_player',
            'private_to_group', 'keeper_only', 'ai_internal', 'system_only',
            'spectator_visible', 'spectator_hidden',
            -- Backward-compatible aliases remain readable until their
            -- producers are migrated to the authoritative vocabulary.
            'investigator_private', 'system_private'
        )
    );

ALTER TABLE public.event_outbox
    DROP CONSTRAINT event_outbox_visibility_label_valid,
    ADD CONSTRAINT event_outbox_visibility_label_valid CHECK (
        visibility_label IN (
            'public', 'party_visible', 'private_to_player',
            'private_to_group', 'keeper_only', 'ai_internal', 'system_only',
            'spectator_visible', 'spectator_hidden',
            'investigator_private', 'system_private'
        )
    );

-- A generic, rebuildable read model makes Projection recovery observable.
-- It is deliberately downstream of Event Store and may be deleted/rebuilt.
CREATE TABLE public.canonical_event_projection (
    projection_name TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    stream_id TEXT NOT NULL,
    stream_version BIGINT NOT NULL,
    event_sequence BIGINT NOT NULL,
    projection_hash TEXT NOT NULL,
    event_document JSONB NOT NULL,
    projected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_event_projection_pkey PRIMARY KEY (
        projection_name, campaign_id, stream_id, stream_version
    ),
    CONSTRAINT canonical_event_projection_event_uq UNIQUE (
        projection_name, event_sequence
    ),
    CONSTRAINT canonical_event_projection_event_fkey FOREIGN KEY (
        campaign_id, stream_id, stream_version, event_sequence
    ) REFERENCES public.event_store (
        campaign_id, stream_id, stream_version, sequence
    ),
    CONSTRAINT canonical_event_projection_cursor_valid CHECK (
        stream_version > 0 AND event_sequence > 0
    ),
    CONSTRAINT canonical_event_projection_hash_valid CHECK (
        projection_hash ~ '^sha256:[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_event_projection_identity_valid CHECK (
        btrim(projection_name) <> ''
        AND btrim(campaign_id) <> ''
        AND btrim(stream_id) <> ''
    )
);

CREATE INDEX canonical_event_projection_event_idx
    ON public.canonical_event_projection (event_sequence);

-- RAG is a pgvector-backed, rebuildable read model. Canonical security and
-- provenance metadata is copied from the referenced Event Store row and is
-- verified again by a trigger so a projector cannot relabel hidden facts.
CREATE TABLE public.rag_snapshot_chunk (
    campaign_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    chunk_id TEXT NOT NULL,
    source_event_sequence BIGINT NOT NULL REFERENCES public.event_store(sequence),
    source_type TEXT NOT NULL,
    visibility TEXT NOT NULL,
    visibility_subject TEXT NOT NULL,
    copyright_status TEXT NOT NULL,
    version BIGINT NOT NULL,
    owner TEXT NOT NULL,
    allowed_use TEXT NOT NULL,
    fact_provenance JSONB NOT NULL,
    chunk_hash TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding_model TEXT NOT NULL,
    embedding_dimensions INTEGER NOT NULL,
    embedding vector NOT NULL,
    projected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT rag_snapshot_chunk_pkey PRIMARY KEY (
        campaign_id, snapshot_id, chunk_id
    ),
    CONSTRAINT rag_snapshot_chunk_source_hash_uq UNIQUE (
        campaign_id, snapshot_id, source_event_sequence, chunk_hash
    ),
    CONSTRAINT rag_snapshot_chunk_identity_valid CHECK (
        btrim(campaign_id) <> ''
        AND btrim(snapshot_id) <> ''
        AND btrim(chunk_id) <> ''
        AND length(snapshot_id) <= 160
        AND length(chunk_id) <= 160
    ),
    CONSTRAINT rag_snapshot_chunk_metadata_valid CHECK (
        source_event_sequence > 0
        AND version > 0
        AND source_type ~ '^[a-z][a-z0-9_]{0,127}$'
        AND copyright_status ~ '^[a-z][a-z0-9_]{0,127}$'
        AND btrim(owner) <> ''
        AND allowed_use ~ '^[a-z][a-z0-9_]{0,127}$'
        AND btrim(content) <> ''
        AND length(content) <= 1048576
        AND btrim(embedding_model) <> ''
        AND length(embedding_model) <= 256
        AND embedding_dimensions BETWEEN 1 AND 4096
        AND vector_dims(embedding) = embedding_dimensions
        AND vector_norm(embedding) > 0
        AND embedding::TEXT !~ '(NaN|Infinity)'
        AND chunk_hash ~ '^[0-9a-f]{64}$'
        AND visibility IN (
            'public', 'party_visible', 'private_to_player',
            'private_to_group', 'keeper_only', 'ai_internal', 'system_only',
            'spectator_visible', 'spectator_hidden',
            'investigator_private', 'system_private'
        )
        AND jsonb_typeof(fact_provenance) = 'object'
        AND fact_provenance ?& ARRAY['kind', 'reference', 'recorded_by']
        AND fact_provenance - ARRAY['kind', 'reference', 'recorded_by'] = '{}'::jsonb
        AND btrim(fact_provenance->>'kind') <> ''
        AND btrim(fact_provenance->>'reference') <> ''
        AND btrim(fact_provenance->>'recorded_by') <> ''
    )
);

CREATE INDEX rag_snapshot_chunk_visibility_idx
    ON public.rag_snapshot_chunk (
        campaign_id, snapshot_id, visibility, visibility_subject
    );
CREATE INDEX rag_snapshot_chunk_source_event_idx
    ON public.rag_snapshot_chunk (source_event_sequence);
CREATE INDEX rag_snapshot_chunk_embedding_contract_idx
    ON public.rag_snapshot_chunk (
        campaign_id, snapshot_id, embedding_model, embedding_dimensions
    );

-- One transaction-scoped lock protects the complete delete-and-replace
-- boundary and is also acquired by raw inserts through the trigger below.
-- jsonb framing avoids ambiguous concatenated lock keys.
CREATE OR REPLACE FUNCTION public.lock_rag_snapshot(
    target_campaign_id TEXT,
    target_snapshot_id TEXT
)
RETURNS void
LANGUAGE sql
VOLATILE
SET search_path = pg_catalog, public
AS $$
    SELECT pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(
            pg_catalog.jsonb_build_array(
                'rag_snapshot', target_campaign_id, target_snapshot_id
            )::TEXT,
            0
        )
    );
$$;

CREATE OR REPLACE FUNCTION public.enforce_rag_snapshot_chunk_source()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
BEGIN
    PERFORM public.lock_rag_snapshot(NEW.campaign_id, NEW.snapshot_id);

    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'RAG snapshot chunks are immutable; rebuild the snapshot';
    END IF;

    SELECT * INTO stored_event
      FROM public.event_store
     WHERE sequence = NEW.source_event_sequence;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RAG source event does not exist';
    END IF;
    IF NEW.campaign_id IS DISTINCT FROM stored_event.campaign_id
       OR NEW.visibility IS DISTINCT FROM stored_event.visibility_label
       OR NEW.visibility_subject IS DISTINCT FROM stored_event.visibility_subject
       OR NEW.version IS DISTINCT FROM stored_event.stream_version
       OR NEW.owner IS DISTINCT FROM stored_event.authority_owner
       OR NEW.chunk_hash IS DISTINCT FROM encode(
           sha256(convert_to(NEW.content, 'UTF8')), 'hex'
       )
       OR NEW.fact_provenance IS DISTINCT FROM jsonb_build_object(
           'kind', stored_event.fact_provenance_kind,
           'reference', stored_event.fact_provenance_reference,
           'recorded_by', stored_event.fact_recorded_by
       ) THEN
        RAISE EXCEPTION 'RAG snapshot metadata does not match Event Store';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM public.rag_snapshot_chunk AS existing
         WHERE existing.campaign_id = NEW.campaign_id
           AND existing.snapshot_id = NEW.snapshot_id
           AND (
               existing.embedding_model IS DISTINCT FROM NEW.embedding_model
               OR existing.embedding_dimensions IS DISTINCT FROM NEW.embedding_dimensions
           )
    ) THEN
        RAISE EXCEPTION 'RAG snapshot embedding contract is inconsistent';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER rag_snapshot_chunk_source_guard
BEFORE INSERT OR UPDATE ON public.rag_snapshot_chunk
FOR EACH ROW EXECUTE FUNCTION public.enforce_rag_snapshot_chunk_source();

-- Canonical JSON used by the Rust projection hasher is compact and object
-- keys are lexicographically ordered. Reproduce that representation in
-- PostgreSQL so a trigger can independently verify every chain link rather
-- than trusting a projector-supplied digest.
CREATE OR REPLACE FUNCTION public.canonical_projection_json(value JSONB)
RETURNS TEXT
LANGUAGE plpgsql
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
DECLARE
    value_kind TEXT;
BEGIN
    value_kind := pg_catalog.jsonb_typeof(value);
    IF value_kind = 'object' THEN
        RETURN '{' || COALESCE((
            SELECT pg_catalog.string_agg(
                pg_catalog.to_jsonb(entry.key)::TEXT || ':' ||
                    public.canonical_projection_json(entry.value),
                ',' ORDER BY entry.key COLLATE "C"
            )
              FROM pg_catalog.jsonb_each(value) AS entry(key, value)
        ), '') || '}';
    ELSIF value_kind = 'array' THEN
        RETURN '[' || COALESCE((
            SELECT pg_catalog.string_agg(
                public.canonical_projection_json(entry.value),
                ',' ORDER BY entry.ordinality
            )
              FROM pg_catalog.jsonb_array_elements(value)
                   WITH ORDINALITY AS entry(value, ordinality)
        ), '') || ']';
    END IF;
    RETURN value::TEXT;
END;
$$;

CREATE OR REPLACE FUNCTION public.projection_hash_field(
    field_tag INTEGER,
    field_value BYTEA
)
RETURNS BYTEA
LANGUAGE sql
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
    SELECT pg_catalog.set_byte(pg_catalog.decode('00', 'hex'), 0, field_tag)
        || pg_catalog.int8send(pg_catalog.octet_length(field_value)::BIGINT)
        || field_value;
$$;

CREATE OR REPLACE FUNCTION public.compute_canonical_projection_hash_v3(
    previous_hash TEXT,
    stored_event public.event_store
)
RETURNS TEXT
LANGUAGE plpgsql
IMMUTABLE
STRICT
SET search_path = pg_catalog, public
AS $$
DECLARE
    hash_input BYTEA := ''::BYTEA;
    recorded_at_micros BIGINT;
BEGIN
    IF previous_hash !~ '^sha256:[0-9a-f]{64}$' THEN
        RAISE EXCEPTION 'invalid previous canonical projection hash';
    END IF;

    recorded_at_micros := (
        extract(epoch FROM stored_event.recorded_at)::NUMERIC * 1000000
    )::BIGINT;

    hash_input := hash_input
        || public.projection_hash_field(1, pg_catalog.convert_to(
            'trpg-canonical-projection-hash-v3', 'UTF8'))
        || public.projection_hash_field(2, pg_catalog.convert_to(previous_hash, 'UTF8'))
        || public.projection_hash_field(3, pg_catalog.int8send(stored_event.sequence))
        || public.projection_hash_field(4, pg_catalog.int8send(stored_event.stream_version))
        || public.projection_hash_field(5, pg_catalog.convert_to(stored_event.stream_id, 'UTF8'))
        || public.projection_hash_field(6, pg_catalog.convert_to(stored_event.event_type, 'UTF8'))
        || public.projection_hash_field(7, pg_catalog.int4send(stored_event.event_schema_version))
        || public.projection_hash_field(8, pg_catalog.convert_to(stored_event.campaign_id, 'UTF8'))
        || public.projection_hash_field(9, pg_catalog.int8send(stored_event.expected_version))
        || public.projection_hash_field(10, pg_catalog.convert_to(stored_event.authority_mode, 'UTF8'))
        || public.projection_hash_field(11, pg_catalog.convert_to(stored_event.authenticated_actor_id, 'UTF8'))
        || public.projection_hash_field(36, pg_catalog.convert_to(stored_event.authenticated_actor_role, 'UTF8'))
        || public.projection_hash_field(37, pg_catalog.convert_to(
            public.canonical_projection_json(stored_event.authenticated_actor_origin), 'UTF8'))
        || public.projection_hash_field(12, pg_catalog.convert_to(stored_event.resource_type, 'UTF8'))
        || public.projection_hash_field(13, pg_catalog.convert_to(stored_event.resource_id, 'UTF8'))
        || public.projection_hash_field(14, pg_catalog.convert_to(stored_event.authority_contract_id, 'UTF8'))
        || public.projection_hash_field(15, pg_catalog.convert_to(stored_event.authority_owner, 'UTF8'))
        || public.projection_hash_field(16, pg_catalog.convert_to(stored_event.command_id, 'UTF8'))
        || public.projection_hash_field(17, pg_catalog.convert_to(stored_event.idempotency_key, 'UTF8'))
        || public.projection_hash_field(18, pg_catalog.convert_to(stored_event.idempotency_operation, 'UTF8'))
        || public.projection_hash_field(19, pg_catalog.int8send(stored_event.authority_contract_version))
        || public.projection_hash_field(20, pg_catalog.convert_to(stored_event.visibility_label, 'UTF8'))
        || public.projection_hash_field(21, pg_catalog.convert_to(stored_event.visibility_subject, 'UTF8'))
        || public.projection_hash_field(22, pg_catalog.convert_to(stored_event.fact_provenance_kind, 'UTF8'))
        || public.projection_hash_field(23, pg_catalog.convert_to(stored_event.fact_provenance_reference, 'UTF8'))
        || public.projection_hash_field(24, pg_catalog.convert_to(stored_event.fact_recorded_by, 'UTF8'))
        || public.projection_hash_field(25, pg_catalog.convert_to(stored_event.correlation_id, 'UTF8'))
        || public.projection_hash_field(26, pg_catalog.convert_to(stored_event.causation_id, 'UTF8'))
        || public.projection_hash_field(27, pg_catalog.convert_to(stored_event.trace_id, 'UTF8'))
        || public.projection_hash_field(29, pg_catalog.int8send(recorded_at_micros))
        || public.projection_hash_field(30, pg_catalog.convert_to(
            COALESCE(stored_event.event_integrity_hash, ''), 'UTF8'))
        || public.projection_hash_field(31, pg_catalog.convert_to(stored_event.request_hash, 'UTF8'))
        || public.projection_hash_field(32, pg_catalog.convert_to(stored_event.request_hash_source, 'UTF8'))
        || public.projection_hash_field(33, pg_catalog.convert_to(stored_event.integrity_status, 'UTF8'))
        || public.projection_hash_field(34, pg_catalog.convert_to(stored_event.payload_integrity_source, 'UTF8'))
        || public.projection_hash_field(35, pg_catalog.convert_to(
            public.canonical_projection_json(stored_event.payload_json), 'UTF8'));

    RETURN 'sha256:' || pg_catalog.encode(pg_catalog.sha256(hash_input), 'hex');
END;
$$;

-- The worker may delete this downstream read model for a complete rebuild,
-- but no caller may mutate or forge a materialized canonical event in place.
-- This turns document corruption into a rejected write, while missing rows are
-- detected against the durable checkpoint and atomically replayed from Event
-- Store by PostgresProjectionWorker.
CREATE OR REPLACE FUNCTION enforce_canonical_event_projection_document()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
    previous_projection_hash TEXT;
    expected_projection_hash TEXT;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'canonical event projection rows are immutable';
    END IF;

    SELECT * INTO stored_event
      FROM public.event_store
     WHERE campaign_id = NEW.campaign_id
       AND stream_id = NEW.stream_id
       AND stream_version = NEW.stream_version
       AND sequence = NEW.event_sequence;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'canonical projection event does not exist';
    END IF;

    IF jsonb_typeof(NEW.event_document) IS DISTINCT FROM 'object'
       OR (SELECT count(*) FROM pg_catalog.jsonb_object_keys(NEW.event_document)) <> 34
       OR NOT NEW.event_document ?& ARRAY[
           'sequence', 'stream_version', 'stream_id', 'event_type',
           'event_schema_version', 'campaign_id', 'expected_version',
           'authority_mode', 'authenticated_actor_id',
           'authenticated_actor_role', 'authenticated_actor_origin', 'resource_type',
           'resource_id', 'authority_contract_id', 'authority_owner',
           'command_id', 'idempotency_key', 'idempotency_operation',
           'authority_contract_version', 'visibility_label',
           'visibility_subject', 'provenance_kind', 'provenance_reference',
           'provenance_recorded_by', 'correlation_id', 'causation_id',
           'trace_id', 'payload', 'recorded_at', 'event_integrity_hash',
           'request_hash', 'request_hash_source', 'integrity_status',
           'payload_integrity_source'
       ]
       OR (NEW.event_document->>'sequence')::BIGINT IS DISTINCT FROM stored_event.sequence
       OR (NEW.event_document->>'stream_version')::BIGINT IS DISTINCT FROM stored_event.stream_version
       OR NEW.event_document->>'stream_id' IS DISTINCT FROM stored_event.stream_id
       OR NEW.event_document->>'event_type' IS DISTINCT FROM stored_event.event_type
       OR (NEW.event_document->>'event_schema_version')::INTEGER IS DISTINCT FROM stored_event.event_schema_version
       OR NEW.event_document->>'campaign_id' IS DISTINCT FROM stored_event.campaign_id
       OR (NEW.event_document->>'expected_version')::BIGINT IS DISTINCT FROM stored_event.expected_version
       OR NEW.event_document->>'authority_mode' IS DISTINCT FROM stored_event.authority_mode
       OR NEW.event_document->>'authenticated_actor_id' IS DISTINCT FROM stored_event.authenticated_actor_id
       OR NEW.event_document->>'authenticated_actor_role' IS DISTINCT FROM stored_event.authenticated_actor_role
       OR NEW.event_document->'authenticated_actor_origin' IS DISTINCT FROM stored_event.authenticated_actor_origin
       OR NEW.event_document->>'resource_type' IS DISTINCT FROM stored_event.resource_type
       OR NEW.event_document->>'resource_id' IS DISTINCT FROM stored_event.resource_id
       OR NEW.event_document->>'authority_contract_id' IS DISTINCT FROM stored_event.authority_contract_id
       OR NEW.event_document->>'authority_owner' IS DISTINCT FROM stored_event.authority_owner
       OR NEW.event_document->>'command_id' IS DISTINCT FROM stored_event.command_id
       OR NEW.event_document->>'idempotency_key' IS DISTINCT FROM stored_event.idempotency_key
       OR NEW.event_document->>'idempotency_operation' IS DISTINCT FROM stored_event.idempotency_operation
       OR (NEW.event_document->>'authority_contract_version')::BIGINT IS DISTINCT FROM stored_event.authority_contract_version
       OR NEW.event_document->>'visibility_label' IS DISTINCT FROM stored_event.visibility_label
       OR NEW.event_document->>'visibility_subject' IS DISTINCT FROM stored_event.visibility_subject
       OR NEW.event_document->>'provenance_kind' IS DISTINCT FROM stored_event.fact_provenance_kind
       OR NEW.event_document->>'provenance_reference' IS DISTINCT FROM stored_event.fact_provenance_reference
       OR NEW.event_document->>'provenance_recorded_by' IS DISTINCT FROM stored_event.fact_recorded_by
       OR NEW.event_document->>'correlation_id' IS DISTINCT FROM stored_event.correlation_id
       OR NEW.event_document->>'causation_id' IS DISTINCT FROM stored_event.causation_id
       OR NEW.event_document->>'trace_id' IS DISTINCT FROM stored_event.trace_id
       OR NEW.event_document->'payload' IS DISTINCT FROM stored_event.payload_json
       OR (NEW.event_document->>'recorded_at')::TIMESTAMPTZ IS DISTINCT FROM stored_event.recorded_at
       OR NEW.event_document->>'event_integrity_hash' IS DISTINCT FROM stored_event.event_integrity_hash
       OR NEW.event_document->>'request_hash' IS DISTINCT FROM stored_event.request_hash
       OR NEW.event_document->>'request_hash_source' IS DISTINCT FROM stored_event.request_hash_source
       OR NEW.event_document->>'integrity_status' IS DISTINCT FROM stored_event.integrity_status
       OR NEW.event_document->>'payload_integrity_source' IS DISTINCT FROM stored_event.payload_integrity_source THEN
        RAISE EXCEPTION 'canonical projection document does not match Event Store';
    END IF;

    IF NEW.stream_version = 1 THEN
        previous_projection_hash :=
            'sha256:0000000000000000000000000000000000000000000000000000000000000000';
    ELSE
        SELECT projection_hash INTO previous_projection_hash
          FROM public.canonical_event_projection
         WHERE projection_name = NEW.projection_name
           AND campaign_id = NEW.campaign_id
           AND stream_id = NEW.stream_id
           AND stream_version = NEW.stream_version - 1;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'canonical projection hash predecessor is missing';
        END IF;
    END IF;

    expected_projection_hash := public.compute_canonical_projection_hash_v3(
        previous_projection_hash, stored_event
    );
    IF NEW.projection_hash IS DISTINCT FROM expected_projection_hash THEN
        RAISE EXCEPTION 'canonical projection hash does not match Event Store chain';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER canonical_event_projection_document_guard
BEFORE INSERT OR UPDATE ON public.canonical_event_projection
FOR EACH ROW EXECUTE FUNCTION enforce_canonical_event_projection_document();

CREATE OR REPLACE FUNCTION enforce_projection_checkpoint_monotonicity()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    materialized_event_sequence BIGINT;
    materialized_projection_hash TEXT;
BEGIN
    IF TG_OP = 'UPDATE' AND ROW(
        NEW.projection_name, NEW.campaign_id, NEW.stream_id
    ) IS DISTINCT FROM ROW(
        OLD.projection_name, OLD.campaign_id, OLD.stream_id
    ) THEN
        RAISE EXCEPTION 'projection checkpoint identity is immutable';
    END IF;

    IF TG_OP = 'UPDATE'
       AND (
           NEW.version < OLD.version
           OR NEW.last_event_sequence < OLD.last_event_sequence
       ) THEN
        RAISE EXCEPTION 'projection checkpoint cannot move backwards';
    END IF;

    IF TG_OP = 'UPDATE'
       AND NEW.version = OLD.version
       AND NEW.last_event_sequence = OLD.last_event_sequence
       AND NEW.projection_hash IS DISTINCT FROM OLD.projection_hash THEN
        RAISE EXCEPTION 'projection checkpoint hash conflicts at existing cursor';
    END IF;

    IF NEW.version = 0 AND NEW.last_event_sequence = 0 THEN
        IF NEW.projection_hash IS DISTINCT FROM
            'sha256:0000000000000000000000000000000000000000000000000000000000000000'
           OR EXISTS (
               SELECT 1
                 FROM public.canonical_event_projection
                WHERE projection_name = NEW.projection_name
                  AND campaign_id = NEW.campaign_id
                  AND stream_id = NEW.stream_id
           ) THEN
            RAISE EXCEPTION 'genesis checkpoint conflicts with materialized projection';
        END IF;
        RETURN NEW;
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM public.event_store
         WHERE campaign_id = NEW.campaign_id
           AND stream_id = NEW.stream_id
           AND stream_version = NEW.version
           AND sequence = NEW.last_event_sequence
    ) THEN
        RAISE EXCEPTION 'projection checkpoint does not reference its canonical stream event';
    END IF;

    SELECT event_sequence, projection_hash
      INTO materialized_event_sequence, materialized_projection_hash
      FROM public.canonical_event_projection
     WHERE projection_name = NEW.projection_name
       AND campaign_id = NEW.campaign_id
       AND stream_id = NEW.stream_id
       AND stream_version = NEW.version;
    IF NOT FOUND
       OR materialized_event_sequence IS DISTINCT FROM NEW.last_event_sequence
       OR materialized_projection_hash IS DISTINCT FROM NEW.projection_hash THEN
        RAISE EXCEPTION 'projection checkpoint does not match materialized Event Store chain';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER projection_checkpoint_monotonic_guard
BEFORE INSERT OR UPDATE ON public.projection_checkpoint
FOR EACH ROW EXECUTE FUNCTION enforce_projection_checkpoint_monotonicity();

-- P04 also owns the outbox referential-integrity acceptance boundary. Replace
-- the previously published function in-place so a session-local lookalike
-- table cannot influence its canonical lookup.
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
        NEW.idempotency_key, NEW.visibility_label, NEW.correlation_id,
        NEW.causation_id, NEW.payload_json, NEW.commit_id, NEW.campaign_id,
        NEW.stream_id, NEW.event_schema_version, NEW.idempotency_operation,
        NEW.request_hash, NEW.request_hash_source, NEW.integrity_status
    ) IS DISTINCT FROM ROW(
        OLD.outbox_id, OLD.event_id, OLD.event_sequence, OLD.nats_subject,
        OLD.idempotency_key, OLD.visibility_label, OLD.correlation_id,
        OLD.causation_id, OLD.payload_json, OLD.commit_id, OLD.campaign_id,
        OLD.stream_id, OLD.event_schema_version, OLD.idempotency_operation,
        OLD.request_hash, OLD.request_hash_source, OLD.integrity_status
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
       OR NEW.correlation_id IS DISTINCT FROM stored_event.correlation_id
       OR NEW.causation_id IS DISTINCT FROM stored_event.causation_id
       OR NEW.payload_json IS DISTINCT FROM stored_event.payload_json
       OR NEW.request_hash IS DISTINCT FROM stored_event.request_hash
       OR NEW.request_hash_source IS DISTINCT FROM stored_event.request_hash_source
       OR NEW.integrity_status IS DISTINCT FROM stored_event.integrity_status THEN
        RAISE EXCEPTION 'outbox metadata does not match canonical event';
    END IF;
    RETURN NEW;
END;
$$;
