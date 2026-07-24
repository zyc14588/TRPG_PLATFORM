-- P05: unverified historical events remain auditable but must never cross the
-- shared canonical NATS subject. This is forward-only: no historical Event
-- Store or outbox row is deleted or assigned fabricated integrity metadata.

UPDATE event_outbox
   SET delivery_status = 'dead_lettered',
       dead_lettered_at = COALESCE(dead_lettered_at, now()),
       available_at = now(),
       last_error = 'UNVERIFIED_HISTORY_QUARANTINED',
       claimed_at = NULL,
       claim_owner = NULL,
       claim_token = NULL,
       locked_until = NULL
 WHERE published_at IS NULL
   AND dead_lettered_at IS NULL
   AND (
        integrity_status <> 'verified_hmac'
        OR request_hash_source <> 'formal_commit'
        OR commit_id IS NULL
   );

CREATE OR REPLACE FUNCTION prevent_unverified_outbox_publish()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NEW.delivery_status = 'published' OR NEW.published_at IS NOT NULL THEN
        IF NEW.integrity_status <> 'verified_hmac'
           OR NEW.request_hash_source <> 'formal_commit'
           OR NEW.commit_id IS NULL
           OR NOT (NEW.payload_json ? 'protected_payload')
           OR NOT EXISTS (
                SELECT 1
                  FROM public.formal_commits AS formal
                  JOIN public.event_store AS event
                    ON event.sequence = NEW.event_sequence
                 WHERE formal.commit_id = NEW.commit_id
                   AND event.sequence BETWEEN formal.first_event_sequence
                                          AND formal.last_event_sequence
                   AND event.campaign_id = formal.campaign_id
                   AND event.stream_id = formal.stream_id
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.request_hash = formal.request_hash
                   AND event.event_integrity_hash IS NOT NULL
                   AND event.payload_json = NEW.payload_json
                   AND event.payload_json ? 'protected_payload'
           )
        THEN
            RAISE EXCEPTION 'unverified outbox delivery cannot be published';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS prevent_unverified_outbox_publish_trigger ON event_outbox;
CREATE TRIGGER prevent_unverified_outbox_publish_trigger
BEFORE INSERT OR UPDATE
ON event_outbox
FOR EACH ROW
EXECUTE FUNCTION prevent_unverified_outbox_publish();

COMMENT ON FUNCTION prevent_unverified_outbox_publish() IS
    'P05 fail-closed guard: only encrypted, formal-commit, verified-HMAC events may be marked published';

CREATE OR REPLACE FUNCTION prevent_unverified_rag_source()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM public.event_store AS event
         WHERE event.sequence = NEW.source_event_sequence
           AND event.campaign_id = NEW.campaign_id
           AND event.integrity_status = 'verified_hmac'
           AND event.request_hash_source = 'formal_commit'
           AND event.event_integrity_hash IS NOT NULL
           AND event.payload_json ? 'protected_payload'
    ) THEN
        RAISE EXCEPTION 'unverified Event Store row cannot source a RAG chunk';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS p05_prevent_unverified_rag_source ON rag_snapshot_chunk;
CREATE TRIGGER p05_prevent_unverified_rag_source
BEFORE INSERT OR UPDATE
ON rag_snapshot_chunk
FOR EACH ROW
EXECUTE FUNCTION prevent_unverified_rag_source();
