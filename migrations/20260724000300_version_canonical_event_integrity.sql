-- P05 repair: the original event HMAC covered only request hash, batch index,
-- event type, and protected payload. It did not authenticate the persisted
-- authorization, visibility, provenance, routing, actor, or derivation
-- columns. Version 2 is computed by the canonical service over the complete
-- persisted security record.
--
-- A SQL migration does not possess the application HMAC key. Existing
-- version-1 signatures therefore cannot be honestly promoted to version 2.
-- Quarantine them as historical/unverified so they remain auditable but are
-- ineligible for canonical replay, projection, or outbox publication.

ALTER TABLE event_store
    ADD COLUMN event_integrity_version INTEGER;

DROP TRIGGER event_store_append_only ON event_store;

UPDATE event_store
   SET event_integrity_version = CASE
           WHEN event_integrity_hash IS NULL THEN 0
           ELSE 1
       END,
       integrity_status = CASE
           WHEN event_integrity_hash IS NULL THEN 'historical_unsigned'
           ELSE 'historical_unverified_hmac'
       END;

CREATE TRIGGER event_store_append_only
BEFORE UPDATE OR DELETE ON event_store
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

ALTER TABLE event_outbox DISABLE TRIGGER event_outbox_event_binding;
ALTER TABLE event_outbox DISABLE TRIGGER event_outbox_delivery_transition_guard;

UPDATE event_outbox AS outbox
   SET integrity_status = event.integrity_status
  FROM event_store AS event
 WHERE event.sequence = outbox.event_sequence;

ALTER TABLE event_outbox ENABLE TRIGGER event_outbox_event_binding;
ALTER TABLE event_outbox ENABLE TRIGGER event_outbox_delivery_transition_guard;

ALTER TABLE event_store
    ALTER COLUMN event_integrity_version SET NOT NULL,
    ALTER COLUMN event_integrity_version SET DEFAULT 2,
    DROP CONSTRAINT event_store_integrity_status_valid,
    ADD CONSTRAINT event_store_integrity_status_valid CHECK (
        integrity_status = 'verified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_version = 2
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unverified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_version = 1
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unsigned'
        AND request_hash_source = 'historical_unavailable'
        AND event_integrity_version = 0
        AND event_integrity_hash IS NULL
    );

COMMENT ON COLUMN event_store.event_integrity_version IS
    '1=legacy payload-only HMAC (quarantined), 2=complete persisted security-record HMAC';
