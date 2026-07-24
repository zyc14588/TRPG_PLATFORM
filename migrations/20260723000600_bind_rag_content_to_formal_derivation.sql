ALTER TABLE event_store
    ADD COLUMN IF NOT EXISTS derived_source_event_sequence BIGINT,
    ADD COLUMN IF NOT EXISTS derived_snapshot_id TEXT,
    ADD COLUMN IF NOT EXISTS derived_chunk_id TEXT,
    ADD COLUMN IF NOT EXISTS derived_content_hash TEXT;

ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_rag_derivation_fields_check,
    ADD CONSTRAINT event_store_rag_derivation_fields_check CHECK (
        (event_type = 'RagChunkDerived'
         AND derived_source_event_sequence > 0
         AND btrim(derived_snapshot_id) <> ''
         AND btrim(derived_chunk_id) <> ''
         AND derived_content_hash ~ '^[0-9a-f]{64}$')
        OR
        (event_type <> 'RagChunkDerived'
         AND derived_source_event_sequence IS NULL
         AND derived_snapshot_id IS NULL
         AND derived_chunk_id IS NULL
         AND derived_content_hash IS NULL)
    ) NOT VALID;

-- RAG is explicitly rebuildable. Rows predating formal derivation evidence
-- are removed instead of being relabelled with invented provenance.
DELETE FROM rag_snapshot_chunk;
ALTER TABLE rag_snapshot_chunk
    ADD COLUMN IF NOT EXISTS derivation_event_sequence BIGINT;
ALTER TABLE rag_snapshot_chunk
    ALTER COLUMN derivation_event_sequence SET NOT NULL;
ALTER TABLE rag_snapshot_chunk
    DROP CONSTRAINT IF EXISTS rag_snapshot_chunk_derivation_event_fkey,
    ADD CONSTRAINT rag_snapshot_chunk_derivation_event_fkey
        FOREIGN KEY (derivation_event_sequence) REFERENCES event_store(sequence);
CREATE UNIQUE INDEX IF NOT EXISTS rag_snapshot_chunk_derivation_event_uq
    ON rag_snapshot_chunk(derivation_event_sequence);

CREATE OR REPLACE FUNCTION public.enforce_rag_snapshot_chunk_source()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
    derivation_event public.event_store%ROWTYPE;
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
    IF derivation_event.event_type <> 'RagChunkDerived'
       OR derivation_event.integrity_status <> 'verified_hmac'
       OR derivation_event.request_hash_source <> 'formal_commit'
       OR derivation_event.event_integrity_hash IS NULL
       OR NOT (derivation_event.payload_json ? 'protected_payload')
       OR derivation_event.derived_source_event_sequence IS DISTINCT FROM stored_event.sequence
       OR derivation_event.derived_snapshot_id IS DISTINCT FROM NEW.snapshot_id
       OR derivation_event.derived_chunk_id IS DISTINCT FROM NEW.chunk_id
       OR derivation_event.derived_content_hash IS DISTINCT FROM NEW.chunk_hash
       OR derivation_event.campaign_id IS DISTINCT FROM stored_event.campaign_id
       OR derivation_event.visibility_label IS DISTINCT FROM stored_event.visibility_label
       OR derivation_event.visibility_subject IS DISTINCT FROM stored_event.visibility_subject
       OR derivation_event.fact_provenance_kind IS DISTINCT FROM stored_event.fact_provenance_kind
       OR derivation_event.fact_provenance_reference IS DISTINCT FROM stored_event.fact_provenance_reference
       OR derivation_event.fact_recorded_by IS DISTINCT FROM stored_event.fact_recorded_by THEN
        RAISE EXCEPTION 'RAG content is not bound to formal derivation evidence';
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
        SELECT 1 FROM public.rag_snapshot_chunk AS existing
         WHERE existing.campaign_id = NEW.campaign_id
           AND existing.snapshot_id = NEW.snapshot_id
           AND (existing.embedding_model IS DISTINCT FROM NEW.embedding_model
                OR existing.embedding_dimensions IS DISTINCT FROM NEW.embedding_dimensions)
    ) THEN
        RAISE EXCEPTION 'RAG snapshot embedding contract is inconsistent';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS rag_snapshot_chunk_source_guard ON rag_snapshot_chunk;
CREATE TRIGGER rag_snapshot_chunk_source_guard
BEFORE INSERT OR UPDATE ON rag_snapshot_chunk
FOR EACH ROW EXECUTE FUNCTION enforce_rag_snapshot_chunk_source();
