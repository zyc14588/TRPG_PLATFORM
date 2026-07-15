DROP TABLE IF EXISTS workflow_transitions;
DROP TABLE IF EXISTS workflow_instances;
DROP TABLE IF EXISTS formal_commits;
DROP TABLE IF EXISTS canonical_audit_log;
DROP TRIGGER IF EXISTS event_store_append_only ON event_store;
DROP TRIGGER IF EXISTS event_store_no_truncate ON event_store;
DROP FUNCTION IF EXISTS enforce_canonical_audit_chain();
DROP FUNCTION IF EXISTS reject_canonical_append_mutation();
DROP INDEX IF EXISTS event_store_campaign_stream_version_idx;
DROP INDEX IF EXISTS event_store_campaign_sequence_idx;
DROP INDEX IF EXISTS event_outbox_commit_idx;
ALTER TABLE event_store
    DROP CONSTRAINT IF EXISTS event_store_integrity_hash_format;
ALTER TABLE event_outbox
    DROP COLUMN IF EXISTS dead_lettered_at,
    DROP COLUMN IF EXISTS last_error,
    DROP COLUMN IF EXISTS claim_owner,
    DROP COLUMN IF EXISTS claimed_at,
    DROP COLUMN IF EXISTS commit_id;
ALTER TABLE event_store
    DROP COLUMN IF EXISTS event_integrity_hash,
    DROP COLUMN IF EXISTS trace_id,
    DROP COLUMN IF EXISTS visibility_subject,
    DROP COLUMN IF EXISTS authority_owner,
    DROP COLUMN IF EXISTS authority_contract_id,
    DROP COLUMN IF EXISTS resource_id,
    DROP COLUMN IF EXISTS resource_type,
    DROP COLUMN IF EXISTS authenticated_actor_id,
    DROP COLUMN IF EXISTS stream_version,
    DROP COLUMN IF EXISTS campaign_id;
