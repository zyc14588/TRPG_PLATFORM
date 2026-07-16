-- Forward-only repair for the published event persistence schema.
--
-- Every preceding migration remains byte-for-byte frozen. This migration
-- accepts only the two observed pre-P03 shapes, validates their constraints
-- and mutation guards, and converges both shapes without rewriting the SQLx
-- ledger or pretending that unsigned historical rows have a request/HMAC.

CREATE EXTENSION IF NOT EXISTS vector;

DO $$
DECLARE
    actual_columns TEXT[];
BEGIN
    -- Catalog deparsing below must not depend on the migration role's
    -- inherited search_path.
    PERFORM set_config('search_path', 'public, pg_catalog', true);

    IF to_regclass('public.event_store') IS NULL
       OR to_regclass('public.event_outbox') IS NULL
       OR to_regclass('public.projection_checkpoint') IS NULL
       OR to_regclass('public.canonical_audit_log') IS NULL
       OR to_regclass('public.formal_commits') IS NULL THEN
        RAISE EXCEPTION 'unsupported event persistence schema: required table missing';
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'event_store';
    IF actual_columns <> ARRAY[
        'sequence:int8:NO:nextval(''event_store_sequence_seq''::regclass)',
        'event_type:text:NO:-', 'command_id:text:NO:-',
        'idempotency_key:text:NO:-', 'expected_version:int8:NO:-',
        'authority_mode:text:NO:-', 'authority_contract_version:int8:NO:-',
        'visibility_label:text:NO:-', 'fact_provenance_kind:text:NO:-',
        'fact_provenance_reference:text:NO:-', 'fact_recorded_by:text:NO:-',
        'correlation_id:text:NO:-', 'causation_id:text:NO:-',
        'payload_json:text:NO:-', 'recorded_at:timestamptz:NO:now()',
        'campaign_id:text:NO:''historical_unscoped''::text',
        'stream_version:int8:NO:-',
        'authenticated_actor_id:text:NO:''historical_unknown''::text',
        'resource_type:text:NO:''historical_unknown''::text',
        'resource_id:text:NO:''historical_unknown''::text',
        'authority_contract_id:text:NO:''historical_unknown''::text',
        'authority_owner:text:NO:''historical_unknown''::text',
        'visibility_subject:text:NO:''not_applicable''::text',
        'trace_id:text:NO:''historical_unknown''::text',
        'event_integrity_hash:text:YES:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'unsupported event_store schema: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'event_outbox';
    IF actual_columns <> ARRAY[
        'outbox_id:int8:NO:nextval(''event_outbox_outbox_id_seq''::regclass)',
        'event_sequence:int8:NO:-', 'nats_subject:text:NO:-',
        'idempotency_key:text:NO:-', 'visibility_label:text:NO:-',
        'correlation_id:text:NO:-', 'causation_id:text:NO:-',
        'payload_json:text:NO:-', 'published_at:timestamptz:YES:-',
        'retry_count:int4:NO:0', 'commit_id:text:YES:-',
        'claimed_at:timestamptz:YES:-', 'claim_owner:text:YES:-',
        'last_error:text:YES:-', 'dead_lettered_at:timestamptz:YES:-'
    ]::TEXT[] AND actual_columns <> ARRAY[
        'outbox_id:int8:NO:nextval(''event_outbox_outbox_id_seq''::regclass)',
        'event_id:int8:NO:-', 'event_sequence:int8:NO:-',
        'nats_subject:text:NO:-', 'idempotency_key:text:NO:-',
        'visibility_label:text:NO:-', 'correlation_id:text:NO:-',
        'causation_id:text:NO:-', 'payload_json:text:NO:-',
        'published_at:timestamptz:YES:-', 'retry_count:int4:NO:0',
        'commit_id:text:YES:-', 'claimed_at:timestamptz:YES:-',
        'claim_owner:text:YES:-', 'last_error:text:YES:-',
        'dead_lettered_at:timestamptz:YES:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'unsupported event_outbox schema: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'projection_checkpoint';
    IF actual_columns <> ARRAY[
        'projection_name:text:NO:-', 'last_event_sequence:int8:NO:-',
        'projection_hash:text:NO:-', 'rebuilt_at:timestamptz:NO:now()'
    ]::TEXT[] AND actual_columns <> ARRAY[
        'projection_name:text:NO:-', 'stream_id:text:NO:-',
        'version:int8:NO:-', 'last_event_sequence:int8:NO:-',
        'projection_hash:text:NO:-', 'rebuilt_at:timestamptz:NO:now()'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'unsupported projection_checkpoint schema: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'formal_commits';
    IF actual_columns <> ARRAY[
        'commit_id:text:NO:-', 'campaign_id:text:NO:-',
        'idempotency_key:text:NO:-', 'request_hash:text:NO:-',
        'expected_version:int8:NO:-', 'first_event_sequence:int8:NO:-',
        'last_event_sequence:int8:NO:-', 'first_stream_version:int8:NO:-',
        'last_stream_version:int8:NO:-', 'audit_sequence:int8:NO:-',
        'witness_prepare_sequence:int8:NO:-', 'witness_prepare_hash:text:NO:-',
        'committed_at:timestamptz:NO:now()'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'unsupported formal_commits schema: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'canonical_audit_log';
    IF actual_columns <> ARRAY[
        'sequence:int8:NO:-', 'commit_id:text:NO:-', 'campaign_id:text:NO:-',
        'actor_id:text:NO:-', 'actor_origin:text:NO:-',
        'authentication_reference:text:NO:-', 'resource_type:text:NO:-',
        'resource_id:text:NO:-', 'action:text:NO:-', 'requested_role:text:NO:-',
        'visibility_label:text:NO:-', 'visibility_subject:text:NO:-',
        'provenance_kind:text:NO:-', 'provenance_reference:text:NO:-',
        'provenance_recorded_by:text:NO:-', 'decision:text:NO:-',
        'openfga_decision_id:text:NO:-', 'openfga_policy_revision:text:NO:-',
        'opa_decision_id:text:NO:-', 'opa_policy_revision:text:NO:-',
        'trace_id:text:NO:-', 'event_batch_hash:text:NO:-',
        'witness_prepare_sequence:int8:NO:-', 'witness_prepare_hash:text:NO:-',
        'occurred_at:timestamptz:NO:now()', 'integrity_key_id:text:NO:-',
        'previous_hash:text:NO:-', 'record_hash:text:NO:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'unsupported canonical_audit_log schema: %', actual_columns;
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('event_store'::regclass, 'event_store_pkey',
               'PRIMARY KEY (sequence)'),
              ('event_store'::regclass, 'event_store_idempotency_key_key',
               'UNIQUE (idempotency_key)'),
              ('event_store'::regclass, 'event_store_integrity_hash_format',
               'CHECK (((event_integrity_hash IS NULL) OR (event_integrity_hash ~ ''^hmac-sha256:[0-9a-f]{64}$''::text)))'),
              ('event_outbox'::regclass, 'event_outbox_pkey',
               'PRIMARY KEY (outbox_id)'),
              ('event_outbox'::regclass, 'event_outbox_event_sequence_fkey',
               'FOREIGN KEY (event_sequence) REFERENCES event_store(sequence)'),
              ('event_outbox'::regclass, 'event_outbox_idempotency_key_key',
               'UNIQUE (idempotency_key)'),
              ('projection_checkpoint'::regclass, 'projection_checkpoint_pkey',
               'PRIMARY KEY (projection_name)'),
              ('formal_commits'::regclass, 'formal_commits_pkey',
               'PRIMARY KEY (commit_id)'),
              ('formal_commits'::regclass, 'formal_commits_idempotency_key_key',
               'UNIQUE (idempotency_key)'),
              ('formal_commits'::regclass, 'formal_commits_first_event_sequence_fkey',
               'FOREIGN KEY (first_event_sequence) REFERENCES event_store(sequence)'),
              ('formal_commits'::regclass, 'formal_commits_last_event_sequence_fkey',
               'FOREIGN KEY (last_event_sequence) REFERENCES event_store(sequence)'),
              ('formal_commits'::regclass, 'formal_commits_audit_sequence_fkey',
               'FOREIGN KEY (audit_sequence) REFERENCES canonical_audit_log(sequence)'),
              ('formal_commits'::regclass, 'formal_commits_request_hash_check',
               'CHECK ((request_hash ~ ''^sha256:[0-9a-f]{64}$''::text))'),
              ('formal_commits'::regclass, 'formal_commits_witness_prepare_hash_check',
               'CHECK ((witness_prepare_hash ~ ''^hmac-sha256:[0-9a-f]{64}$''::text))'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_pkey',
               'PRIMARY KEY (sequence)'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_commit_id_key',
               'UNIQUE (commit_id)'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_record_hash_key',
               'UNIQUE (record_hash)'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_decision_check',
               'CHECK ((decision = ''PERMIT''::text))'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_event_batch_hash_check',
               'CHECK ((event_batch_hash ~ ''^sha256:[0-9a-f]{64}$''::text))'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_witness_prepare_hash_check',
               'CHECK ((witness_prepare_hash ~ ''^hmac-sha256:[0-9a-f]{64}$''::text))'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_previous_hash_check',
               'CHECK ((previous_hash ~ ''^hmac-sha256:[0-9a-f]{64}$''::text))'),
              ('canonical_audit_log'::regclass, 'canonical_audit_log_record_hash_check',
               'CHECK ((record_hash ~ ''^hmac-sha256:[0-9a-f]{64}$''::text))')
          ) AS expected(relation_id, constraint_name, definition)
          LEFT JOIN pg_constraint AS actual
            ON actual.conrelid = expected.relation_id
           AND actual.conname = expected.constraint_name
         WHERE actual.oid IS NULL
            OR NOT actual.convalidated
            OR pg_get_constraintdef(actual.oid) <> expected.definition
    ) THEN
        RAISE EXCEPTION 'unsupported event persistence schema: base constraint drift';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = 'event_outbox'
           AND column_name = 'event_id'
    ) AND NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'event_outbox'::regclass
           AND conname = 'event_outbox_event_id_fkey'
           AND convalidated
           AND pg_get_constraintdef(oid) =
               'FOREIGN KEY (event_id) REFERENCES event_store(sequence)'
    ) THEN
        RAISE EXCEPTION 'unsupported event_outbox schema: event_id foreign key drift';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'event_store'
           AND indexname = 'event_store_campaign_stream_version_idx'
           AND indexdef =
               'CREATE UNIQUE INDEX event_store_campaign_stream_version_idx ON public.event_store USING btree (campaign_id, stream_version)'
    ) THEN
        RAISE EXCEPTION 'unsupported event_store schema: campaign/version index drift';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_language AS language ON language.oid = procedure.prolang
         WHERE procedure.oid =
                   to_regprocedure('public.reject_canonical_append_mutation()')
           AND language.lanname = 'plpgsql'
           AND procedure.prorettype = 'trigger'::regtype
           AND procedure.prokind = 'f'
           AND procedure.pronargs = 0
           AND NOT procedure.proretset
           AND procedure.provolatile = 'v'
           AND NOT procedure.prosecdef
           AND NOT procedure.proleakproof
           AND NOT procedure.proisstrict
           AND procedure.proparallel = 'u'
           AND procedure.proconfig IS NULL
           AND procedure.procost = 100
           AND procedure.prorows = 0
           AND procedure.prosupport = 0
           AND procedure.proowner = (
               SELECT relowner FROM pg_class WHERE oid = 'event_store'::regclass
           )
           AND procedure.prosrc = $function$
