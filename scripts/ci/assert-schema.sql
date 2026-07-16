\set ON_ERROR_STOP on

DO $$
DECLARE
    actual_columns TEXT[];
    constraint_signature TEXT;
    trigger_signature TEXT;
    trigger_function_signature TEXT;
    invalid_commit TEXT;
BEGIN
    -- Make catalog deparsing deterministic for callers with a custom
    -- search_path. All canonical persistence objects live in public.
    PERFORM set_config('search_path', 'public, pg_catalog', true);

    IF to_regclass('public._sqlx_migrations') IS NULL THEN
        RAISE EXCEPTION 'SQLx migration ledger is missing';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM _sqlx_migrations
         WHERE version = 20260705000100
           AND success
           AND encode(checksum, 'hex') =
               '40539cf7e8f2fd0a87481a7c41dc1d14b24083ceaee3dbe3ab3d6f6b38e76bbfd117942b3d20b4ef547ccb40be709379'
    ) THEN
        RAISE EXCEPTION 'published event-store migration checksum is not frozen';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM _sqlx_migrations
         WHERE version = 20260716000100 AND success
    ) THEN
        RAISE EXCEPTION 'event persistence hardening migration is not applied';
    END IF;
    IF EXISTS (SELECT 1 FROM _sqlx_migrations WHERE NOT success) THEN
        RAISE EXCEPTION 'failed SQLx migration ledger row found';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        RAISE EXCEPTION 'pgvector extension is missing';
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'event_store';
    IF actual_columns IS NULL OR actual_columns <> ARRAY[
        'sequence:int8:NO:nextval(''event_store_sequence_seq''::regclass)',
        'event_type:text:NO:-', 'command_id:text:NO:-',
        'idempotency_key:text:NO:-', 'expected_version:int8:NO:-',
        'authority_mode:text:NO:-', 'authority_contract_version:int8:NO:-',
        'visibility_label:text:NO:-', 'fact_provenance_kind:text:NO:-',
        'fact_provenance_reference:text:NO:-', 'fact_recorded_by:text:NO:-',
        'correlation_id:text:NO:-', 'causation_id:text:NO:-',
        'payload_json:jsonb:NO:-', 'recorded_at:timestamptz:NO:now()',
        'campaign_id:text:NO:''historical_unscoped''::text',
        'stream_version:int8:NO:-',
        'authenticated_actor_id:text:NO:''historical_unknown''::text',
        'resource_type:text:NO:''historical_unknown''::text',
        'resource_id:text:NO:''historical_unknown''::text',
        'authority_contract_id:text:NO:''historical_unknown''::text',
        'authority_owner:text:NO:''historical_unknown''::text',
        'visibility_subject:text:NO:''not_applicable''::text',
        'trace_id:text:NO:''historical_unknown''::text',
        'event_integrity_hash:text:YES:-', 'stream_id:text:NO:-',
        'event_schema_version:int4:NO:-', 'idempotency_operation:text:NO:-',
        'request_hash:text:NO:-', 'request_hash_source:text:NO:-',
        'integrity_status:text:NO:-', 'payload_integrity_source:text:NO:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'event_store columns/types/nullability/defaults drifted: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'event_outbox';
    IF actual_columns IS NULL OR (actual_columns <> ARRAY[
        'outbox_id:int8:NO:nextval(''event_outbox_outbox_id_seq''::regclass)',
        'event_sequence:int8:NO:-', 'nats_subject:text:NO:-',
        'idempotency_key:text:NO:-', 'visibility_label:text:NO:-',
        'correlation_id:text:NO:-', 'causation_id:text:NO:-',
        'payload_json:jsonb:NO:-', 'published_at:timestamptz:YES:-',
        'retry_count:int4:NO:0', 'commit_id:text:YES:-',
        'claimed_at:timestamptz:YES:-', 'claim_owner:text:YES:-',
        'last_error:text:YES:-', 'dead_lettered_at:timestamptz:YES:-',
        'event_id:int8:NO:-', 'campaign_id:text:NO:-', 'stream_id:text:NO:-',
        'event_schema_version:int4:NO:-', 'idempotency_operation:text:NO:-',
        'request_hash:text:NO:-', 'request_hash_source:text:NO:-',
        'integrity_status:text:NO:-'
    ]::TEXT[] AND actual_columns <> ARRAY[
        'outbox_id:int8:NO:nextval(''event_outbox_outbox_id_seq''::regclass)',
        'event_id:int8:NO:-', 'event_sequence:int8:NO:-',
        'nats_subject:text:NO:-', 'idempotency_key:text:NO:-',
        'visibility_label:text:NO:-', 'correlation_id:text:NO:-',
        'causation_id:text:NO:-', 'payload_json:jsonb:NO:-',
        'published_at:timestamptz:YES:-', 'retry_count:int4:NO:0',
        'commit_id:text:YES:-', 'claimed_at:timestamptz:YES:-',
        'claim_owner:text:YES:-', 'last_error:text:YES:-',
        'dead_lettered_at:timestamptz:YES:-', 'campaign_id:text:NO:-',
        'stream_id:text:NO:-', 'event_schema_version:int4:NO:-',
        'idempotency_operation:text:NO:-', 'request_hash:text:NO:-',
        'request_hash_source:text:NO:-', 'integrity_status:text:NO:-'
    ]::TEXT[]) THEN
        RAISE EXCEPTION 'event_outbox columns/types/nullability/defaults drifted: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'projection_checkpoint';
    IF actual_columns IS NULL OR (actual_columns <> ARRAY[
        'projection_name:text:NO:-', 'last_event_sequence:int8:NO:-',
        'projection_hash:text:NO:-', 'rebuilt_at:timestamptz:NO:now()',
        'stream_id:text:NO:-', 'version:int8:NO:-', 'campaign_id:text:NO:-'
    ]::TEXT[] AND actual_columns <> ARRAY[
        'projection_name:text:NO:-', 'stream_id:text:NO:-', 'version:int8:NO:-',
        'last_event_sequence:int8:NO:-', 'projection_hash:text:NO:-',
        'rebuilt_at:timestamptz:NO:now()', 'campaign_id:text:NO:-'
    ]::TEXT[]) THEN
        RAISE EXCEPTION 'projection_checkpoint columns/types/nullability/defaults drifted: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'formal_commits';
    IF actual_columns IS NULL OR actual_columns <> ARRAY[
        'commit_id:text:NO:-', 'campaign_id:text:NO:-',
        'idempotency_key:text:NO:-', 'request_hash:text:NO:-',
        'expected_version:int8:NO:-', 'first_event_sequence:int8:NO:-',
        'last_event_sequence:int8:NO:-', 'first_stream_version:int8:NO:-',
        'last_stream_version:int8:NO:-', 'audit_sequence:int8:NO:-',
        'witness_prepare_sequence:int8:NO:-', 'witness_prepare_hash:text:NO:-',
        'committed_at:timestamptz:NO:now()', 'stream_id:text:NO:-',
        'idempotency_operation:text:NO:-', 'status:text:NO:-',
        'result_event_sequence:int8:NO:-', 'response_payload:jsonb:NO:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'formal_commits columns/types/nullability/defaults drifted: %', actual_columns;
    END IF;

    SELECT array_agg(
               column_name || ':' || udt_name || ':' || is_nullable || ':' ||
               COALESCE(column_default, '-')
               ORDER BY ordinal_position
           )
      INTO actual_columns
      FROM information_schema.columns
     WHERE table_schema = 'public' AND table_name = 'canonical_audit_log';
    IF actual_columns IS NULL OR actual_columns <> ARRAY[
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
        'previous_hash:text:NO:-', 'record_hash:text:NO:-',
        'integrity_version:int4:NO:2'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'canonical_audit_log columns/types/nullability/defaults drifted: %',
            actual_columns;
    END IF;

    SELECT md5(string_agg(
               conrelid::regclass::text || '|' || conname || '|' || contype::text ||
               '|' || convalidated::text || '|' || condeferrable::text || '|' ||
               condeferred::text || '|' || pg_get_constraintdef(oid),
               E'\n' ORDER BY conrelid::regclass::text, conname
           ))
      INTO constraint_signature
      FROM pg_constraint
     WHERE connamespace = 'public'::regnamespace
       AND conrelid IN (
           'event_store'::regclass, 'event_outbox'::regclass,
           'projection_checkpoint'::regclass, 'formal_commits'::regclass,
           'canonical_audit_log'::regclass
       );
    IF constraint_signature IS NULL
       OR constraint_signature <> '1289e4f2857a305fc7283fd02319db11' THEN
        RAISE EXCEPTION 'event persistence constraint relation/definition signature drifted: %',
            constraint_signature;
    END IF;

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
        RAISE EXCEPTION 'event persistence trigger relation/enabled/definition signature drifted: %',
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
        RAISE EXCEPTION 'event persistence trigger function definition/execution signature drifted: %',
            trigger_function_signature;
    END IF;

    BEGIN
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'SchemaHistoricalWriteProbe', 'schema_historical_command',
            'schema_historical_event', 0, 'human_kp', 1, 'party_visible',
            'imported_source', 'schema_assertion', 'schema_assertion',
            'schema_historical_correlation', 'schema_historical_causation',
            '{}'::jsonb, 'historical_unscoped', 1, 'historical_unknown',
            'historical_unknown', 'historical_unknown', 'historical_unknown',
            'historical_unknown', 'not_applicable', 'historical_unknown', NULL,
            'historical_unscoped', 1, 'canonical_commit',
            'sha256:0000000000000000000000000000000000000000000000000000000000000000',
            'historical_unavailable', 'historical_unsigned', '{}'
        );
        RAISE EXCEPTION 'post-migration historical event insertion was accepted';
    EXCEPTION WHEN check_violation THEN
        IF SQLERRM <> 'historical classification is migration-only' THEN
            RAISE EXCEPTION 'historical event guard returned an unexpected error: %', SQLERRM;
        END IF;
    END;

    BEGIN
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            9223372036854775807, 9223372036854775807,
            'trpg.events.appended', 'schema_historical_outbox',
            'party_visible', 'schema_historical_correlation',
            'schema_historical_causation', '{}'::jsonb, 0,
            'historical_unscoped', 'historical_unscoped', 1,
            'canonical_commit',
            'sha256:0000000000000000000000000000000000000000000000000000000000000000',
            'historical_unavailable', 'historical_unsigned'
        );
        RAISE EXCEPTION 'post-migration historical outbox insertion was accepted';
    EXCEPTION WHEN check_violation THEN
        IF SQLERRM <> 'historical classification is migration-only' THEN
            RAISE EXCEPTION 'historical outbox guard returned an unexpected error: %', SQLERRM;
        END IF;
    END;

    BEGIN
        INSERT INTO canonical_audit_log (
            sequence, commit_id, campaign_id, actor_id, actor_origin,
            authentication_reference, resource_type, resource_id, action,
            requested_role, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            decision, openfga_decision_id, openfga_policy_revision,
            opa_decision_id, opa_policy_revision, trace_id, event_batch_hash,
            witness_prepare_sequence, witness_prepare_hash, integrity_version,
            integrity_key_id, previous_hash, record_hash
        ) VALUES (
            0, 'schema_historical_audit_probe', 'schema_campaign',
            'schema_actor', 'system_fixture', 'schema_auth', 'campaign',
            'schema_campaign', 'write_official_state', 'human_keeper',
            'party_visible', 'not_applicable', 'system_fixture',
            'schema_assertion', 'schema_assertion', 'PERMIT', 'schema_fga',
            'schema_fga_revision', 'schema_opa', 'schema_opa_revision',
            'schema_trace',
            'sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
            1,
            'hmac-sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
            1, 'schema_key',
            COALESCE(
                (SELECT record_hash FROM canonical_audit_log ORDER BY sequence DESC LIMIT 1),
                'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
            ),
            'hmac-sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'
        );
        RAISE EXCEPTION 'post-migration historical audit insertion was accepted';
    EXCEPTION WHEN check_violation THEN
        IF SQLERRM <> 'historical audit integrity version is migration-only' THEN
            RAISE EXCEPTION 'historical audit guard returned an unexpected error: %', SQLERRM;
        END IF;
    END;

    BEGIN
        INSERT INTO canonical_audit_log (
            sequence, commit_id, campaign_id, actor_id, actor_origin,
            authentication_reference, resource_type, resource_id, action,
            requested_role, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            decision, openfga_decision_id, openfga_policy_revision,
            opa_decision_id, opa_policy_revision, trace_id, event_batch_hash,
            witness_prepare_sequence, witness_prepare_hash, occurred_at,
            integrity_version, integrity_key_id, previous_hash, record_hash
        ) VALUES (
            0, 'schema_audit_mutation_probe', 'schema_campaign',
            'schema_actor', 'system_fixture', 'schema_auth', 'campaign',
            'schema_campaign', 'write_official_state', 'human_keeper',
            'party_visible', 'not_applicable', 'system_fixture',
            'schema_assertion', 'schema_assertion', 'PERMIT', 'schema_fga',
            'schema_fga_revision', 'schema_opa', 'schema_opa_revision',
            'schema_trace',
            'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            1,
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            '2026-07-16T00:00:00Z'::timestamptz, 2, 'schema_key',
            COALESCE(
                (SELECT record_hash FROM canonical_audit_log ORDER BY sequence DESC LIMIT 1),
                'hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000'
            ),
            'hmac-sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc'
        );
        UPDATE canonical_audit_log
           SET occurred_at = occurred_at + interval '1 microsecond'
         WHERE commit_id = 'schema_audit_mutation_probe';
        RAISE EXCEPTION 'canonical audit timestamp mutation was accepted';
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM <> 'canonical commit records are append-only' THEN
            RAISE EXCEPTION 'canonical audit mutation guard returned an unexpected error: %', SQLERRM;
        END IF;
    END;

    IF EXISTS (
        SELECT 1
          FROM event_store AS event
         WHERE NOT EXISTS (
             SELECT 1
               FROM event_outbox AS outbox
              WHERE outbox.event_sequence = event.sequence
         )
    ) THEN
        RAISE EXCEPTION 'stored canonical event lacks its outbox row';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM event_outbox AS outbox
          JOIN event_store AS event ON event.sequence = outbox.event_sequence
         WHERE outbox.event_id IS DISTINCT FROM event.sequence
            OR outbox.campaign_id IS DISTINCT FROM event.campaign_id
            OR outbox.stream_id IS DISTINCT FROM event.stream_id
            OR outbox.event_schema_version IS DISTINCT FROM event.event_schema_version
            OR outbox.idempotency_operation IS DISTINCT FROM event.idempotency_operation
            OR outbox.visibility_label IS DISTINCT FROM event.visibility_label
            OR outbox.correlation_id IS DISTINCT FROM event.correlation_id
            OR outbox.causation_id IS DISTINCT FROM event.causation_id
            OR outbox.payload_json IS DISTINCT FROM event.payload_json
            OR outbox.request_hash IS DISTINCT FROM event.request_hash
            OR outbox.request_hash_source IS DISTINCT FROM event.request_hash_source
            OR outbox.integrity_status IS DISTINCT FROM event.integrity_status
            OR outbox.request_hash_source = 'formal_commit' AND outbox.commit_id IS NULL
            OR outbox.request_hash_source = 'historical_unavailable' AND outbox.commit_id IS NOT NULL
            OR outbox.nats_subject <> 'trpg.events.appended'
    ) THEN
        RAISE EXCEPTION 'stored outbox/event metadata binding is invalid';
    END IF;

    IF EXISTS (
        SELECT 1 FROM event_store
         WHERE integrity_status IN (
                   'verified_hmac', 'historical_unverified_hmac'
               ) AND event_integrity_hash IS NULL
            OR integrity_status = 'historical_unsigned' AND event_integrity_hash IS NOT NULL
            OR integrity_status = 'historical_unverified_hmac'
               AND request_hash_source <> 'formal_commit'
            OR request_hash_source = 'formal_commit' AND request_hash =
               'sha256:0000000000000000000000000000000000000000000000000000000000000000'
            OR request_hash_source = 'historical_unavailable' AND request_hash <>
               'sha256:0000000000000000000000000000000000000000000000000000000000000000'
    ) THEN
        RAISE EXCEPTION 'stored event integrity/request classification is invalid';
    END IF;

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

SELECT 'P03_SCHEMA_ASSERTION_OK' AS schema_assertion;
