-- P05 repair: copyright/use policy and embedding bytes are derived data, but
-- they still require formal, integrity-bound derivation evidence. Existing
-- RAG rows are rebuildable and are removed instead of being assigned invented
-- attestations.

ALTER TABLE public.event_store
    ADD COLUMN IF NOT EXISTS derived_source_type TEXT,
    ADD COLUMN IF NOT EXISTS derived_copyright_status TEXT,
    ADD COLUMN IF NOT EXISTS derived_allowed_use TEXT,
    ADD COLUMN IF NOT EXISTS derived_embedding_model TEXT,
    ADD COLUMN IF NOT EXISTS derived_embedding_dimensions INTEGER,
    ADD COLUMN IF NOT EXISTS derived_embedding_hash TEXT;

ALTER TABLE public.event_store
    DROP CONSTRAINT IF EXISTS event_store_rag_derivation_fields_check,
    ADD CONSTRAINT event_store_rag_derivation_fields_check CHECK (
        (
            event_type = 'RagChunkDerived'
            AND derived_source_event_sequence IS NOT NULL
            AND derived_source_event_sequence > 0
            AND derived_snapshot_id IS NOT NULL
            AND btrim(derived_snapshot_id) <> ''
            AND derived_chunk_id IS NOT NULL
            AND btrim(derived_chunk_id) <> ''
            AND derived_content_hash IS NOT NULL
            AND derived_content_hash ~ '^[0-9a-f]{64}$'
            AND derived_source_type IS NOT NULL
            AND derived_source_type ~ '^[a-z0-9_]+$'
            AND derived_copyright_status IS NOT NULL
            AND derived_copyright_status ~ '^[a-z0-9_]+$'
            AND derived_allowed_use IS NOT NULL
            AND derived_allowed_use ~ '^[a-z0-9_]+$'
            AND derived_embedding_model IS NOT NULL
            AND btrim(derived_embedding_model) <> ''
            AND derived_embedding_dimensions BETWEEN 1 AND 4096
            AND derived_embedding_hash IS NOT NULL
            AND derived_embedding_hash ~ '^[0-9a-f]{64}$'
        )
        OR
        (
            event_type <> 'RagChunkDerived'
            AND derived_source_event_sequence IS NULL
            AND derived_snapshot_id IS NULL
            AND derived_chunk_id IS NULL
            AND derived_content_hash IS NULL
            AND derived_source_type IS NULL
            AND derived_copyright_status IS NULL
            AND derived_allowed_use IS NULL
            AND derived_embedding_model IS NULL
            AND derived_embedding_dimensions IS NULL
            AND derived_embedding_hash IS NULL
        )
    ) NOT VALID;

DELETE FROM public.rag_snapshot_chunk;

CREATE OR REPLACE FUNCTION public.enforce_rag_snapshot_chunk_source()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
    derivation_event public.event_store%ROWTYPE;
    actual_embedding_hash TEXT;
BEGIN
    PERFORM public.lock_rag_snapshot(NEW.campaign_id, NEW.snapshot_id);

    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'RAG snapshot chunks are immutable; rebuild the snapshot';
    END IF;

    SELECT * INTO stored_event
      FROM public.event_store
     WHERE sequence = NEW.source_event_sequence;
    SELECT * INTO derivation_event
      FROM public.event_store
     WHERE sequence = NEW.derivation_event_sequence;
    IF NOT FOUND OR stored_event.sequence IS NULL THEN
        RAISE EXCEPTION 'RAG source or derivation event does not exist';
    END IF;

    actual_embedding_hash := encode(
        sha256(substring(vector_send(NEW.embedding) FROM 5)),
        'hex'
    );
    IF derivation_event.event_type <> 'RagChunkDerived'
       OR derivation_event.integrity_status <> 'verified_hmac'
       OR derivation_event.request_hash_source <> 'formal_commit'
       OR derivation_event.event_integrity_hash IS NULL
       OR NOT (derivation_event.payload_json ? 'protected_payload')
       OR derivation_event.derived_source_event_sequence
          IS DISTINCT FROM stored_event.sequence
       OR derivation_event.derived_snapshot_id IS DISTINCT FROM NEW.snapshot_id
       OR derivation_event.derived_chunk_id IS DISTINCT FROM NEW.chunk_id
       OR derivation_event.derived_content_hash IS DISTINCT FROM NEW.chunk_hash
       OR derivation_event.derived_source_type IS DISTINCT FROM NEW.source_type
       OR derivation_event.derived_copyright_status
          IS DISTINCT FROM NEW.copyright_status
       OR derivation_event.derived_allowed_use IS DISTINCT FROM NEW.allowed_use
       OR derivation_event.derived_embedding_model
          IS DISTINCT FROM NEW.embedding_model
       OR derivation_event.derived_embedding_dimensions
          IS DISTINCT FROM NEW.embedding_dimensions
       OR derivation_event.derived_embedding_hash
          IS DISTINCT FROM actual_embedding_hash
       OR derivation_event.campaign_id IS DISTINCT FROM stored_event.campaign_id
       OR derivation_event.visibility_label
          IS DISTINCT FROM stored_event.visibility_label
       OR derivation_event.visibility_subject
          IS DISTINCT FROM stored_event.visibility_subject
       OR derivation_event.fact_provenance_kind
          IS DISTINCT FROM stored_event.fact_provenance_kind
       OR derivation_event.fact_provenance_reference
          IS DISTINCT FROM stored_event.fact_provenance_reference
       OR derivation_event.fact_recorded_by
          IS DISTINCT FROM stored_event.fact_recorded_by THEN
        RAISE EXCEPTION
            'RAG content, policy, or embedding lacks formal derivation evidence';
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
               OR existing.embedding_dimensions
                  IS DISTINCT FROM NEW.embedding_dimensions
           )
    ) THEN
        RAISE EXCEPTION 'RAG snapshot embedding contract is inconsistent';
    END IF;
    RETURN NEW;
END;
$$;