BEGIN
    RAISE EXCEPTION 'canonical commit records are append-only';
END;
$function$
    ) THEN
        RAISE EXCEPTION 'unsupported event persistence schema: mutation function drift';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_language AS language ON language.oid = procedure.prolang
         WHERE procedure.oid =
                   to_regprocedure('public.enforce_canonical_audit_chain()')
           AND language.lanname = 'plpgsql'
           AND procedure.prorettype = 'trigger'::regtype
           AND procedure.prokind = 'f'
           AND procedure.pronargs = 0
           AND NOT procedure.proretset
           AND procedure.provolatile = 'v'
           AND NOT procedure.prosecdef
           AND NOT procedure.proleakproof
           AND NOT procedure.proisstrict
           AND procedure.proparallel = 'u'
           AND procedure.proconfig IS NULL
           AND procedure.procost = 100
           AND procedure.prorows = 0
           AND procedure.prosupport = 0
           AND procedure.proowner = (
               SELECT relowner FROM pg_class WHERE oid = 'event_store'::regclass
           )
           AND procedure.prosrc = $function$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended('trpg.canonical_audit_log.chain', 0));
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM canonical_audit_log
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
$function$
    ) THEN
        RAISE EXCEPTION 'unsupported canonical_audit_log schema: chain function drift';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('event_store'::regclass, 'event_store_append_only', 27::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER event_store_append_only BEFORE DELETE OR UPDATE ON public.event_store FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation()'),
              ('event_store'::regclass, 'event_store_no_truncate', 34::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER event_store_no_truncate BEFORE TRUNCATE ON public.event_store FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation()'),
              ('canonical_audit_log'::regclass,
               'canonical_audit_log_append_only', 27::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER canonical_audit_log_append_only BEFORE DELETE OR UPDATE ON public.canonical_audit_log FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation()'),
              ('canonical_audit_log'::regclass,
               'canonical_audit_log_no_truncate', 34::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER canonical_audit_log_no_truncate BEFORE TRUNCATE ON public.canonical_audit_log FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation()'),
              ('canonical_audit_log'::regclass,
               'canonical_audit_log_chain_guard', 7::smallint,
               'enforce_canonical_audit_chain()'::regprocedure,
               'CREATE TRIGGER canonical_audit_log_chain_guard BEFORE INSERT ON public.canonical_audit_log FOR EACH ROW EXECUTE FUNCTION enforce_canonical_audit_chain()'),
              ('formal_commits'::regclass, 'formal_commits_append_only', 27::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER formal_commits_append_only BEFORE DELETE OR UPDATE ON public.formal_commits FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation()'),
              ('formal_commits'::regclass, 'formal_commits_no_truncate', 34::smallint,
               'reject_canonical_append_mutation()'::regprocedure,
               'CREATE TRIGGER formal_commits_no_truncate BEFORE TRUNCATE ON public.formal_commits FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation()')
          ) AS expected(
              relation_id, trigger_name, trigger_type,
              trigger_function, trigger_definition
          )
          LEFT JOIN pg_trigger AS actual
            ON actual.tgrelid = expected.relation_id
           AND actual.tgname = expected.trigger_name
           AND NOT actual.tgisinternal
         WHERE actual.oid IS NULL
            OR actual.tgenabled <> 'O'
            OR actual.tgtype <> expected.trigger_type
            OR actual.tgfoid <> expected.trigger_function
            OR actual.tgconstraint <> 0
            OR actual.tgdeferrable
            OR actual.tginitdeferred
            OR actual.tgconstrrelid <> 0
            OR octet_length(actual.tgargs) <> 0
            OR actual.tgqual IS NOT NULL
            OR pg_get_triggerdef(actual.oid, false) <> expected.trigger_definition
    ) THEN
        RAISE EXCEPTION 'unsupported event persistence schema: mutation trigger drift';
    END IF;

    IF EXISTS (
        SELECT event_sequence
          FROM event_outbox
         GROUP BY event_sequence
        HAVING count(*) <> 1
    ) THEN
        RAISE EXCEPTION 'unsupported event_outbox data: event has duplicate outbox rows';
    END IF;
    IF EXISTS (
        SELECT 1 FROM event_store AS event
         WHERE NOT EXISTS (
             SELECT 1 FROM event_outbox AS outbox
              WHERE outbox.event_sequence = event.sequence
         )
    ) THEN
        RAISE EXCEPTION 'unsupported event persistence data: event lacks outbox row';
    END IF;
END;
$$;

-- Temporarily remove only row-mutation guards needed for backfill. The
-- no-truncate guards stay enabled for the entire migration.
DROP TRIGGER event_store_append_only ON event_store;
DROP TRIGGER formal_commits_append_only ON formal_commits;

-- Version 1 preserves the HMAC input used by records written before P03.
-- New rows default to version 2, whose application hash also binds the
-- database timestamp. Adding the constant default does not execute an UPDATE
-- and therefore never bypasses the audit table's append-only trigger.
ALTER TABLE canonical_audit_log
    ADD COLUMN integrity_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE canonical_audit_log
    ALTER COLUMN integrity_version SET DEFAULT 2,
    ADD CONSTRAINT canonical_audit_log_integrity_version_valid
        CHECK (integrity_version IN (1, 2));

ALTER TABLE event_store
    ADD COLUMN stream_id TEXT,
    ADD COLUMN event_schema_version INTEGER,
    ADD COLUMN idempotency_operation TEXT,
    ADD COLUMN request_hash TEXT,
    ADD COLUMN request_hash_source TEXT,
    ADD COLUMN integrity_status TEXT,
    ADD COLUMN payload_integrity_source TEXT;

UPDATE event_store
   SET stream_id = campaign_id,
       event_schema_version = 1,
       idempotency_operation = 'canonical_commit',
       request_hash =
           'sha256:0000000000000000000000000000000000000000000000000000000000000000',
       request_hash_source = 'historical_unavailable',
       integrity_status = CASE
           WHEN event_integrity_hash IS NULL THEN 'historical_unsigned'
           ELSE 'historical_unverified_hmac'
       END,
       payload_integrity_source = payload_json;

-- Formal request binding follows the exact outbox commit identity. Global
-- sequence ranges can overlap when independent campaigns commit concurrently.
UPDATE event_store AS event
   SET request_hash = formal.request_hash,
       request_hash_source = 'formal_commit'
  FROM event_outbox AS outbox
  JOIN formal_commits AS formal ON formal.commit_id = outbox.commit_id
 WHERE outbox.event_sequence = event.sequence;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM event_store AS event
         WHERE event.event_integrity_hash IS NOT NULL
           AND event.request_hash_source <> 'formal_commit'
    ) THEN
        RAISE EXCEPTION 'unsupported event data: signed event lacks exact formal commit binding';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM event_store AS event
          JOIN event_outbox AS outbox ON outbox.event_sequence = event.sequence
          JOIN formal_commits AS formal ON formal.commit_id = outbox.commit_id
         WHERE event.campaign_id <> formal.campaign_id
            OR event.sequence < formal.first_event_sequence
            OR event.sequence > formal.last_event_sequence
            OR event.stream_version < formal.first_stream_version
            OR event.stream_version > formal.last_stream_version
    ) THEN
        RAISE EXCEPTION 'unsupported event data: formal commit metadata mismatch';
    END IF;
END;
$$;

ALTER TABLE event_store
    ALTER COLUMN stream_id SET NOT NULL,
    ALTER COLUMN event_schema_version SET NOT NULL,
    ALTER COLUMN idempotency_operation SET NOT NULL,
    ALTER COLUMN request_hash SET NOT NULL,
    ALTER COLUMN request_hash_source SET NOT NULL,
    ALTER COLUMN integrity_status SET NOT NULL,
    ALTER COLUMN payload_integrity_source SET NOT NULL,
    ALTER COLUMN payload_json TYPE JSONB USING payload_json::jsonb,
    DROP CONSTRAINT event_store_idempotency_key_key;

DROP INDEX event_store_campaign_stream_version_idx;

ALTER TABLE event_store
    ADD CONSTRAINT event_store_stream_version_uq
        UNIQUE (campaign_id, stream_id, stream_version),
    ADD CONSTRAINT event_store_idempotency_scope_uq
        UNIQUE (campaign_id, stream_id, idempotency_operation, idempotency_key),
    ADD CONSTRAINT event_store_versions_valid CHECK (
        expected_version >= 0
        AND stream_version > 0
        AND authority_contract_version > 0
        AND event_schema_version > 0
    ),
    ADD CONSTRAINT event_store_authority_mode_valid CHECK (
        authority_mode IN ('human_kp', 'ai_kp')
    ),
    ADD CONSTRAINT event_store_visibility_label_valid CHECK (
        visibility_label IN (
            'public', 'party_visible', 'private_to_player', 'keeper_only',
            'investigator_private', 'ai_internal', 'system_only', 'system_private'
        )
    ),
    ADD CONSTRAINT event_store_provenance_kind_valid CHECK (
        fact_provenance_kind IN (
            'human_keeper_statement', 'rules_engine_decision', 'tool_result',
            'agent_proposal', 'imported_source', 'system_fixture'
        )
    ),
    ADD CONSTRAINT event_store_request_binding_valid CHECK (
        request_hash ~ '^sha256:[0-9a-f]{64}$'
        AND (
            request_hash_source = 'formal_commit'
            AND request_hash <>
                'sha256:0000000000000000000000000000000000000000000000000000000000000000'
            OR request_hash_source = 'historical_unavailable'
            AND request_hash =
                'sha256:0000000000000000000000000000000000000000000000000000000000000000'
            AND campaign_id = 'historical_unscoped'
            AND stream_id = 'historical_unscoped'
        )
    ),
    ADD CONSTRAINT event_store_integrity_status_valid CHECK (
        integrity_status = 'verified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unverified_hmac'
        AND request_hash_source = 'formal_commit'
        AND event_integrity_hash ~ '^hmac-sha256:[0-9a-f]{64}$'
        OR integrity_status = 'historical_unsigned'
        AND request_hash_source = 'historical_unavailable'
        AND event_integrity_hash IS NULL
    ),
    ADD CONSTRAINT event_store_payload_integrity_source_valid CHECK (
        payload_integrity_source::jsonb = payload_json
    ),
    ADD CONSTRAINT event_store_nonblank_fields CHECK (
        btrim(event_type) <> ''
        AND btrim(command_id) <> ''
        AND btrim(campaign_id) <> ''
        AND btrim(stream_id) <> ''
        AND btrim(idempotency_operation) <> ''
        AND btrim(idempotency_key) <> ''
        AND btrim(authenticated_actor_id) <> ''
        AND btrim(resource_type) <> ''
        AND btrim(resource_id) <> ''
        AND btrim(authority_contract_id) <> ''
        AND btrim(authority_owner) <> ''
        AND btrim(fact_provenance_reference) <> ''
        AND btrim(fact_recorded_by) <> ''
        AND btrim(correlation_id) <> ''
        AND btrim(causation_id) <> ''
        AND btrim(trace_id) <> ''
    );

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = 'event_outbox'
           AND column_name = 'event_id'
    ) THEN
        ALTER TABLE event_outbox ADD COLUMN event_id BIGINT;
        UPDATE event_outbox SET event_id = event_sequence;
        ALTER TABLE event_outbox
            ADD CONSTRAINT event_outbox_event_id_fkey
            FOREIGN KEY (event_id) REFERENCES event_store(sequence);
    ELSE
        IF EXISTS (
            SELECT 1 FROM event_outbox
             WHERE event_id IS NOT NULL AND event_id <> event_sequence
        ) THEN
            RAISE EXCEPTION 'unsupported event_outbox data: event_id differs from event_sequence';
        END IF;
        UPDATE event_outbox SET event_id = event_sequence WHERE event_id IS NULL;
    END IF;
    ALTER TABLE event_outbox ALTER COLUMN event_id SET NOT NULL;
