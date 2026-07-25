ALTER TABLE event_store
    ADD COLUMN IF NOT EXISTS data_subject_id TEXT NOT NULL DEFAULT 'not_applicable';
ALTER TABLE event_outbox
    ADD COLUMN IF NOT EXISTS data_subject_id TEXT NOT NULL DEFAULT 'not_applicable';

CREATE INDEX IF NOT EXISTS event_store_data_subject_idx
    ON event_store(data_subject_id, sequence)
    WHERE data_subject_id <> 'not_applicable';
CREATE INDEX IF NOT EXISTS event_outbox_data_subject_idx
    ON event_outbox(data_subject_id, event_sequence)
    WHERE data_subject_id <> 'not_applicable';

-- Existing private rows remain audit-only until an application-controlled
-- re-encryption migration can bind them to a subject key. Assigning the new
-- column from metadata alone would falsely claim crypto-erasure support.
CREATE OR REPLACE FUNCTION enforce_subject_scoped_event_protection()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    subject_key public.privacy_subject_keys%ROWTYPE;
    fence_status TEXT;
BEGIN
    IF NEW.data_subject_id <> 'not_applicable' THEN
        SELECT * INTO subject_key
          FROM public.privacy_subject_keys
         WHERE subject_id = NEW.data_subject_id;
        IF NOT FOUND OR subject_key.wrapped_key IS NULL OR subject_key.destroyed_at IS NOT NULL
           OR NEW.payload_key_reference IS DISTINCT FROM subject_key.key_reference THEN
            RAISE EXCEPTION 'private event payload is not bound to an active subject key';
        END IF;
        SELECT status INTO fence_status
          FROM public.privacy_subject_deletion_fences
         WHERE subject_id = NEW.data_subject_id;
        IF FOUND AND fence_status IN ('running', 'completed') THEN
            RAISE EXCEPTION 'data subject is fenced for deletion';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS event_store_subject_protection_guard ON event_store;
CREATE TRIGGER event_store_subject_protection_guard
BEFORE INSERT OR UPDATE ON event_store
FOR EACH ROW EXECUTE FUNCTION enforce_subject_scoped_event_protection();

CREATE OR REPLACE FUNCTION enforce_subject_scoped_outbox_protection()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
DECLARE
    stored_event public.event_store%ROWTYPE;
BEGIN
    SELECT * INTO stored_event
      FROM public.event_store
     WHERE sequence = NEW.event_sequence;
    IF NOT FOUND
       OR NEW.data_subject_id IS DISTINCT FROM stored_event.data_subject_id
       OR NEW.payload_key_reference IS DISTINCT FROM stored_event.payload_key_reference THEN
        RAISE EXCEPTION 'outbox data subject does not match canonical event';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS event_outbox_subject_protection_guard ON event_outbox;
CREATE TRIGGER event_outbox_subject_protection_guard
BEFORE INSERT OR UPDATE ON event_outbox
FOR EACH ROW EXECUTE FUNCTION enforce_subject_scoped_outbox_protection();

CREATE OR REPLACE FUNCTION prevent_destroyed_subject_key_restoration()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
    IF OLD.destroyed_at IS NOT NULL
       AND (NEW.wrapped_key IS NOT NULL OR NEW.destroyed_at IS NULL) THEN
        RAISE EXCEPTION 'destroyed subject key cannot be restored';
    END IF;
    IF NEW.subject_id IS DISTINCT FROM OLD.subject_id
       OR NEW.key_reference IS DISTINCT FROM OLD.key_reference THEN
        RAISE EXCEPTION 'subject key identity is immutable';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS privacy_subject_key_destruction_guard ON privacy_subject_keys;
CREATE TRIGGER privacy_subject_key_destruction_guard
BEFORE UPDATE ON privacy_subject_keys
FOR EACH ROW EXECUTE FUNCTION prevent_destroyed_subject_key_restoration();

-- Legacy rows that claimed a visibility audience as a data owner were never
-- independently classified. They remain durable, explicit dead letters until
-- a trusted migration assigns an actual data subject (or not_applicable).
UPDATE event_outbox
   SET delivery_status = 'dead_lettered',
       dead_lettered_at = COALESCE(dead_lettered_at, now()),
       available_at = now(),
       last_error = 'OUTBOX_PRIVATE_PAYLOAD_NOT_SUBJECT_SCOPED',
       claimed_at = NULL,
       claim_owner = NULL,
       claim_token = NULL,
       locked_until = NULL
 WHERE data_subject_id = 'not_applicable'
   AND visibility_subject <> 'not_applicable'
   AND published_at IS NULL
   AND dead_lettered_at IS NULL;