END;
$$;

ALTER TABLE event_outbox
    ADD COLUMN campaign_id TEXT,
    ADD COLUMN stream_id TEXT,
    ADD COLUMN event_schema_version INTEGER,
    ADD COLUMN idempotency_operation TEXT,
    ADD COLUMN request_hash TEXT,
    ADD COLUMN request_hash_source TEXT,
    ADD COLUMN integrity_status TEXT;

UPDATE event_outbox AS outbox
   SET campaign_id = event.campaign_id,
       stream_id = event.stream_id,
       event_schema_version = event.event_schema_version,
       idempotency_operation = event.idempotency_operation,
       request_hash = event.request_hash,
       request_hash_source = event.request_hash_source,
       integrity_status = event.integrity_status
  FROM event_store AS event
 WHERE event.sequence = outbox.event_sequence;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM event_outbox AS outbox
          JOIN event_store AS event ON event.sequence = outbox.event_sequence
         WHERE outbox.payload_json::jsonb IS DISTINCT FROM event.payload_json
            OR outbox.visibility_label IS DISTINCT FROM event.visibility_label
            OR outbox.correlation_id IS DISTINCT FROM event.correlation_id
            OR outbox.causation_id IS DISTINCT FROM event.causation_id
    ) THEN
        RAISE EXCEPTION 'unsupported event_outbox data: envelope differs from event';
    END IF;

    IF EXISTS (
        SELECT 1 FROM event_outbox AS outbox
         WHERE outbox.commit_id IS NOT NULL
           AND NOT EXISTS (
               SELECT 1 FROM formal_commits AS formal
                WHERE formal.commit_id = outbox.commit_id
           )
    ) THEN
        RAISE EXCEPTION 'unsupported event_outbox data: unknown formal commit';
    END IF;
END;
$$;

ALTER TABLE event_outbox
    ALTER COLUMN campaign_id SET NOT NULL,
    ALTER COLUMN stream_id SET NOT NULL,
    ALTER COLUMN event_schema_version SET NOT NULL,
    ALTER COLUMN idempotency_operation SET NOT NULL,
    ALTER COLUMN request_hash SET NOT NULL,
    ALTER COLUMN request_hash_source SET NOT NULL,
    ALTER COLUMN integrity_status SET NOT NULL,
    ALTER COLUMN payload_json TYPE JSONB USING payload_json::jsonb,
    DROP CONSTRAINT event_outbox_idempotency_key_key;

ALTER TABLE event_outbox
    ADD CONSTRAINT event_outbox_event_sequence_uq UNIQUE (event_sequence),
    ADD CONSTRAINT event_outbox_idempotency_scope_uq
        UNIQUE (campaign_id, stream_id, idempotency_operation, idempotency_key),
    ADD CONSTRAINT event_outbox_commit_fkey
        FOREIGN KEY (commit_id) REFERENCES formal_commits(commit_id)
        DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT event_outbox_event_reference_consistent CHECK (
        event_id = event_sequence AND event_id > 0
    ),
    ADD CONSTRAINT event_outbox_version_retry_valid CHECK (
        event_schema_version > 0 AND retry_count >= 0
    ),
    ADD CONSTRAINT event_outbox_visibility_label_valid CHECK (
        visibility_label IN (
            'public', 'party_visible', 'private_to_player', 'keeper_only',
            'investigator_private', 'ai_internal', 'system_only', 'system_private'
        )
    ),
    ADD CONSTRAINT event_outbox_request_binding_valid CHECK (
        request_hash ~ '^sha256:[0-9a-f]{64}$'
        AND (
            request_hash_source = 'formal_commit'
            AND request_hash <>
                'sha256:0000000000000000000000000000000000000000000000000000000000000000'
            OR request_hash_source = 'historical_unavailable'
            AND request_hash =
                'sha256:0000000000000000000000000000000000000000000000000000000000000000'
        )
    ),
    ADD CONSTRAINT event_outbox_integrity_status_valid CHECK (
        integrity_status = 'verified_hmac'
        AND request_hash_source = 'formal_commit'
        OR integrity_status = 'historical_unverified_hmac'
        AND request_hash_source = 'formal_commit'
        OR integrity_status = 'historical_unsigned'
        AND request_hash_source = 'historical_unavailable'
    ),
    ADD CONSTRAINT event_outbox_nonblank_fields CHECK (
        btrim(campaign_id) <> ''
        AND btrim(stream_id) <> ''
        AND btrim(idempotency_operation) <> ''
        AND btrim(idempotency_key) <> ''
        AND nats_subject = 'trpg.events.appended'
        AND btrim(correlation_id) <> ''
        AND btrim(causation_id) <> ''
    );

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = 'projection_checkpoint'
           AND column_name = 'stream_id'
    ) THEN
        ALTER TABLE projection_checkpoint
            ADD COLUMN stream_id TEXT,
            ADD COLUMN version BIGINT;
        UPDATE projection_checkpoint
           SET stream_id = projection_name,
               version = last_event_sequence;
    END IF;
    ALTER TABLE projection_checkpoint
        ALTER COLUMN stream_id SET NOT NULL,
        ALTER COLUMN version SET NOT NULL;
END;
$$;

ALTER TABLE projection_checkpoint ADD COLUMN campaign_id TEXT;
UPDATE projection_checkpoint SET campaign_id = 'historical_unscoped';
ALTER TABLE projection_checkpoint
    ALTER COLUMN campaign_id SET NOT NULL,
    DROP CONSTRAINT projection_checkpoint_pkey,
    ADD CONSTRAINT projection_checkpoint_pkey
        PRIMARY KEY (projection_name, campaign_id, stream_id),
    ADD CONSTRAINT projection_checkpoint_versions_valid CHECK (
        version >= 0 AND last_event_sequence >= 0
    ),
    ADD CONSTRAINT projection_checkpoint_nonblank_fields CHECK (
        btrim(projection_name) <> ''
        AND btrim(campaign_id) <> ''
        AND btrim(stream_id) <> ''
        AND btrim(projection_hash) <> ''
    );

ALTER TABLE formal_commits
    ADD COLUMN stream_id TEXT,
    ADD COLUMN idempotency_operation TEXT,
    ADD COLUMN status TEXT,
    ADD COLUMN result_event_sequence BIGINT,
    ADD COLUMN response_payload JSONB;

UPDATE formal_commits
   SET stream_id = campaign_id,
       idempotency_operation = 'canonical_commit',
       status = 'committed',
       result_event_sequence = last_event_sequence,
       response_payload = jsonb_build_object(
           'first_event_sequence', first_event_sequence,
           'last_event_sequence', last_event_sequence,
           'first_stream_version', first_stream_version,
           'last_stream_version', last_stream_version
       );

ALTER TABLE formal_commits
    ALTER COLUMN stream_id SET NOT NULL,
    ALTER COLUMN idempotency_operation SET NOT NULL,
    ALTER COLUMN status SET NOT NULL,
    ALTER COLUMN result_event_sequence SET NOT NULL,
    ALTER COLUMN response_payload SET NOT NULL,
    DROP CONSTRAINT formal_commits_idempotency_key_key,
    DROP CONSTRAINT formal_commits_request_hash_check,
    ADD CONSTRAINT formal_commits_result_event_sequence_fkey
        FOREIGN KEY (result_event_sequence) REFERENCES event_store(sequence),
    ADD CONSTRAINT formal_commits_idempotency_scope_uq
        UNIQUE (campaign_id, stream_id, idempotency_operation, idempotency_key),
    ADD CONSTRAINT formal_commits_request_hash_format CHECK (
        request_hash ~ '^sha256:[0-9a-f]{64}$'
        AND request_hash <>
            'sha256:0000000000000000000000000000000000000000000000000000000000000000'
    ),
    ADD CONSTRAINT formal_commits_status_valid CHECK (status = 'committed'),
    ADD CONSTRAINT formal_commits_versions_valid CHECK (
        expected_version >= 0
        AND first_event_sequence > 0
        AND last_event_sequence >= first_event_sequence
        AND result_event_sequence = last_event_sequence
        AND first_stream_version = expected_version + 1
        AND last_stream_version >= first_stream_version
    ),
    ADD CONSTRAINT formal_commits_response_binding_valid CHECK (
        response_payload = jsonb_build_object(
            'first_event_sequence', first_event_sequence,
            'last_event_sequence', last_event_sequence,
            'first_stream_version', first_stream_version,
            'last_stream_version', last_stream_version
        )
    ),
    ADD CONSTRAINT formal_commits_nonblank_fields CHECK (
        btrim(commit_id) <> ''
        AND btrim(campaign_id) <> ''
        AND btrim(stream_id) <> ''
        AND btrim(idempotency_operation) <> ''
        AND btrim(idempotency_key) <> ''
        AND btrim(witness_prepare_hash) <> ''
    );

-- Validate every historical formal marker against the exact event/outbox set.
DO $$
DECLARE
    invalid_commit TEXT;
BEGIN
    SELECT formal.commit_id
      INTO invalid_commit
      FROM formal_commits AS formal
      LEFT JOIN LATERAL (
          SELECT count(*) AS event_count,
                 min(event.sequence) AS first_sequence,
                 max(event.sequence) AS last_sequence,
                 min(event.stream_version) AS first_version,
                 max(event.stream_version) AS last_version,
                 bool_and(event.campaign_id = formal.campaign_id) AS campaign_matches,
                 bool_and(event.stream_id = formal.stream_id) AS stream_matches,
                 bool_and(event.request_hash = formal.request_hash) AS event_hash_matches,
                 bool_and(outbox.request_hash = formal.request_hash) AS outbox_hash_matches,
                 bool_and(event.request_hash_source = 'formal_commit') AS event_source_matches,
                 bool_and(outbox.request_hash_source = 'formal_commit') AS outbox_source_matches,
                 bool_and(
                     event.integrity_status = outbox.integrity_status
                     AND event.integrity_status IN (
                         'verified_hmac', 'historical_unverified_hmac'
                     )
                     AND event.event_integrity_hash IS NOT NULL
                 ) AS integrity_matches,
                 count(DISTINCT event.integrity_status) AS integrity_status_count
            FROM event_outbox AS outbox
            JOIN event_store AS event ON event.sequence = outbox.event_sequence
           WHERE outbox.commit_id = formal.commit_id
      ) AS bound ON TRUE
     WHERE bound.event_count <> formal.last_stream_version - formal.first_stream_version + 1
        OR bound.first_sequence <> formal.first_event_sequence
        OR bound.last_sequence <> formal.last_event_sequence
        OR bound.first_version <> formal.first_stream_version
        OR bound.last_version <> formal.last_stream_version
        OR NOT COALESCE(bound.campaign_matches, FALSE)
        OR NOT COALESCE(bound.stream_matches, FALSE)
        OR NOT COALESCE(bound.event_hash_matches, FALSE)
        OR NOT COALESCE(bound.outbox_hash_matches, FALSE)
        OR NOT COALESCE(bound.event_source_matches, FALSE)
        OR NOT COALESCE(bound.outbox_source_matches, FALSE)
        OR NOT COALESCE(bound.integrity_matches, FALSE)
        OR bound.integrity_status_count <> 1
     LIMIT 1;
    IF invalid_commit IS NOT NULL THEN
        RAISE EXCEPTION 'formal commit % is not bound to its exact event/outbox set', invalid_commit;
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION enforce_canonical_audit_chain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    latest_sequence BIGINT;
    latest_hash TEXT;
BEGIN
    IF NEW.integrity_version <> 2 THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = 'historical audit integrity version is migration-only';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended('trpg.canonical_audit_log.chain', 0));
    SELECT sequence, record_hash
      INTO latest_sequence, latest_hash
      FROM canonical_audit_log
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

CREATE OR REPLACE FUNCTION enforce_event_outbox_binding()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    stored_event event_store%ROWTYPE;
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
      FROM event_store
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

CREATE TRIGGER event_outbox_event_binding
BEFORE INSERT OR UPDATE ON event_outbox
FOR EACH ROW EXECUTE FUNCTION enforce_event_outbox_binding();

CREATE TRIGGER event_outbox_no_delete
BEFORE DELETE ON event_outbox
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

CREATE TRIGGER event_outbox_no_truncate
BEFORE TRUNCATE ON event_outbox
FOR EACH STATEMENT EXECUTE FUNCTION reject_canonical_append_mutation();

CREATE OR REPLACE FUNCTION enforce_formal_commit_binding()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    event_count BIGINT;
    first_sequence BIGINT;
    last_sequence BIGINT;
    first_version BIGINT;
    last_version BIGINT;
    all_metadata_matches BOOLEAN;
BEGIN
    SELECT count(*), min(event.sequence), max(event.sequence),
           min(event.stream_version), max(event.stream_version),
           bool_and(
               event.campaign_id = NEW.campaign_id
               AND event.stream_id = NEW.stream_id
               AND event.request_hash = NEW.request_hash
               AND event.request_hash_source = 'formal_commit'
               AND outbox.request_hash = NEW.request_hash
               AND outbox.request_hash_source = 'formal_commit'
               AND event.integrity_status = 'verified_hmac'
               AND outbox.integrity_status = 'verified_hmac'
               AND event.event_integrity_hash IS NOT NULL
           )
      INTO event_count, first_sequence, last_sequence,
           first_version, last_version, all_metadata_matches
      FROM event_outbox AS outbox
      JOIN event_store AS event ON event.sequence = outbox.event_sequence
     WHERE outbox.commit_id = NEW.commit_id;
    IF event_count <> NEW.last_stream_version - NEW.first_stream_version + 1
       OR first_sequence IS DISTINCT FROM NEW.first_event_sequence
       OR last_sequence IS DISTINCT FROM NEW.last_event_sequence
       OR first_version IS DISTINCT FROM NEW.first_stream_version
       OR last_version IS DISTINCT FROM NEW.last_stream_version
       OR NOT COALESCE(all_metadata_matches, FALSE) THEN
        RAISE EXCEPTION 'formal commit is not bound to its exact event/outbox set';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER formal_commits_event_binding
BEFORE INSERT ON formal_commits
FOR EACH ROW EXECUTE FUNCTION enforce_formal_commit_binding();

CREATE OR REPLACE FUNCTION enforce_existing_formal_commit_set()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    formal formal_commits%ROWTYPE;
    event_count BIGINT;
    first_sequence BIGINT;
    last_sequence BIGINT;
    first_version BIGINT;
    last_version BIGINT;
    all_metadata_matches BOOLEAN;
    integrity_status_count BIGINT;
BEGIN
    IF NEW.commit_id IS NULL THEN
        RETURN NULL;
    END IF;

    SELECT * INTO formal
      FROM formal_commits
     WHERE commit_id = NEW.commit_id;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'outbox formal commit does not exist';
    END IF;

    SELECT count(*), min(event.sequence), max(event.sequence),
           min(event.stream_version), max(event.stream_version),
           bool_and(
               event.campaign_id = formal.campaign_id
               AND event.stream_id = formal.stream_id
               AND event.request_hash = formal.request_hash
               AND event.request_hash_source = 'formal_commit'
               AND outbox.request_hash = formal.request_hash
               AND outbox.request_hash_source = 'formal_commit'
               AND event.integrity_status = outbox.integrity_status
               AND event.integrity_status IN (
                   'verified_hmac', 'historical_unverified_hmac'
               )
               AND event.event_integrity_hash IS NOT NULL
           ),
           count(DISTINCT event.integrity_status)
      INTO event_count, first_sequence, last_sequence,
           first_version, last_version, all_metadata_matches,
           integrity_status_count
      FROM event_outbox AS outbox
      JOIN event_store AS event ON event.sequence = outbox.event_sequence
     WHERE outbox.commit_id = formal.commit_id;
    IF event_count <> formal.last_stream_version - formal.first_stream_version + 1
       OR first_sequence IS DISTINCT FROM formal.first_event_sequence
       OR last_sequence IS DISTINCT FROM formal.last_event_sequence
       OR first_version IS DISTINCT FROM formal.first_stream_version
       OR last_version IS DISTINCT FROM formal.last_stream_version
       OR NOT COALESCE(all_metadata_matches, FALSE)
       OR integrity_status_count <> 1 THEN
        RAISE EXCEPTION 'formal commit exact event/outbox set changed after commit';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER event_outbox_formal_commit_set
AFTER INSERT ON event_outbox
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION enforce_existing_formal_commit_set();

CREATE OR REPLACE FUNCTION enforce_event_formal_completion()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    binding_count BIGINT;
BEGIN
    IF NEW.request_hash_source = 'formal_commit' THEN
        SELECT count(*)
          INTO binding_count
          FROM event_outbox AS outbox
          JOIN formal_commits AS formal ON formal.commit_id = outbox.commit_id
         WHERE outbox.event_sequence = NEW.sequence
           AND formal.campaign_id = NEW.campaign_id
           AND formal.stream_id = NEW.stream_id
           AND formal.request_hash = NEW.request_hash
           AND NEW.integrity_status = 'verified_hmac'
           AND outbox.integrity_status = 'verified_hmac';
        IF binding_count <> 1 THEN
            RAISE EXCEPTION 'formal event lacks one complete outbox/commit binding';
        END IF;
    ELSIF NEW.request_hash_source = 'historical_unavailable' THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = 'historical classification is migration-only';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER event_store_formal_completion
AFTER INSERT ON event_store
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION enforce_event_formal_completion();

-- Historical classification exists only to describe rows that were present
-- before this hardening migration.  Once the backfill and validation above
-- have completed, no application transaction may manufacture another
-- unsigned canonical event or outbox row.
CREATE OR REPLACE FUNCTION reject_historical_classification_insert()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.request_hash_source = 'historical_unavailable'
       OR NEW.integrity_status IN (
           'historical_unsigned', 'historical_unverified_hmac'
       ) THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = 'historical classification is migration-only';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER event_store_classification_insert_guard
BEFORE INSERT ON event_store
FOR EACH ROW EXECUTE FUNCTION reject_historical_classification_insert();

CREATE TRIGGER event_outbox_classification_insert_guard
BEFORE INSERT ON event_outbox
FOR EACH ROW EXECUTE FUNCTION reject_historical_classification_insert();

CREATE TRIGGER event_store_append_only
BEFORE UPDATE OR DELETE ON event_store
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

CREATE TRIGGER formal_commits_append_only
BEFORE UPDATE OR DELETE ON formal_commits
FOR EACH ROW EXECUTE FUNCTION reject_canonical_append_mutation();

-- Fail the migration itself if any trigger predicate/argument or trigger
-- function execution property differs from the canonical P03 definition.
-- OIDs are deliberately excluded; semantic catalog metadata and deparsed DDL
-- are included so restore/recreate remains portable while drift fails closed.
DO $$
DECLARE
    trigger_signature TEXT;
    trigger_function_signature TEXT;
BEGIN
    PERFORM set_config('search_path', 'public, pg_catalog', true);

    SELECT md5(string_agg(
               concat(
                   namespace.nspname, '.', relation.relname, '|',
                   catalog_trigger.tgname, '|', catalog_trigger.tgenabled::text, '|',
                   catalog_trigger.tgtype::text, '|',
                   catalog_trigger.tgisinternal::text, '|',
                   (catalog_trigger.tgconstraint <> 0)::text, '|',
                   catalog_trigger.tgdeferrable::text, '|',
                   catalog_trigger.tginitdeferred::text, '|',
                   CASE WHEN catalog_trigger.tgconstrrelid = 0 THEN '-'
                        ELSE constraint_namespace.nspname || '.' ||
                             constraint_relation.relname END, '|',
                   encode(catalog_trigger.tgargs, 'hex'), '|',
                   COALESCE(
                       pg_get_expr(catalog_trigger.tgqual, catalog_trigger.tgrelid),
                       '-'
                   ), '|',
                   pg_get_triggerdef(catalog_trigger.oid, false)
               ),
               E'\n' ORDER BY namespace.nspname, relation.relname,
                               catalog_trigger.tgname
           ))
      INTO trigger_signature
      FROM pg_trigger AS catalog_trigger
      JOIN pg_class AS relation ON relation.oid = catalog_trigger.tgrelid
      JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace
      LEFT JOIN pg_class AS constraint_relation
        ON constraint_relation.oid = catalog_trigger.tgconstrrelid
      LEFT JOIN pg_namespace AS constraint_namespace
        ON constraint_namespace.oid = constraint_relation.relnamespace
     WHERE NOT catalog_trigger.tgisinternal
       AND catalog_trigger.tgrelid IN (
           'event_store'::regclass, 'event_outbox'::regclass,
           'formal_commits'::regclass, 'canonical_audit_log'::regclass
       );
    IF trigger_signature IS NULL
       OR trigger_signature <> '5b2eddf13c822cb4a220f7f9c790cc73' THEN
        RAISE EXCEPTION 'event persistence final trigger definition signature drifted: %',
            trigger_signature;
    END IF;

    SELECT md5(string_agg(
               concat(
                   namespace.nspname, '.', procedure.proname, '|',
                   pg_get_function_identity_arguments(procedure.oid), '|',
                   pg_get_function_result(procedure.oid), '|', language.lanname, '|',
                   procedure.prokind::text, '|', procedure.provolatile::text, '|',
                   procedure.proparallel::text, '|', procedure.proisstrict::text, '|',
                   procedure.prosecdef::text, '|', procedure.proleakproof::text, '|',
                   procedure.proretset::text, '|', procedure.procost::text, '|',
                   procedure.prorows::text, '|',
                   COALESCE(array_to_string(procedure.proconfig, E'\x1f'), '-'), '|',
                   (procedure.proowner = event_store_relation.relowner)::text, '|',
                   CASE WHEN procedure.prosupport = 0 THEN '-'
                        ELSE procedure.prosupport::regproc::text END, '|',
                   pg_get_functiondef(procedure.oid)
               ),
               E'\n' ORDER BY namespace.nspname, procedure.proname,
                               pg_get_function_identity_arguments(procedure.oid)
           ))
      INTO trigger_function_signature
      FROM pg_proc AS procedure
      JOIN pg_namespace AS namespace ON namespace.oid = procedure.pronamespace
      JOIN pg_language AS language ON language.oid = procedure.prolang
      CROSS JOIN pg_class AS event_store_relation
     WHERE event_store_relation.oid = 'event_store'::regclass
       AND procedure.oid IN (
           'reject_canonical_append_mutation()'::regprocedure,
           'enforce_canonical_audit_chain()'::regprocedure,
           'enforce_event_outbox_binding()'::regprocedure,
           'enforce_formal_commit_binding()'::regprocedure,
           'enforce_existing_formal_commit_set()'::regprocedure,
           'enforce_event_formal_completion()'::regprocedure,
           'reject_historical_classification_insert()'::regprocedure
       );
    IF trigger_function_signature IS NULL
       OR trigger_function_signature <> '6db05b2875333b14d2962a4f56d433e8' THEN
        RAISE EXCEPTION 'event persistence final trigger function signature drifted: %',
            trigger_function_signature;
    END IF;
END;
$$;
