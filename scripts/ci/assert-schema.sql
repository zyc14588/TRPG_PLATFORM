\set ON_ERROR_STOP on

BEGIN;

DO $$
DECLARE
    actual_columns TEXT[];
    constraint_signature TEXT;
    expected_constraint_signature TEXT;
    postgres_major INTEGER;
    trigger_signature TEXT;
    trigger_function_signature TEXT;
    invalid_commit TEXT;
    search_path_bypass_rejected BOOLEAN := FALSE;
    erased_user_reactivation_rejected BOOLEAN := FALSE;
    unauthorized_erasure_rejected BOOLEAN := FALSE;
    cloud_audit_mismatch_rejected BOOLEAN := FALSE;
    erased_probe_id TEXT :=
        'schema_probe_erased_' || pg_backend_pid()::TEXT || '_' || txid_current()::TEXT;
    erased_probe_digest TEXT;
    cloud_probe_id TEXT :=
        'schema_cloud_' || pg_backend_pid()::TEXT || '_' || txid_current()::TEXT;
BEGIN
    -- Make catalog deparsing deterministic for callers with a custom
    -- search_path. All canonical persistence objects live in public.
    PERFORM set_config('search_path', 'pg_catalog, public, pg_temp', true);
    postgres_major := current_setting('server_version_num')::INTEGER / 10000;
    expected_constraint_signature := CASE postgres_major
        WHEN 16 THEN '7d8f0c9f7bdd6fe7e2bb9f07d721dd98'
        WHEN 18 THEN 'fade2b6f3356f1fc51d9256312511505'
        ELSE NULL
    END;
    IF expected_constraint_signature IS NULL THEN
        RAISE EXCEPTION
            'unsupported PostgreSQL major for schema fingerprint: %',
            postgres_major;
    END IF;

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
    IF NOT EXISTS (
        SELECT 1 FROM _sqlx_migrations
         WHERE version = 20260717000100
           AND success
           AND encode(checksum, 'hex') =
               'd67991333d4d9e06b5c1c51a9f3c17855bddfbb64558873f4821e7338700981a98fe78f1c05441244c95e5010e156adc'
    ) THEN
        RAISE EXCEPTION 'event delivery/checkpoint migration is missing or checksum-drifted';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM _sqlx_migrations
         WHERE version = 20260721000200 AND success
    ) THEN
        RAISE EXCEPTION 'privacy visibility/encryption migration is not applied';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              (20260723000100::BIGINT, 'd47d65b1cd0d5de540720d9daf8e76baa9148ed792531861c876f59ff9221ad829a05b68306801c2cfd0f1592cbe87dc'),
              (20260723000200::BIGINT, 'd8d3771de8ea914e47f2283a83f07bed3ced0eda8b9ab977d971d793d25198ff92a38a7a4c85810176d6b0d82d1c5eae'),
              (20260723000300::BIGINT, '3651703e59cc0871905e8739e667cc991b60802da71576e74f530763529324b28508d12608fce6e0f06dc0d449ea7bd8'),
              (20260723000400::BIGINT, '0a297ccf3ad17e25eadd1eb9c06f348e73a1c5573f625bd8b9b31f0bf835515f43dc48c02077c4deb467eccef23bd072'),
              (20260723000500::BIGINT, 'cf5871b30c9f5e7bb3e8a3cbc77c397ccc607e9a11df7cb93ca711fe9fbd53eaf7d8e329dc37a8cdf93b0f7744e503e1'),
              (20260723000600::BIGINT, '0be381a3fe07b82f5c67167f07a5c0a6375c11be9acfaaec2f402165bd54439a786579b38be56041468315cd6a50b7d5'),
              (20260723000700::BIGINT, '532d72f2541b94538ac4031164474f9e43cc3a7a7a203ab042699e713f59fa61b6c54c72f56cd0683c892448ac4112b3'),
              (20260723000800::BIGINT, '7ad0e2f71fd914bee579a373e91a5f775f378f8cb3c64f7a84e673e6c3dc23e7c2ad15fe835a7e15f4341a3fa66f77d1'),
              (20260723000900::BIGINT, '9401b667b1119d95256e0aeff580a1713fbb276caccb3c9cf7c3672d702b9be21df54f6db27e99f47ae26336665f24c2'),
              (20260724000100::BIGINT, '9233bdfb7089e673019c93d491d59722fe65e10b6b08d738bb9946ee8d05d75fa71a835d9bef4999e7023c5280af8d5a'),
              (20260724000200::BIGINT, 'e7bd37bdd99cf15971195dc055aab9be6336971c598d8bfde7ecbe4092b60c84ae9142bb8850b72b6b7593f9490f91e4'),
              (20260724000300::BIGINT, 'b0e29db7c453ae5bef7cc8272a814c6b9870b5a236b9cb0d17627c6bbed641230015bc4378537f57f1045637671e60a8'),
              (20260725000100::BIGINT, '8cd03566d1d43e90046a9dcded3985d5958596d153110606a2de0b8f00b2df5a86ba7f8e6b5d5588d247fbc84b145072')
          ) AS expected(version, checksum)
          LEFT JOIN _sqlx_migrations AS applied
            ON applied.version = expected.version
           AND applied.success
           AND encode(applied.checksum, 'hex') = expected.checksum
         WHERE applied.version IS NULL
    ) THEN
        RAISE EXCEPTION 'P05 migration is missing, failed, or checksum-drifted';
    END IF;
    IF EXISTS (SELECT 1 FROM _sqlx_migrations WHERE NOT success) THEN
        RAISE EXCEPTION 'failed SQLx migration ledger row found';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM _sqlx_migrations
         WHERE version = 20260726000100
           AND success
           AND encode(checksum, 'hex') =
               '47774b27008ca0ed39188582d97edc21beb3f31ad739da688e0702854e25504e26169ab864624e584c61777f6691f424'
    ) THEN
        RAISE EXCEPTION 'P06 core-domain migration is missing, failed, or checksum-drifted';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        RAISE EXCEPTION 'pgvector extension is missing';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('trpg_application', FALSE),
              ('trpg_api_service', FALSE),
              ('trpg_canonical_service', FALSE),
              ('trpg_worker_service', FALSE),
              ('trpg_realtime_service', FALSE),
              ('trpg_api_login', TRUE),
              ('trpg_canonical_login', TRUE),
              ('trpg_worker_login', TRUE),
              ('trpg_realtime_login', TRUE)
          ) AS expected(role_name, may_login)
          LEFT JOIN pg_roles AS role ON role.rolname = expected.role_name
         WHERE role.oid IS NULL
            OR role.rolsuper
            OR role.rolcreatedb
            OR role.rolcreaterole
            OR role.rolreplication
            OR role.rolbypassrls
            OR role.rolcanlogin IS DISTINCT FROM expected.may_login
    ) THEN
        RAISE EXCEPTION 'P05 application role topology or privilege flags drifted';
    END IF;
    IF has_schema_privilege('trpg_application', 'public', 'USAGE')
       OR EXISTS (
           SELECT 1
             FROM pg_class AS relation
            WHERE relation.relnamespace = 'public'::regnamespace
              AND relation.relkind IN ('r', 'p', 'S')
              AND (
                  has_table_privilege('trpg_application', relation.oid, 'SELECT')
                  OR has_table_privilege('trpg_application', relation.oid, 'INSERT')
                  OR has_table_privilege('trpg_application', relation.oid, 'UPDATE')
                  OR has_table_privilege('trpg_application', relation.oid, 'DELETE')
                  OR has_table_privilege('trpg_application', relation.oid, 'TRUNCATE')
              )
       )
    THEN
        RAISE EXCEPTION 'legacy aggregate application role retains database authority';
    END IF;
    IF has_table_privilege('trpg_api_service', 'event_store', 'UPDATE')
       OR has_table_privilege('trpg_api_service', 'event_store', 'INSERT')
       OR has_table_privilege('trpg_api_service', 'event_store', 'DELETE')
       OR has_table_privilege('trpg_api_service', 'event_outbox', 'INSERT')
       OR has_table_privilege('trpg_api_service', 'event_outbox', 'UPDATE')
       OR has_table_privilege('trpg_api_service', 'formal_commits', 'INSERT')
       OR has_table_privilege('trpg_api_service', 'formal_commits', 'UPDATE')
       OR has_table_privilege('trpg_api_service', 'canonical_audit_log', 'INSERT')
       OR has_table_privilege('trpg_api_service', 'canonical_audit_log', 'UPDATE')
       OR has_table_privilege('trpg_api_service', 'privacy_erased_subjects', 'INSERT')
       OR has_table_privilege('trpg_api_service', 'privacy_subject_keys', 'UPDATE')
       OR has_table_privilege('trpg_worker_service', 'event_store', 'INSERT')
       OR has_table_privilege('trpg_worker_service', 'formal_commits', 'INSERT')
       OR has_table_privilege('trpg_worker_service', 'canonical_audit_log', 'INSERT')
       OR has_table_privilege('trpg_canonical_service', 'users', 'SELECT')
       OR has_table_privilege('trpg_canonical_service', 'campaign_memberships', 'SELECT')
       OR has_table_privilege('trpg_canonical_service', 'cloud_egress_consents', 'SELECT')
       OR has_table_privilege('trpg_canonical_service', 'privacy_subject_keys', 'UPDATE')
       OR has_table_privilege('trpg_canonical_service', 'privacy_subject_keys', 'DELETE')
       OR EXISTS (
           SELECT 1
             FROM pg_class AS relation
            WHERE relation.relnamespace = 'public'::regnamespace
              AND relation.relkind IN ('r', 'p')
              AND (
                  has_table_privilege('trpg_realtime_service', relation.oid, 'INSERT')
                  OR has_table_privilege('trpg_realtime_service', relation.oid, 'UPDATE')
                  OR has_table_privilege('trpg_realtime_service', relation.oid, 'DELETE')
                  OR has_table_privilege('trpg_realtime_service', relation.oid, 'TRUNCATE')
              )
       )
    THEN
        RAISE EXCEPTION 'service database role crosses its P05 write boundary';
    END IF;
    IF NOT pg_has_role('trpg_api_login', 'trpg_api_service', 'MEMBER')
       OR NOT pg_has_role('trpg_canonical_login', 'trpg_canonical_service', 'MEMBER')
       OR NOT pg_has_role('trpg_worker_login', 'trpg_worker_service', 'MEMBER')
       OR NOT pg_has_role('trpg_realtime_login', 'trpg_realtime_service', 'MEMBER')
       OR EXISTS (
           SELECT 1
             FROM pg_auth_members AS membership
             JOIN pg_roles AS granted_role ON granted_role.oid = membership.roleid
             JOIN pg_roles AS member_role ON member_role.oid = membership.member
            WHERE (
                    granted_role.rolname = ANY (ARRAY[
                        'trpg_application',
                        'trpg_api_service',
                        'trpg_canonical_service',
                        'trpg_worker_service',
                        'trpg_realtime_service',
                        'trpg_api_login',
                        'trpg_canonical_login',
                        'trpg_worker_login',
                        'trpg_realtime_login'
                    ])
                    OR member_role.rolname = ANY (ARRAY[
                        'trpg_application',
                        'trpg_api_service',
                        'trpg_canonical_service',
                        'trpg_worker_service',
                        'trpg_realtime_service',
                        'trpg_api_login',
                        'trpg_canonical_login',
                        'trpg_worker_login',
                        'trpg_realtime_login'
                    ])
                  )
              AND (granted_role.rolname, member_role.rolname) NOT IN (
                  VALUES
                      ('trpg_api_service', 'trpg_api_login'),
                      ('trpg_canonical_service', 'trpg_canonical_login'),
                      ('trpg_worker_service', 'trpg_worker_login'),
                      ('trpg_realtime_service', 'trpg_realtime_login')
              )
       )
    THEN
        RAISE EXCEPTION 'service login role membership crosses a trust boundary';
    END IF;
    IF NOT has_column_privilege(
               'trpg_worker_service', 'privacy_deletion_jobs',
               'lease_expires_at', 'UPDATE'
           )
       OR NOT has_column_privilege(
               'trpg_worker_service', 'privacy_deletion_jobs',
               'lease_recovery_count', 'UPDATE'
           )
       OR NOT has_column_privilege(
               'trpg_worker_service', 'privacy_deletion_jobs',
               'last_lease_expired_at', 'UPDATE'
           )
       OR NOT has_column_privilege(
               'trpg_worker_service', 'privacy_subject_deletion_fences',
               'lease_expires_at', 'UPDATE'
           )
       OR NOT has_column_privilege(
               'trpg_worker_service', 'privacy_deletion_job_targets',
               'progress_cursor', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_deletion_jobs',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_deletion_jobs',
               'lease_recovery_count', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_deletion_jobs',
               'last_lease_expired_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_subject_deletion_fences',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_deletion_job_targets',
               'progress_cursor', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_deletion_jobs',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_deletion_jobs',
               'lease_recovery_count', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_deletion_jobs',
               'last_lease_expired_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_subject_deletion_fences',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_deletion_job_targets',
               'progress_cursor', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_deletion_jobs',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_deletion_jobs',
               'lease_recovery_count', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_deletion_jobs',
               'last_lease_expired_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_subject_deletion_fences',
               'lease_expires_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_deletion_job_targets',
               'progress_cursor', 'UPDATE'
           )
    THEN
        RAISE EXCEPTION 'deletion execution lease authority drifted';
    END IF;
    IF NOT has_column_privilege(
               'trpg_worker_service', 'privacy_deletion_jobs',
               'execution_claim_token', 'UPDATE'
           )
       OR NOT has_column_privilege(
               'trpg_worker_service', 'privacy_subject_deletion_fences',
               'execution_claim_token', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_api_service', 'privacy_deletion_jobs',
               'execution_claim_token', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_canonical_service', 'privacy_deletion_jobs',
               'execution_claim_token', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_realtime_service', 'privacy_deletion_jobs',
               'execution_claim_token', 'UPDATE'
           )
       OR has_table_privilege('trpg_worker_service', 'users', 'UPDATE')
       OR has_column_privilege(
               'trpg_worker_service', 'campaign_memberships',
               'revoked_at', 'UPDATE'
           )
       OR has_column_privilege(
               'trpg_worker_service', 'campaign_group_memberships',
               'revoked_at', 'UPDATE'
           )
       OR has_table_privilege('trpg_worker_service', 'sessions', 'DELETE')
       OR has_table_privilege(
               'trpg_worker_service', 'rag_snapshot_chunk', 'DELETE'
           )
       OR has_table_privilege(
               'trpg_worker_service',
               'privacy_deletion_surface_records',
               'DELETE'
           )
       OR NOT has_function_privilege(
               'trpg_worker_service',
               'erase_privacy_database_subject(text,text,text)',
               'EXECUTE'
           )
       OR NOT has_function_privilege(
               'trpg_worker_service',
               'erase_privacy_rag_subject(text,text,text)',
               'EXECUTE'
           )
       OR NOT has_function_privilege(
               'trpg_worker_service',
               'begin_privacy_deletion_revalidation(text,text)',
               'EXECUTE'
           )
       OR NOT has_function_privilege(
               'trpg_worker_service',
               'record_privacy_deletion_revalidation_result(text,text,text,text,text,text,text,text)',
               'EXECUTE'
           )
       OR has_function_privilege(
               'trpg_worker_service',
               'require_privacy_deletion_claim(text,text,text)',
               'EXECUTE'
           )
       OR EXISTS (
           SELECT 1
             FROM pg_proc AS procedure
             CROSS JOIN LATERAL pg_catalog.aclexplode(
                 COALESCE(
                     procedure.proacl,
                     pg_catalog.acldefault('f', procedure.proowner)
                 )
             ) AS privilege
            WHERE procedure.oid = ANY (ARRAY[
                'erase_privacy_database_subject(text,text,text)'::regprocedure,
                'erase_privacy_rag_subject(text,text,text)'::regprocedure,
                'begin_privacy_deletion_revalidation(text,text)'::regprocedure,
                'record_privacy_deletion_revalidation_result(text,text,text,text,text,text,text,text)'::regprocedure,
                'require_privacy_deletion_claim(text,text,text)'::regprocedure
            ])
              AND privilege.grantee = 0
              AND privilege.privilege_type = 'EXECUTE'
       )
    THEN
        RAISE EXCEPTION
            'privacy deletion claim/function least-privilege authority drifted';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
            ('require_privacy_deletion_claim',
                'p_job_id text, p_subject_id text, p_claim_token text'),
            ('erase_privacy_database_subject',
                'p_job_id text, p_subject_id text, p_claim_token text'),
            ('erase_privacy_rag_subject',
                'p_job_id text, p_subject_id text, p_claim_token text'),
            ('begin_privacy_deletion_revalidation',
                'p_job_id text, p_subject_id text'),
            ('record_privacy_deletion_revalidation_result',
                'p_run_id text, p_job_id text, p_subject_id text, p_claim_token text, p_result_status text, p_failure_target text, p_error_code text, p_evidence_hash text')
          ) AS expected(function_name, identity_arguments)
          LEFT JOIN pg_proc AS procedure
            ON procedure.pronamespace = 'public'::regnamespace
           AND procedure.proname = expected.function_name
           AND pg_get_function_identity_arguments(procedure.oid) =
               expected.identity_arguments
           AND procedure.prosecdef
           AND procedure.proowner =
               (SELECT relowner FROM pg_class
                 WHERE oid = 'privacy_deletion_jobs'::regclass)
           AND procedure.provolatile = 'v'
           AND procedure.prokind = 'f'
           AND COALESCE(
                 procedure.proconfig @>
                     ARRAY['search_path=pg_catalog, public']::TEXT[],
                 FALSE
               )
         WHERE procedure.oid IS NULL
    ) THEN
        RAISE EXCEPTION
            'privacy deletion constrained function execution properties drifted';
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
        'integrity_status:text:NO:-', 'payload_integrity_source:text:NO:-',
        'authenticated_actor_role:text:NO:-',
        'authenticated_actor_origin:jsonb:NO:-',
        'payload_ciphertext:bytea:YES:-',
        'payload_key_reference:text:YES:-', 'payload_nonce:bytea:YES:-',
        'data_subject_id:text:NO:''not_applicable''::text',
        'derived_source_event_sequence:int8:YES:-',
        'derived_snapshot_id:text:YES:-', 'derived_chunk_id:text:YES:-',
        'derived_content_hash:text:YES:-', 'deletion_job_id:text:YES:-',
        'deletion_subject_id:text:YES:-', 'deletion_requested_by:text:YES:-',
        'deletion_retention_policy:text:YES:-',
        'derived_source_type:text:YES:-',
        'derived_copyright_status:text:YES:-',
        'derived_allowed_use:text:YES:-',
        'derived_embedding_model:text:YES:-',
        'derived_embedding_dimensions:int4:YES:-',
        'derived_embedding_hash:text:YES:-',
        'event_integrity_version:int4:NO:3',
        'projection_targets:jsonb:NO:''[]''::jsonb'
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
     WHERE table_schema = 'public' AND table_name = 'rag_snapshot_chunk';
    IF actual_columns IS NULL OR actual_columns <> ARRAY[
        'campaign_id:text:NO:-', 'snapshot_id:text:NO:-', 'chunk_id:text:NO:-',
        'source_event_sequence:int8:NO:-', 'source_type:text:NO:-',
        'visibility:text:NO:-', 'visibility_subject:text:NO:-',
        'copyright_status:text:NO:-', 'version:int8:NO:-',
        'owner:text:NO:-', 'allowed_use:text:NO:-',
        'fact_provenance:jsonb:NO:-', 'chunk_hash:text:NO:-',
        'content:text:NO:-', 'embedding_model:text:NO:-',
        'embedding_dimensions:int4:NO:-', 'embedding:vector:NO:-',
        'projected_at:timestamptz:NO:now()',
        'derivation_event_sequence:int8:NO:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'rag_snapshot_chunk columns/types/nullability/defaults drifted: %',
            actual_columns;
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
        'integrity_status:text:NO:-',
        'delivery_status:text:NO:''pending''::text',
        'available_at:timestamptz:NO:now()', 'locked_until:timestamptz:YES:-',
        'claim_token:text:YES:-', 'visibility_subject:text:YES:-',
        'payload_ciphertext:bytea:YES:-',
        'payload_key_reference:text:YES:-', 'payload_nonce:bytea:YES:-',
        'data_subject_id:text:NO:''not_applicable''::text'
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
        'request_hash_source:text:NO:-', 'integrity_status:text:NO:-',
        'delivery_status:text:NO:''pending''::text',
        'available_at:timestamptz:NO:now()', 'locked_until:timestamptz:YES:-',
        'claim_token:text:YES:-', 'visibility_subject:text:YES:-',
        'payload_ciphertext:bytea:YES:-',
        'payload_key_reference:text:YES:-', 'payload_nonce:bytea:YES:-',
        'data_subject_id:text:NO:''not_applicable''::text'
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
     WHERE table_schema = 'public' AND table_name = 'canonical_event_projection';
    IF actual_columns IS NULL OR actual_columns <> ARRAY[
        'projection_name:text:NO:-', 'campaign_id:text:NO:-',
        'stream_id:text:NO:-', 'stream_version:int8:NO:-',
        'event_sequence:int8:NO:-', 'projection_hash:text:NO:-',
        'event_document:jsonb:NO:-', 'projected_at:timestamptz:NO:now()'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'canonical_event_projection columns/types/nullability/defaults drifted: %',
            actual_columns;
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
        'integrity_version:int4:NO:3', 'correlation_id:text:NO:-',
        'causation_id:text:NO:-'
    ]::TEXT[] THEN
        RAISE EXCEPTION 'canonical_audit_log columns/types/nullability/defaults drifted: %',
            actual_columns;
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
            ('privacy_deletion_jobs', ARRAY[
                'job_id:text:NO:-', 'subject_id:text:NO:-', 'requested_by:text:NO:-',
                'retention_policy:text:NO:-', 'status:text:NO:-',
                'failure_code:text:YES:-', 'created_at:timestamptz:NO:now()',
                'updated_at:timestamptz:NO:now()',
                'evidence_status:text:NO:''pending''::text', 'command_id:text:YES:-',
                'correlation_id:text:YES:-', 'causation_id:text:YES:-',
                'canonical_event_type:text:YES:-',
                'canonical_event_sequence:int8:YES:-',
                'canonical_event_integrity_hash:text:YES:-',
                'campaign_id:text:NO:-',
                'lease_expires_at:timestamptz:YES:-',
                'lease_recovery_count:int8:NO:0',
                'last_lease_expired_at:timestamptz:YES:-',
                'execution_claim_token:text:YES:-'
            ]::TEXT[]),
            ('privacy_deletion_job_targets', ARRAY[
                'job_id:text:NO:-', 'target:text:NO:-', 'status:text:NO:-',
                'error_code:text:YES:-', 'deleted_at:timestamptz:YES:-',
                'verified_at:timestamptz:YES:-',
                'progress_cursor:int8:NO:1'
            ]::TEXT[]),
            ('privacy_legal_holds', ARRAY[
                'subject_id:text:NO:-', 'hold_reference:text:NO:-',
                'active:bool:NO:-', 'updated_at:timestamptz:NO:now()'
            ]::TEXT[]),
            ('privacy_subject_keys', ARRAY[
                'subject_id:text:NO:-', 'key_reference:text:NO:-',
                'wrapped_key:bytea:YES:-', 'destroyed_at:timestamptz:YES:-'
            ]::TEXT[]),
            ('privacy_subject_deletion_fences', ARRAY[
                'subject_id:text:NO:-', 'job_id:text:NO:-', 'status:text:NO:-',
                'started_at:timestamptz:NO:now()', 'updated_at:timestamptz:NO:now()',
                'lease_expires_at:timestamptz:YES:-',
                'execution_claim_token:text:YES:-'
            ]::TEXT[]),
            ('privacy_deletion_revalidation_runs', ARRAY[
                'run_id:text:NO:-', 'job_id:text:NO:-', 'subject_id:text:NO:-',
                'claim_token_hash:text:NO:-', 'completion_evidence_hash:text:NO:-',
                'started_at:timestamptz:NO:statement_timestamp()',
                'lease_expires_at:timestamptz:NO:-'
            ]::TEXT[]),
            ('privacy_deletion_revalidation_results', ARRAY[
                'result_id:text:NO:-', 'run_id:text:NO:-',
                'result_status:text:NO:-', 'failure_target:text:YES:-',
                'error_code:text:YES:-', 'evidence_hash:text:NO:-',
                'alert_status:text:NO:-',
                'recorded_at:timestamptz:NO:statement_timestamp()'
            ]::TEXT[]),
            ('privacy_erased_subjects', ARRAY[
                'subject_id:text:NO:-', 'erasure_digest:text:NO:-',
                'erased_at:timestamptz:NO:now()'
            ]::TEXT[]),
            ('cloud_egress_consents', ARRAY[
                'consent_id:text:NO:-', 'subject_id:text:NO:-',
                'target_provider:text:NO:-', 'purpose:text:NO:-',
                'policy_version:text:NO:-', 'visibility_scope:text:NO:-',
                'granted:bool:NO:-', 'expires_at_unix_ms:int8:NO:-',
                'created_at:timestamptz:NO:now()', 'updated_at:timestamptz:NO:now()'
            ]::TEXT[]),
            ('cloud_egress_route_snapshots', ARRAY[
                'snapshot_id:text:NO:-', 'subject_id:text:NO:-',
                'consent_id:text:YES:-', 'source_provider:text:NO:-',
                'target_provider:text:NO:-', 'purpose:text:NO:-',
                'policy_version:text:NO:-', 'notice_reference:text:YES:-',
                'context_manifest_hash:text:NO:-', 'allowed_fact_ids:jsonb:NO:-',
                'decision:text:NO:-', 'denial_code:text:YES:-',
                'created_at_unix_ms:int8:NO:-', 'source_endpoint:text:NO:-',
                'target_endpoint:text:NO:-', 'model_id:text:NO:-',
                'source_credential_id:text:NO:-',
                'source_credential_version:int8:NO:-',
                'target_credential_id:text:NO:-',
                'target_credential_version:int8:NO:-',
                'fallback_policy:text:NO:-', 'privacy_boundary:text:NO:-',
                'consent_expires_at_unix_ms:int8:YES:-'
            ]::TEXT[]),
            ('cloud_egress_audit', ARRAY[
                'audit_id:text:NO:-', 'snapshot_id:text:NO:-', 'subject_id:text:NO:-',
                'decision:text:NO:-', 'denial_code:text:YES:-',
                'context_manifest_hash:text:NO:-', 'created_at_unix_ms:int8:NO:-',
                'source_provider:text:NO:-', 'target_provider:text:NO:-',
                'source_endpoint:text:NO:-', 'target_endpoint:text:NO:-',
                'model_id:text:NO:-', 'source_credential_id:text:NO:-',
                'source_credential_version:int8:NO:-',
                'target_credential_id:text:NO:-',
                'target_credential_version:int8:NO:-',
                'fallback_policy:text:NO:-', 'privacy_boundary:text:NO:-'
            ]::TEXT[]),
            ('cloud_egress_notices', ARRAY[
                'notice_reference:text:NO:-', 'subject_id:text:NO:-',
                'policy_version:text:NO:-', 'notice_digest:text:NO:-',
                'recorded_at:timestamptz:NO:now()'
            ]::TEXT[])
          ) AS expected(table_name, column_signature)
          LEFT JOIN LATERAL (
              SELECT array_agg(
                         column_name || ':' || udt_name || ':' || is_nullable || ':' ||
                         COALESCE(column_default, '-') ORDER BY ordinal_position
                     ) AS column_signature
                FROM information_schema.columns
               WHERE table_schema = 'public'
                 AND table_name = expected.table_name
          ) AS actual ON TRUE
         WHERE actual.column_signature IS DISTINCT FROM expected.column_signature
    ) THEN
        RAISE EXCEPTION 'P05 privacy/cloud table columns drifted';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
            ('campaign_groups', 'campaign_groups_truncate_guard'),
            ('campaign_group_memberships', 'campaign_group_memberships_truncate_guard'),
            ('cloud_egress_consents', 'cloud_egress_consent_transition_guard'),
            ('cloud_egress_consents', 'cloud_egress_consents_truncate_guard'),
            ('cloud_egress_route_snapshots', 'cloud_egress_route_snapshot_append_only'),
            ('cloud_egress_route_snapshots', 'cloud_egress_route_snapshots_truncate_guard'),
            ('cloud_egress_audit', 'cloud_egress_audit_append_only'),
            ('cloud_egress_audit', 'cloud_egress_route_audit_binding'),
            ('cloud_egress_audit', 'cloud_egress_audit_truncate_guard'),
            ('cloud_egress_notices', 'cloud_egress_notices_append_only'),
            ('cloud_egress_notices', 'cloud_egress_notices_truncate_guard'),
            ('privacy_deletion_jobs', 'privacy_deletion_job_evidence_guard'),
            ('privacy_deletion_jobs', 'privacy_deletion_jobs_delete_guard'),
            ('privacy_deletion_job_targets', 'privacy_deletion_target_transition_guard'),
            ('privacy_deletion_job_targets', 'privacy_deletion_targets_delete_guard'),
            ('privacy_subject_deletion_fences', 'privacy_deletion_fences_delete_guard'),
            ('privacy_deletion_revalidation_runs',
                'privacy_deletion_revalidation_runs_immutable'),
            ('privacy_deletion_revalidation_results',
                'privacy_deletion_revalidation_results_immutable'),
            ('privacy_erased_subjects', 'privacy_erased_subjects_mutation_guard'),
            ('privacy_subject_keys', 'privacy_subject_key_destruction_guard'),
            ('privacy_subject_keys', 'privacy_subject_keys_delete_guard'),
            ('privacy_legal_holds', 'privacy_legal_holds_delete_guard'),
            ('event_store', 'event_store_subject_protection_guard'),
            ('event_outbox', 'event_outbox_subject_protection_guard'),
            ('users', 'users_erasure_guard'),
            ('sessions', 'sessions_erasure_guard'),
            ('campaign_memberships', 'campaign_memberships_erasure_guard'),
            ('campaign_group_memberships', 'campaign_group_memberships_erasure_guard')
          ) AS expected(table_name, trigger_name)
          LEFT JOIN pg_trigger AS trigger
            ON trigger.tgrelid = to_regclass('public.' || expected.table_name)
           AND trigger.tgname = expected.trigger_name
           AND trigger.tgenabled = 'O'
           AND NOT trigger.tgisinternal
         WHERE trigger.oid IS NULL
    ) THEN
        RAISE EXCEPTION 'P05 security/privacy trigger is missing or disabled';
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname IN (
             'cloud_egress_consents_no_truncate',
             'cloud_egress_route_snapshots_no_truncate',
             'cloud_egress_audit_no_truncate',
             'cloud_egress_notices_no_truncate'
         )
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'superseded cloud-egress truncate trigger remains installed';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM (VALUES
            ('prevent_unverified_outbox_publish'),
            ('prevent_unverified_rag_source'),
            ('enforce_cloud_egress_route_audit_binding'),
            ('enforce_privacy_deletion_job_evidence'),
            ('enforce_privacy_deletion_target_transition'),
            ('require_running_deletion_authority'),
            ('enforce_subject_scoped_event_protection'),
            ('enforce_subject_scoped_outbox_protection'),
            ('prevent_destroyed_subject_key_restoration'),
            ('reject_erased_user_reactivation'),
            ('reject_erased_subject_session'),
            ('reject_erased_subject_membership'),
            ('reject_retained_security_history_truncate'),
            ('reject_privacy_evidence_removal'),
            ('reject_privacy_revalidation_mutation')
          ) AS expected(function_name)
          LEFT JOIN pg_proc AS procedure
            ON procedure.pronamespace = 'public'::regnamespace
           AND procedure.proname = expected.function_name
           AND pg_get_function_identity_arguments(procedure.oid) = ''
           AND COALESCE(
                 procedure.proconfig @> ARRAY['search_path=pg_catalog, public']::TEXT[],
                 FALSE
               )
         WHERE procedure.oid IS NULL
    ) THEN
        RAISE EXCEPTION 'P05 trigger function is missing or has unsafe search_path';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_class AS event_store_relation
            ON event_store_relation.oid = 'event_store'::regclass
         WHERE procedure.pronamespace = 'public'::regnamespace
           AND procedure.proname IN (
               'prevent_unverified_outbox_publish',
               'prevent_unverified_rag_source',
               'enforce_cloud_egress_route_audit_binding',
               'enforce_privacy_deletion_job_evidence',
               'enforce_privacy_deletion_target_transition',
               'require_running_deletion_authority',
               'enforce_subject_scoped_event_protection',
               'enforce_subject_scoped_outbox_protection',
               'prevent_destroyed_subject_key_restoration',
               'reject_erased_user_reactivation',
               'reject_erased_subject_session',
               'reject_erased_subject_membership',
               'reject_retained_security_history_truncate',
               'reject_privacy_evidence_removal',
               'reject_privacy_revalidation_mutation'
           )
           AND (
               procedure.prosecdef
               OR procedure.proowner <> event_store_relation.relowner
               OR procedure.provolatile <> 'v'
               OR procedure.prokind <> 'f'
           )
    ) THEN
        RAISE EXCEPTION 'P05 trigger function execution properties drifted';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM unnest(ARRAY[
              '%NEW.subject_id IS DISTINCT FROM route.subject_id%',
              '%NEW.source_provider IS DISTINCT FROM route.source_provider%',
              '%NEW.target_provider IS DISTINCT FROM route.target_provider%',
              '%NEW.source_endpoint IS DISTINCT FROM route.source_endpoint%',
              '%NEW.target_endpoint IS DISTINCT FROM route.target_endpoint%',
              '%NEW.model_id IS DISTINCT FROM route.model_id%',
              '%NEW.source_credential_id IS DISTINCT FROM route.source_credential_id%',
              '%NEW.source_credential_version IS DISTINCT FROM route.source_credential_version%',
              '%NEW.target_credential_id IS DISTINCT FROM route.target_credential_id%',
              '%NEW.target_credential_version IS DISTINCT FROM route.target_credential_version%',
              '%NEW.fallback_policy IS DISTINCT FROM route.fallback_policy%',
              '%NEW.privacy_boundary IS DISTINCT FROM route.privacy_boundary%',
              '%NEW.context_manifest_hash IS DISTINCT FROM route.context_manifest_hash%',
              '%NEW.created_at_unix_ms IS DISTINCT FROM route.created_at_unix_ms%'
          ]::TEXT[]) AS expected(body_fragment)
         WHERE pg_get_functiondef(
                   'enforce_cloud_egress_route_audit_binding()'::regprocedure
               ) NOT LIKE expected.body_fragment
    )
    THEN
        RAISE EXCEPTION 'cloud-egress trigger body no longer binds the exact route tuple';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'event_store'::regclass
           AND conname = 'event_store_rag_derivation_fields_check'
           AND pg_get_constraintdef(oid) LIKE '%derived_source_event_sequence IS NOT NULL%'
           AND pg_get_constraintdef(oid) LIKE '%derived_content_hash IS NOT NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'event_store'::regclass
           AND conname = 'event_store_deletion_request_fields_check'
           AND pg_get_constraintdef(oid) LIKE '%deletion_job_id IS NOT NULL%'
           AND pg_get_constraintdef(oid) LIKE '%deletion_retention_policy IS NOT NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_deletion_jobs'::regclass
           AND conname = 'privacy_deletion_jobs_evidence_binding_check'
           AND pg_get_constraintdef(oid) LIKE '%hmac-sha256:%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_deletion_jobs'::regclass
           AND conname = 'privacy_deletion_jobs_live_lease_check'
           AND pg_get_constraintdef(oid) LIKE '%lease_expires_at IS NOT NULL%'
           AND pg_get_constraintdef(oid) LIKE '%lease_expires_at IS NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_deletion_jobs'::regclass
           AND conname = 'privacy_deletion_jobs_lease_recovery_check'
           AND pg_get_constraintdef(oid) LIKE '%lease_recovery_count >= 0%'
           AND pg_get_constraintdef(oid) LIKE '%lease_recovery_count <= 3%'
           AND pg_get_constraintdef(oid) LIKE '%last_lease_expired_at IS NOT NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_subject_keys'::regclass
           AND conname = 'privacy_subject_keys_destroyed_material_check'
           AND pg_get_constraintdef(oid) LIKE '%destroyed_at IS NULL%'
           AND pg_get_constraintdef(oid) LIKE '%wrapped_key IS NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_subject_deletion_fences'::regclass
           AND conname = 'privacy_deletion_fences_live_lease_check'
           AND pg_get_constraintdef(oid) LIKE '%lease_expires_at IS NOT NULL%'
           AND pg_get_constraintdef(oid) LIKE '%lease_expires_at IS NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_deletion_jobs'::regclass
           AND conname = 'privacy_deletion_jobs_execution_claim_check'
           AND pg_get_constraintdef(oid) LIKE
               '%btrim(COALESCE(execution_claim_token%'
           AND pg_get_constraintdef(oid) LIKE
               '%execution_claim_token IS NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_subject_deletion_fences'::regclass
           AND conname = 'privacy_deletion_fences_execution_claim_check'
           AND pg_get_constraintdef(oid) LIKE
               '%btrim(COALESCE(execution_claim_token%'
           AND pg_get_constraintdef(oid) LIKE
               '%execution_claim_token IS NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid =
               'privacy_deletion_revalidation_runs'::regclass
           AND conname =
               'privacy_deletion_revalidation_runs_claim_token_hash_check'
           AND pg_get_constraintdef(oid) LIKE
               '%claim_token_hash ~ ''^[0-9a-f]{64}$''%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid =
               'privacy_deletion_revalidation_results'::regclass
           AND conname = 'privacy_deletion_revalidation_results_check'
           AND pg_get_constraintdef(oid) LIKE
               '%pending_acknowledgement%'
           AND pg_get_constraintdef(oid) LIKE
               '%failure_target IS NOT NULL%'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'privacy_deletion_job_targets'::regclass
           AND conname = 'privacy_deletion_target_progress_cursor_check'
           AND pg_get_constraintdef(oid) LIKE '%progress_cursor > 0%'
    ) THEN
        RAISE EXCEPTION
            'P05 nullable security metadata, HMAC evidence, or deletion lease constraint drifted';
    END IF;

    -- Execute the erased-subject guard, rather than accepting a function that
    -- merely preserves the expected name and search_path. The enclosing
    -- assertion transaction is rolled back, so this probe cannot add retained
    -- production evidence.
    erased_probe_digest :=
        'sha256:' || md5(erased_probe_id) || md5(erased_probe_id || ':erasure');
    INSERT INTO users (
        user_id, login_normalized, password_hash, global_role, disabled_at
    ) VALUES (
        erased_probe_id,
        'deleted_' || erased_probe_digest,
        'DELETED_ACCOUNT_NO_LOGIN_' || erased_probe_digest,
        'USER',
        now()
    );
    BEGIN
        INSERT INTO privacy_erased_subjects (
            subject_id, erasure_digest
        ) VALUES (
            erased_probe_id, erased_probe_digest
        );
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM =
           'privacy erasure mutation requires a live confirmed deletion claim' THEN
            unauthorized_erasure_rejected := TRUE;
        ELSE
            RAISE EXCEPTION
                'erasure-authority behavior probe returned an unexpected error: %',
                SQLERRM;
        END IF;
    END;
    IF NOT unauthorized_erasure_rejected THEN
        RAISE EXCEPTION
            'erasure-authority behavior guard accepted an unauthorised tombstone';
    END IF;

    -- Seed only the downstream reactivation probe. This owner-only schema
    -- assertion runs inside a transaction that is always rolled back; the
    -- trigger is re-enabled before the behavior test and fingerprint checks.
    ALTER TABLE privacy_erased_subjects
        DISABLE TRIGGER privacy_erased_subjects_creation_authority;
    INSERT INTO privacy_erased_subjects (
        subject_id, erasure_digest
    ) VALUES (
        erased_probe_id, erased_probe_digest
    );
    ALTER TABLE privacy_erased_subjects
        ENABLE TRIGGER privacy_erased_subjects_creation_authority;
    BEGIN
        UPDATE users
           SET login_normalized = 'reactivated-' || erased_probe_id || '@example.test',
               password_hash = 'active-password',
               disabled_at = NULL
         WHERE user_id = erased_probe_id;
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM = 'erased user cannot be reactivated' THEN
            erased_user_reactivation_rejected := TRUE;
        ELSE
            RAISE EXCEPTION
                'erased-user behavior probe returned an unexpected error: %',
                SQLERRM;
        END IF;
    END;
    IF NOT erased_user_reactivation_rejected THEN
        RAISE EXCEPTION 'erased-user behavior guard accepted reactivation';
    END IF;

    -- Execute the route/audit binding with a deliberately substituted model.
    -- A matching audit is inserted after the rejection so the final
    -- consistency scan also proves the positive path for this probe.
    INSERT INTO cloud_egress_route_snapshots (
        snapshot_id, subject_id, consent_id, source_provider, target_provider,
        purpose, policy_version, notice_reference, context_manifest_hash,
        allowed_fact_ids, decision, denial_code, created_at_unix_ms,
        source_endpoint, target_endpoint, model_id,
        source_credential_id, source_credential_version,
        target_credential_id, target_credential_version,
        fallback_policy, privacy_boundary, consent_expires_at_unix_ms
    ) VALUES (
        cloud_probe_id, 'schema_subject', NULL, 'ollama', 'cloud_provider',
        'schema_behavior_probe', 'schema_policy_v1', NULL,
        repeat('a', 64), '[]'::jsonb, 'deny', 'schema_probe_denied', 1,
        'http://127.0.0.1:11434/v1', 'https://provider.example.test/v1',
        'schema-model', 'local-credential', 1, 'cloud-credential', 1,
        'explicit_audited_only', 'explicit_consent_no_silent_fallback', NULL
    );
    BEGIN
        INSERT INTO cloud_egress_audit (
            audit_id, snapshot_id, subject_id, decision, denial_code,
            context_manifest_hash, created_at_unix_ms,
            source_provider, target_provider, source_endpoint, target_endpoint,
            model_id, source_credential_id, source_credential_version,
            target_credential_id, target_credential_version,
            fallback_policy, privacy_boundary
        ) VALUES (
            cloud_probe_id || '_forged', cloud_probe_id, 'schema_subject',
            'deny', 'schema_probe_denied', repeat('a', 64), 1,
            'ollama', 'cloud_provider', 'http://127.0.0.1:11434/v1',
            'https://provider.example.test/v1', 'substituted-model',
            'local-credential', 1, 'cloud-credential', 1,
            'explicit_audited_only', 'explicit_consent_no_silent_fallback'
        );
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM = 'cloud egress audit does not match route snapshot' THEN
            cloud_audit_mismatch_rejected := TRUE;
        ELSE
            RAISE EXCEPTION
                'cloud route/audit behavior probe returned an unexpected error: %',
                SQLERRM;
        END IF;
    END;
    IF NOT cloud_audit_mismatch_rejected THEN
        RAISE EXCEPTION 'cloud route/audit behavior probe accepted a substituted model';
    END IF;
    INSERT INTO cloud_egress_audit (
        audit_id, snapshot_id, subject_id, decision, denial_code,
        context_manifest_hash, created_at_unix_ms,
        source_provider, target_provider, source_endpoint, target_endpoint,
        model_id, source_credential_id, source_credential_version,
        target_credential_id, target_credential_version,
        fallback_policy, privacy_boundary
    ) VALUES (
        cloud_probe_id || '_accepted', cloud_probe_id, 'schema_subject',
        'deny', 'schema_probe_denied', repeat('a', 64), 1,
        'ollama', 'cloud_provider', 'http://127.0.0.1:11434/v1',
        'https://provider.example.test/v1', 'schema-model',
        'local-credential', 1, 'cloud-credential', 1,
        'explicit_audited_only', 'explicit_consent_no_silent_fallback'
    );

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
           'projection_checkpoint'::regclass,
           'canonical_event_projection'::regclass, 'formal_commits'::regclass,
           'canonical_audit_log'::regclass, 'rag_snapshot_chunk'::regclass
       );
    IF constraint_signature IS NULL
       OR constraint_signature <> expected_constraint_signature THEN
        RAISE EXCEPTION
            'event persistence constraint relation/definition signature drifted on PostgreSQL %: %',
            postgres_major, constraint_signature;
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
           'projection_checkpoint'::regclass, 'formal_commits'::regclass,
           'canonical_audit_log'::regclass,
           'canonical_event_projection'::regclass,
           'rag_snapshot_chunk'::regclass
       );
    IF trigger_signature IS NULL
       OR trigger_signature <> 'd8bc92078fbcad42090f4d4c3dcd8360' THEN
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
         'reject_historical_classification_insert()'::regprocedure,
         'enforce_projection_checkpoint_monotonicity()'::regprocedure,
         'enforce_canonical_event_projection_document()'::regprocedure,
         'enforce_rag_snapshot_chunk_source()'::regprocedure,
         'enforce_campaign_fork_empty_child_history()'::regprocedure,
         'lock_rag_snapshot(text,text)'::regprocedure,
         'canonical_projection_json(jsonb)'::regprocedure,
         'projection_hash_field(integer,bytea)'::regprocedure,
         'compute_canonical_projection_hash_v3(text,event_store)'::regprocedure
    );
    IF trigger_function_signature IS NULL
       OR trigger_function_signature <> '2b4abc44080f278d09f382cd795cad32' THEN
        RAISE EXCEPTION 'event persistence trigger function definition/execution signature drifted: %',
            trigger_function_signature;
    END IF;

    IF EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
         WHERE procedure.oid IN (
                   'enforce_event_outbox_binding()'::regprocedure,
                   'enforce_projection_checkpoint_monotonicity()'::regprocedure,
                   'enforce_canonical_audit_chain()'::regprocedure,
                   'enforce_canonical_event_projection_document()'::regprocedure,
                   'enforce_rag_snapshot_chunk_source()'::regprocedure,
                   'enforce_campaign_fork_empty_child_history()'::regprocedure,
                   'lock_rag_snapshot(text,text)'::regprocedure,
                   'canonical_projection_json(jsonb)'::regprocedure,
                   'projection_hash_field(integer,bytea)'::regprocedure,
                   'compute_canonical_projection_hash_v3(text,event_store)'::regprocedure
               )
           AND NOT COALESCE(
                   procedure.proconfig @> ARRAY['search_path=pg_catalog, public']::TEXT[],
                   FALSE
               )
    ) THEN
        RAISE EXCEPTION 'P04 canonical lookup function has an unsafe execution search_path';
    END IF;

    -- Prove the checkpoint trigger cannot be redirected to a session-local
    -- event_store lookalike. A catalog signature alone would not prove this.
    CREATE TEMP TABLE event_store (
        campaign_id TEXT NOT NULL,
        stream_id TEXT NOT NULL,
        stream_version BIGINT NOT NULL,
        sequence BIGINT NOT NULL
    ) ON COMMIT DROP;
    INSERT INTO pg_temp.event_store VALUES (
        'schema_shadow_campaign', 'schema_shadow_stream',
        9223372036854775806, 9223372036854775806
    );
    PERFORM set_config('search_path', 'pg_temp, public', true);
    BEGIN
        INSERT INTO public.projection_checkpoint (
            projection_name, campaign_id, stream_id, version,
            last_event_sequence, projection_hash
        ) VALUES (
            'schema_search_path_probe', 'schema_shadow_campaign',
            'schema_shadow_stream', 9223372036854775806,
            9223372036854775806,
            'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        );
    EXCEPTION WHEN OTHERS THEN
        IF SQLERRM =
           'projection checkpoint does not reference its canonical stream event' THEN
            search_path_bypass_rejected := TRUE;
        ELSE
            RAISE EXCEPTION 'checkpoint search_path probe returned an unexpected error: %',
                SQLERRM;
        END IF;
    END;
    IF NOT search_path_bypass_rejected THEN
        RAISE EXCEPTION 'checkpoint search_path bypass was accepted';
    END IF;
    DROP TABLE pg_temp.event_store;
    PERFORM set_config('search_path', 'pg_catalog, public, pg_temp', true);

    IF NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'event_outbox'
           AND indexname = 'event_outbox_claim_ready_idx'
           AND indexdef =
               'CREATE INDEX event_outbox_claim_ready_idx ON public.event_outbox USING btree (available_at, locked_until, outbox_id) WHERE ((published_at IS NULL) AND (dead_lettered_at IS NULL))'
    ) THEN
        RAISE EXCEPTION 'event outbox claim-ready index drifted';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'privacy_deletion_jobs'
           AND indexname = 'privacy_deletion_jobs_expired_lease_idx'
           AND indexdef =
               'CREATE INDEX privacy_deletion_jobs_expired_lease_idx ON public.privacy_deletion_jobs USING btree (lease_expires_at, job_id) WHERE (status = ANY (ARRAY[''running''::text, ''verifying''::text]))'
    ) THEN
        RAISE EXCEPTION 'privacy deletion expired-lease recovery index drifted';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'privacy_deletion_revalidation_runs'
           AND indexname = 'privacy_deletion_revalidation_runs_job_idx'
           AND indexdef =
               'CREATE INDEX privacy_deletion_revalidation_runs_job_idx ON public.privacy_deletion_revalidation_runs USING btree (job_id, started_at DESC, run_id)'
    ) THEN
        RAISE EXCEPTION 'privacy deletion revalidation query index drifted';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'rag_snapshot_chunk'
           AND indexname = 'rag_snapshot_chunk_visibility_idx'
           AND indexdef =
               'CREATE INDEX rag_snapshot_chunk_visibility_idx ON public.rag_snapshot_chunk USING btree (campaign_id, snapshot_id, visibility, visibility_subject)'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'rag_snapshot_chunk'
           AND indexname = 'rag_snapshot_chunk_source_event_idx'
           AND indexdef =
               'CREATE INDEX rag_snapshot_chunk_source_event_idx ON public.rag_snapshot_chunk USING btree (source_event_sequence)'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'rag_snapshot_chunk'
           AND indexname = 'rag_snapshot_chunk_embedding_contract_idx'
           AND indexdef =
               'CREATE INDEX rag_snapshot_chunk_embedding_contract_idx ON public.rag_snapshot_chunk USING btree (campaign_id, snapshot_id, embedding_model, embedding_dimensions)'
    ) THEN
        RAISE EXCEPTION 'RAG snapshot indexes drifted';
    END IF;

    BEGIN
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, authenticated_actor_role,
            authenticated_actor_origin, resource_type, resource_id,
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
            'historical_unknown',
            '{"kind":"workload","role":"historical_unknown"}'::jsonb,
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
            opa_decision_id, opa_policy_revision, trace_id,
            correlation_id, causation_id, event_batch_hash,
            witness_prepare_sequence, witness_prepare_hash, integrity_version,
            integrity_key_id, previous_hash, record_hash
        ) VALUES (
            0, 'schema_historical_audit_probe', 'schema_campaign',
            'schema_actor', 'system_fixture', 'schema_auth', 'campaign',
            'schema_campaign', 'write_official_state', 'human_keeper',
            'party_visible', 'not_applicable', 'system_fixture',
            'schema_assertion', 'schema_assertion', 'PERMIT', 'schema_fga',
            'schema_fga_revision', 'schema_opa', 'schema_opa_revision',
            'schema_trace', 'schema_correlation', 'schema_causation',
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
            opa_decision_id, opa_policy_revision, trace_id,
            correlation_id, causation_id, event_batch_hash,
            witness_prepare_sequence, witness_prepare_hash, occurred_at,
            integrity_version, integrity_key_id, previous_hash, record_hash
        ) VALUES (
            0, 'schema_audit_mutation_probe', 'schema_campaign',
            'schema_actor', 'system_fixture', 'schema_auth', 'campaign',
            'schema_campaign', 'write_official_state', 'human_keeper',
            'party_visible', 'not_applicable', 'system_fixture',
            'schema_assertion', 'schema_assertion', 'PERMIT', 'schema_fga',
            'schema_fga_revision', 'schema_opa', 'schema_opa_revision',
            'schema_trace', 'schema_correlation', 'schema_causation',
            'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            1,
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            '2026-07-16T00:00:00Z'::timestamptz, 3, 'schema_key',
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
        SELECT 1 FROM canonical_audit_log
         WHERE btrim(correlation_id) = '' OR btrim(causation_id) = ''
    ) THEN
        RAISE EXCEPTION 'canonical audit correlation/causation context is incomplete';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM canonical_audit_log AS audit
          JOIN formal_commits AS formal ON formal.audit_sequence = audit.sequence
          JOIN event_store AS event ON event.sequence = formal.first_event_sequence
         WHERE audit.integrity_version = 3
           AND (
               audit.correlation_id IS DISTINCT FROM event.correlation_id
               OR audit.causation_id IS DISTINCT FROM event.causation_id
           )
    ) THEN
        RAISE EXCEPTION 'canonical audit context is not bound to its event batch';
    END IF;

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
            OR outbox.visibility_subject IS DISTINCT FROM event.visibility_subject
            OR outbox.correlation_id IS DISTINCT FROM event.correlation_id
            OR outbox.causation_id IS DISTINCT FROM event.causation_id
            OR outbox.payload_json IS DISTINCT FROM event.payload_json
            OR outbox.payload_ciphertext IS DISTINCT FROM event.payload_ciphertext
            OR outbox.payload_key_reference IS DISTINCT FROM event.payload_key_reference
            OR outbox.payload_nonce IS DISTINCT FROM event.payload_nonce
            OR outbox.data_subject_id IS DISTINCT FROM event.data_subject_id
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

    IF EXISTS (
        SELECT 1
          FROM event_outbox AS outbox
         WHERE (outbox.delivery_status = 'published' OR outbox.published_at IS NOT NULL)
           AND (
               outbox.delivery_status <> 'published'
               OR outbox.published_at IS NULL
               OR outbox.integrity_status <> 'verified_hmac'
               OR outbox.request_hash_source <> 'formal_commit'
               OR outbox.commit_id IS NULL
               OR NOT (outbox.payload_json ? 'protected_payload')
           )
    ) THEN
        RAISE EXCEPTION 'published outbox row bypasses P05 authenticated ciphertext gate';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM privacy_deletion_jobs AS job
          LEFT JOIN event_store AS event
            ON event.sequence = job.canonical_event_sequence
         WHERE job.evidence_status = 'confirmed'
           AND (
               event.sequence IS NULL
               OR event.event_type IS DISTINCT FROM job.canonical_event_type
               OR event.command_id IS DISTINCT FROM job.command_id
               OR event.correlation_id IS DISTINCT FROM job.correlation_id
               OR event.causation_id IS DISTINCT FROM job.causation_id
               OR event.event_integrity_hash IS DISTINCT FROM
                  job.canonical_event_integrity_hash
               OR event.integrity_status IS DISTINCT FROM 'verified_hmac'
               OR event.request_hash_source IS DISTINCT FROM 'formal_commit'
               OR event.deletion_job_id IS DISTINCT FROM job.job_id
               OR event.deletion_subject_id IS DISTINCT FROM job.subject_id
               OR event.deletion_requested_by IS DISTINCT FROM job.requested_by
               OR event.deletion_retention_policy IS DISTINCT FROM job.retention_policy
               OR event.fact_provenance_kind IS DISTINCT FROM 'user_statement'
           )
    ) THEN
        RAISE EXCEPTION 'confirmed deletion job is not bound to its exact canonical HMAC event';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM privacy_deletion_jobs
         WHERE (
                   status IN ('running', 'verifying')
                   AND (
                       lease_expires_at IS NULL
                       OR lease_expires_at <= statement_timestamp()
                       OR btrim(COALESCE(execution_claim_token, '')) = ''
                   )
               )
            OR (
                   status NOT IN ('running', 'verifying')
                   AND (
                       lease_expires_at IS NOT NULL
                       OR execution_claim_token IS NOT NULL
                   )
               )
            OR lease_recovery_count NOT BETWEEN 0 AND 3
            OR (
                   lease_recovery_count = 0
                   AND last_lease_expired_at IS NOT NULL
               )
            OR (
                   lease_recovery_count > 0
                   AND last_lease_expired_at IS NULL
               )
    ) OR EXISTS (
        SELECT 1
          FROM privacy_subject_deletion_fences
         WHERE (
                   status = 'running'
                   AND (
                       lease_expires_at IS NULL
                       OR lease_expires_at <= statement_timestamp()
                       OR btrim(COALESCE(execution_claim_token, '')) = ''
                   )
               )
            OR (
                   status <> 'running'
                   AND (
                       lease_expires_at IS NOT NULL
                       OR execution_claim_token IS NOT NULL
                   )
               )
    ) THEN
        RAISE EXCEPTION 'stale or inconsistent privacy deletion lease remains unrecovered';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM privacy_deletion_revalidation_results AS result
          JOIN privacy_deletion_revalidation_runs AS run
            ON run.run_id = result.run_id
          JOIN privacy_deletion_jobs AS job
            ON job.job_id = run.job_id
           AND job.subject_id = run.subject_id
          JOIN privacy_subject_deletion_fences AS fence
            ON fence.job_id = job.job_id
           AND fence.subject_id = job.subject_id
         WHERE job.status <> 'completed'
            OR fence.status <> 'completed'
            OR (
                result.result_status = 'failed'
                AND result.alert_status <> 'pending_acknowledgement'
            )
            OR (
                result.result_status = 'passed'
                AND result.alert_status <> 'not_required'
            )
    ) THEN
        RAISE EXCEPTION
            'privacy deletion revalidation evidence is not bound to completion/alert state';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM privacy_subject_keys
         WHERE destroyed_at IS NOT NULL
           AND wrapped_key IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'destroyed subject key still retains wrapped key material';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM cloud_egress_route_snapshots AS route
          LEFT JOIN cloud_egress_audit AS audit
            ON audit.snapshot_id = route.snapshot_id
         WHERE audit.audit_id IS NULL
            OR audit.subject_id IS DISTINCT FROM route.subject_id
            OR audit.source_provider IS DISTINCT FROM route.source_provider
            OR audit.target_provider IS DISTINCT FROM route.target_provider
            OR audit.source_endpoint IS DISTINCT FROM route.source_endpoint
            OR audit.target_endpoint IS DISTINCT FROM route.target_endpoint
            OR audit.model_id IS DISTINCT FROM route.model_id
            OR audit.source_credential_id IS DISTINCT FROM route.source_credential_id
            OR audit.source_credential_version IS DISTINCT FROM
               route.source_credential_version
            OR audit.target_credential_id IS DISTINCT FROM route.target_credential_id
            OR audit.target_credential_version IS DISTINCT FROM
               route.target_credential_version
            OR audit.fallback_policy IS DISTINCT FROM route.fallback_policy
            OR audit.privacy_boundary IS DISTINCT FROM route.privacy_boundary
            OR audit.decision IS DISTINCT FROM route.decision
            OR audit.denial_code IS DISTINCT FROM route.denial_code
            OR audit.context_manifest_hash IS DISTINCT FROM route.context_manifest_hash
            OR audit.created_at_unix_ms IS DISTINCT FROM route.created_at_unix_ms
    ) THEN
        RAISE EXCEPTION 'cloud egress route lacks its exact immutable audit record';
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

    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('public', 'campaigns'),
              ('public', 'rooms'),
              ('core_domain', 'sessions'),
              ('public', 'scenes'),
              ('public', 'scenarios'),
              ('public', 'characters'),
              ('public', 'character_sheet_versions'),
              ('public', 'campaign_forks'),
              ('public', 'reconsiderations')
          ) AS expected(schema_name, table_name)
         WHERE to_regclass(
             format('%I.%I', expected.schema_name, expected.table_name)
         ) IS NULL
    ) THEN
        RAISE EXCEPTION 'P06 core-domain table set is incomplete';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = 'sessions'
           AND column_name = 'token_hash'
    ) THEN
        RAISE EXCEPTION 'P06 replaced or damaged the P02 login-session table';
    END IF;
    IF to_regnamespace('core_domain') IS NULL
       OR EXISTS (
           SELECT 1
             FROM pg_namespace AS namespace,
                  LATERAL aclexplode(namespace.nspacl) AS privilege
            WHERE namespace.nspname = 'core_domain'
              AND privilege.grantee = 0
              AND privilege.privilege_type = 'USAGE'
       )
       OR NOT has_schema_privilege(
           'trpg_api_service', 'core_domain', 'USAGE'
       )
       OR NOT has_schema_privilege(
           'trpg_worker_service', 'core_domain', 'USAGE'
       )
    THEN
        RAISE EXCEPTION 'P06 core-domain schema privilege boundary drifted';
    END IF;
    IF has_table_privilege(
           'trpg_canonical_service', 'public.campaigns', 'SELECT'
       )
       OR has_table_privilege(
           'trpg_canonical_service', 'core_domain.sessions', 'SELECT'
       )
       OR NOT has_table_privilege(
           'trpg_api_service', 'public.campaigns', 'INSERT'
       )
       OR NOT has_table_privilege(
           'trpg_api_service', 'public.characters', 'UPDATE'
       )
       OR NOT has_table_privilege(
           'trpg_api_service', 'core_domain.sessions', 'UPDATE'
       )
       OR has_table_privilege(
           'trpg_api_service', 'public.campaigns', 'DELETE'
       )
       OR has_table_privilege(
           'trpg_api_service', 'public.characters', 'DELETE'
       )
       OR has_table_privilege(
           'trpg_api_service', 'core_domain.sessions', 'DELETE'
       )
       OR NOT has_table_privilege(
           'trpg_worker_service', 'public.scenarios', 'SELECT'
       )
       OR NOT has_table_privilege(
           'trpg_worker_service', 'core_domain.sessions', 'SELECT'
       )
    THEN
        RAISE EXCEPTION 'P06 service role privilege boundary drifted';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'core_domain'
           AND tablename = 'sessions'
           AND indexname = 'sessions_one_live_per_room_idx'
           AND indexdef LIKE '%WHERE (state = ANY%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND tablename = 'scenes'
           AND indexname = 'scenes_one_active_per_session_room_idx'
           AND indexdef LIKE '%WHERE (state = %'
    ) THEN
        RAISE EXCEPTION 'P06 active Session/Scene uniqueness is not physical';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('public', 'campaigns', 'campaigns_event_guard'),
              ('public', 'rooms', 'rooms_event_guard'),
              ('public', 'scenarios', 'scenarios_event_guard'),
              ('public', 'characters', 'characters_event_guard'),
              (
                  'public',
                  'character_sheet_versions',
                  'character_sheet_versions_event_guard'
              ),
              ('core_domain', 'sessions', 'sessions_event_guard'),
              ('public', 'scenes', 'scenes_event_guard'),
              ('public', 'campaign_forks', 'campaign_forks_event_guard'),
              (
                  'public',
                  'reconsiderations',
                  'reconsiderations_event_guard'
              )
          ) AS expected(schema_name, table_name, trigger_name)
          LEFT JOIN pg_namespace AS namespace
            ON namespace.nspname = expected.schema_name
          LEFT JOIN pg_class AS relation
            ON relation.relnamespace = namespace.oid
           AND relation.relname = expected.table_name
          LEFT JOIN pg_trigger AS trigger
            ON trigger.tgrelid = relation.oid
           AND trigger.tgname = expected.trigger_name
           AND NOT trigger.tgisinternal
         WHERE trigger.oid IS NULL
    ) THEN
        RAISE EXCEPTION 'P06 canonical-event projection guard is incomplete';
    END IF;
    SELECT pg_get_functiondef(
               'public.enforce_core_projection_event()'::regprocedure
           )
      INTO trigger_function_signature;
    IF strpos(
           trigger_function_signature,
           'canonical.authenticated_actor_role IS DISTINCT FROM ''workflow'''
       ) = 0
       OR strpos(
           trigger_function_signature,
           'audit.action = ''write_official_state'''
       ) = 0
       OR strpos(
           trigger_function_signature,
           'audit.decision = ''PERMIT'''
       ) = 0
       OR strpos(
           trigger_function_signature,
           'canonical.projection_targets'
       ) = 0
       OR strpos(
           trigger_function_signature,
           'trpg.projection_capability'
       ) = 0
       OR strpos(
           trigger_function_signature,
           'capability_hash'
       ) = 0
    THEN
        RAISE EXCEPTION 'P06 projection guard does not require a formal workflow permit and secret capability';
    END IF;
END;
$$;

SELECT 'P06_SCHEMA_ASSERTION_OK' AS schema_assertion;

DO $$
DECLARE
    guarded_function TEXT;
    invite_guarded_function TEXT;
BEGIN
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('player_actions'),
              ('decision_records'),
              ('dice_rolls'),
              ('clues'),
              ('sanity_events')
          ) AS expected(table_name)
         WHERE to_regclass(format('public.%I', expected.table_name)) IS NULL
    ) THEN
        RAISE EXCEPTION 'P07 player-action projection table set is incomplete';
    END IF;
    IF to_regprocedure(
           'core_domain.player_action_projection_id(jsonb)'
       ) IS NULL
       OR to_regprocedure(
           'core_domain.apply_player_action_projection(text,jsonb)'
       ) IS NULL
       OR to_regprocedure(
           'core_domain.campaign_invite_acceptance_projection_id(jsonb)'
       ) IS NULL
       OR to_regprocedure(
           'core_domain.apply_campaign_invite_acceptance(text,jsonb)'
       ) IS NULL
       OR NOT EXISTS (
           SELECT 1
             FROM pg_proc AS procedure
             JOIN pg_namespace AS namespace
               ON namespace.oid = procedure.pronamespace
            WHERE namespace.nspname = 'core_domain'
              AND procedure.proname = 'apply_player_action_projection'
              AND procedure.prosecdef
       )
       OR NOT EXISTS (
           SELECT 1
             FROM pg_proc AS procedure
             JOIN pg_namespace AS namespace
               ON namespace.oid = procedure.pronamespace
            WHERE namespace.nspname = 'core_domain'
              AND procedure.proname = 'apply_campaign_invite_acceptance'
              AND procedure.prosecdef
       )
    THEN
        RAISE EXCEPTION 'P07 guarded projection functions are incomplete';
    END IF;
    IF NOT has_schema_privilege(
           'trpg_canonical_service', 'core_domain', 'USAGE'
       )
       OR NOT has_function_privilege(
           'trpg_canonical_service',
           'core_domain.apply_player_action_projection(text,jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_canonical_service',
           'core_domain.player_action_projection_id(jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_api_service',
           'core_domain.apply_player_action_projection(text,jsonb)',
           'EXECUTE'
       )
       OR NOT has_function_privilege(
           'trpg_api_service',
           'core_domain.player_action_projection_id(jsonb)',
           'EXECUTE'
       )
       OR NOT has_function_privilege(
           'trpg_canonical_service',
           'core_domain.apply_campaign_invite_acceptance(text,jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_canonical_service',
           'core_domain.campaign_invite_acceptance_projection_id(jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_api_service',
           'core_domain.apply_campaign_invite_acceptance(text,jsonb)',
           'EXECUTE'
       )
       OR NOT has_function_privilege(
           'trpg_api_service',
           'core_domain.campaign_invite_acceptance_projection_id(jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_worker_service',
           'core_domain.apply_player_action_projection(text,jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_worker_service',
           'core_domain.apply_campaign_invite_acceptance(text,jsonb)',
           'EXECUTE'
       )
       OR has_table_privilege(
           'trpg_canonical_service',
           'public.campaign_memberships',
           'INSERT'
       )
       OR EXISTS (
           SELECT 1
             FROM pg_proc AS procedure
             JOIN pg_namespace AS namespace
               ON namespace.oid = procedure.pronamespace,
                  LATERAL aclexplode(procedure.proacl) AS privilege
            WHERE namespace.nspname = 'core_domain'
              AND procedure.proname IN (
                  'apply_player_action_projection',
                  'player_action_projection_id',
                  'apply_campaign_invite_acceptance',
                  'campaign_invite_acceptance_projection_id'
              )
              AND privilege.grantee = 0
              AND privilege.privilege_type = 'EXECUTE'
       )
    THEN
        RAISE EXCEPTION 'P07 projection function privilege boundary drifted';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('player_actions'),
              ('decision_records'),
              ('dice_rolls'),
              ('clues'),
              ('sanity_events')
          ) AS expected(table_name)
         WHERE has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'SELECT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'INSERT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'UPDATE'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'DELETE'
               )
            OR NOT has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'SELECT'
               )
            OR has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'INSERT'
               )
            OR has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'UPDATE'
               )
            OR has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'DELETE'
               )
    ) THEN
        RAISE EXCEPTION 'P07 player-action table privilege boundary drifted';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('player_actions', 'player_actions_event_guard'),
              ('decision_records', 'decision_records_event_guard'),
              ('dice_rolls', 'dice_rolls_event_guard'),
              ('clues', 'clues_event_guard'),
              ('sanity_events', 'sanity_events_event_guard')
          ) AS expected(table_name, trigger_name)
          LEFT JOIN pg_class AS relation
            ON relation.oid = to_regclass(
                format('public.%I', expected.table_name)
            )
          LEFT JOIN pg_trigger AS trigger
            ON trigger.tgrelid = relation.oid
           AND trigger.tgname = expected.trigger_name
           AND NOT trigger.tgisinternal
         WHERE trigger.oid IS NULL
    ) THEN
        RAISE EXCEPTION 'P07 canonical-event projection guard is incomplete';
    END IF;
    SELECT pg_get_functiondef(
               'core_domain.apply_player_action_projection(text,jsonb)'::regprocedure
           )
      INTO guarded_function;
    IF strpos(guarded_function, 'trpg.projection_capability') = 0
       OR strpos(guarded_function, 'projection_capability_hash') = 0
       OR strpos(guarded_function, 'canonical_audit_log') = 0
       OR strpos(guarded_function, 'write_official_state') = 0
       OR strpos(guarded_function, 'SERVER_OS_CSPRNG') = 0
    THEN
        RAISE EXCEPTION 'P07 guarded projection omits capability, policy, or server-RNG evidence';
    END IF;
    SELECT pg_get_functiondef(
               'core_domain.apply_campaign_invite_acceptance(text,jsonb)'::regprocedure
           )
      INTO invite_guarded_function;
    IF strpos(invite_guarded_function, 'trpg.projection_capability') = 0
       OR strpos(invite_guarded_function, 'projection_capability_hash') = 0
       OR strpos(invite_guarded_function, 'canonical_audit_log') = 0
       OR strpos(invite_guarded_function, 'write_official_state') = 0
       OR strpos(invite_guarded_function, 'CampaignInviteAccepted') = 0
       OR strpos(invite_guarded_function, 'campaign_memberships') = 0
    THEN
        RAISE EXCEPTION 'P07 invite acceptance is not an atomic guarded projection';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint AS table_constraint
          JOIN pg_class AS relation
            ON relation.oid = table_constraint.conrelid
         WHERE relation.oid = 'public.player_actions'::regclass
           AND pg_get_constraintdef(table_constraint.oid) LIKE '%intent_json%'
           AND pg_get_constraintdef(table_constraint.oid) LIKE '%dice_roll%'
           AND pg_get_constraintdef(table_constraint.oid) LIKE '%random_value%'
    ) THEN
        RAISE EXCEPTION 'P07 player action intent does not reject client dice fields';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000300
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 combat/chase/conclusion migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000400
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 fork materialization/replay migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000500
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 complete fork scope migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000600
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 child lineage uniqueness migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000700
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 global gameplay roll migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000800
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 production rebuild authorization migration is not applied';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM public._sqlx_migrations
         WHERE version = 20260727000900
           AND success
    ) THEN
        RAISE EXCEPTION 'P08 non-null projection shape migration is not applied';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('combat_states'),
              ('chase_states'),
              ('gameplay_roll_consumptions'),
              ('ending_events'),
              ('growth_events'),
              ('campaign_fork_materializations'),
              ('campaign_fork_public_events'),
              ('campaign_fork_clues'),
              ('campaign_fork_npc_states')
          ) AS expected(table_name)
         WHERE to_regclass(format('public.%I', expected.table_name)) IS NULL
    ) THEN
        RAISE EXCEPTION 'P08 persistent aggregate table set is incomplete';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('campaign_forks', 'child_snapshot_hash'),
              ('campaign_forks', 'copy_scope_json'),
              ('campaign_forks', 'snapshot_json'),
              ('campaign_forks', 'materialization_version'),
              ('growth_events', 'increase_roll_id'),
              ('reconsiderations', 'review_workflow_version'),
              ('reconsiderations', 'review_summary'),
              ('reconsiderations', 'outcome'),
              ('reconsiderations', 'corrected_event_type'),
              ('reconsiderations', 'corrected_payload')
          ) AS expected(table_name, column_name)
         WHERE NOT EXISTS (
             SELECT 1
               FROM information_schema.columns AS column_info
              WHERE column_info.table_schema = 'public'
                AND column_info.table_name = expected.table_name
                AND column_info.column_name = expected.column_name
         )
    ) THEN
        RAISE EXCEPTION 'P08 fork/reconsideration hardening columns are incomplete';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('combat_states', 'combat_states_event_guard'),
              ('chase_states', 'chase_states_event_guard'),
              ('gameplay_roll_consumptions',
               'gameplay_roll_consumptions_event_guard'),
              ('ending_events', 'ending_events_event_guard'),
              ('growth_events', 'growth_events_event_guard'),
              ('campaign_fork_materializations',
               'campaign_fork_materializations_event_guard'),
              ('campaign_fork_public_events',
               'campaign_fork_public_events_event_guard'),
              ('campaign_fork_clues',
               'campaign_fork_clues_event_guard'),
              ('campaign_fork_npc_states',
               'campaign_fork_npc_states_event_guard')
          ) AS expected(table_name, trigger_name)
          LEFT JOIN pg_class AS relation
            ON relation.oid = to_regclass(
                format('public.%I', expected.table_name)
            )
          LEFT JOIN pg_trigger AS trigger
            ON trigger.tgrelid = relation.oid
           AND trigger.tgname = expected.trigger_name
           AND NOT trigger.tgisinternal
         WHERE trigger.oid IS NULL
    ) THEN
        RAISE EXCEPTION 'P08 canonical-event projection guard is incomplete';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('scenarios', 'scenarios_event_guard'),
              ('characters', 'characters_event_guard'),
              ('character_sheet_versions',
               'character_sheet_versions_event_guard'),
              ('scenes', 'scenes_event_guard'),
              ('combat_states', 'combat_states_event_guard'),
              ('chase_states', 'chase_states_event_guard'),
              ('ending_events', 'ending_events_event_guard'),
              ('campaign_fork_public_events',
               'campaign_fork_public_events_event_guard'),
              ('campaign_fork_clues',
               'campaign_fork_clues_event_guard'),
              ('campaign_fork_npc_states',
               'campaign_fork_npc_states_event_guard')
          ) AS expected(table_name, trigger_name)
         WHERE NOT EXISTS (
             SELECT 1
               FROM pg_trigger
              WHERE tgrelid = to_regclass(
                        format('public.%I', expected.table_name)
                    )
                AND tgname = expected.trigger_name
                AND NOT tgisinternal
                AND encode(tgargs, 'escape')
                    LIKE '%CampaignForkMaterialized%'
         )
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid = 'core_domain.sessions'::regclass
           AND tgname = 'sessions_event_guard'
           AND NOT tgisinternal
           AND encode(tgargs, 'escape') LIKE '%CampaignForkMaterialized%'
    ) THEN
        RAISE EXCEPTION 'P08 fork materialization projection allow-list is incomplete';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid = 'public.reconsiderations'::regclass
           AND tgname = 'reconsiderations_event_guard'
           AND encode(tgargs, 'escape') LIKE '%ReconsiderationUpheld%'
           AND encode(tgargs, 'escape') LIKE '%ReconsiderationCorrected%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid = 'public.characters'::regclass
           AND tgname = 'characters_event_guard'
           AND encode(tgargs, 'escape') LIKE '%SanityLossApplied%'
           AND encode(tgargs, 'escape') LIKE '%CharacterGrowthApplied%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid = 'public.character_sheet_versions'::regclass
           AND tgname = 'character_sheet_versions_event_guard'
           AND encode(tgargs, 'escape') LIKE '%SanityLossApplied%'
           AND encode(tgargs, 'escape') LIKE '%CharacterGrowthApplied%'
    ) THEN
        RAISE EXCEPTION 'P08 correction/growth event allow-list is incomplete';
    END IF;
    IF (
        SELECT count(*)
          FROM pg_constraint
         WHERE conrelid IN (
             'public.combat_states'::regclass,
             'public.chase_states'::regclass,
             'public.ending_events'::regclass,
             'public.growth_events'::regclass,
             'core_domain.session_ending_reservations'::regclass
         )
           AND confrelid = 'core_domain.sessions'::regclass
           AND contype = 'f'
           AND condeferrable
           AND condeferred
    ) <> 5 THEN
        RAISE EXCEPTION 'P08 session references would block projection rebuild';
    END IF;
    IF EXISTS (
        SELECT 1
          FROM (VALUES
              ('combat_states'),
              ('chase_states'),
              ('gameplay_roll_consumptions'),
              ('ending_events'),
              ('growth_events'),
              ('campaign_fork_materializations'),
              ('campaign_fork_public_events'),
              ('campaign_fork_clues'),
              ('campaign_fork_npc_states')
          ) AS expected(table_name)
         WHERE has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'SELECT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'INSERT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'UPDATE'
               )
            OR has_table_privilege(
                   'trpg_canonical_service',
                   format('public.%I', expected.table_name),
                   'DELETE'
               )
            OR NOT has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'SELECT'
               )
            OR has_table_privilege(
                   'trpg_api_service',
                   format('public.%I', expected.table_name),
                   'DELETE'
               )
    ) THEN
        RAISE EXCEPTION 'P08 table privilege boundary drifted';
    END IF;
    IF NOT has_function_privilege(
               'trpg_api_service',
               'core_domain.clear_p08_rebuildable_projections(text,text)',
               'EXECUTE'
           )
       OR has_function_privilege(
               'trpg_canonical_service',
               'core_domain.clear_p08_rebuildable_projections(text,text)',
               'EXECUTE'
           )
       OR has_function_privilege(
               'trpg_worker_service',
               'core_domain.clear_p08_rebuildable_projections(text,text)',
               'EXECUTE'
           )
       OR has_function_privilege(
               'trpg_realtime_service',
               'core_domain.clear_p08_rebuildable_projections(text,text)',
               'EXECUTE'
           )
       OR NOT EXISTS (
            SELECT 1
              FROM pg_proc AS procedure
              JOIN pg_namespace AS namespace
                ON namespace.oid = procedure.pronamespace
             WHERE namespace.nspname = 'core_domain'
               AND procedure.proname =
                   'clear_p08_rebuildable_projections'
               AND procedure.prosecdef
               AND pg_get_functiondef(procedure.oid)
                   LIKE '%P08 projection rebuild capability rejected%'
               AND pg_get_functiondef(procedure.oid)
                   LIKE '%verified_hmac%'
               AND pg_get_functiondef(procedure.oid)
                   LIKE '%formal_commit%'
               AND pg_get_functiondef(procedure.oid)
                   LIKE '%projection_targets%'
       ) THEN
        RAISE EXCEPTION 'P08 rebuild repair capability boundary drifted';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'public.campaign_forks'::regclass
           AND pg_get_constraintdef(oid) LIKE '%child_snapshot_hash%'
           AND pg_get_constraintdef(oid)
               LIKE '%child_snapshot_hash IS NOT NULL%'
           AND pg_get_constraintdef(oid)
               LIKE '%copy_scope_json IS NOT NULL%'
           AND pg_get_constraintdef(oid)
               LIKE '%snapshot_json IS NOT NULL%'
           AND pg_get_constraintdef(oid) LIKE '%AI_INTERNAL_MEMORY%'
           AND pg_get_constraintdef(oid) LIKE '%materialization_version%'
           AND pg_get_constraintdef(oid) LIKE '%child_campaign_id%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'public.reconsiderations'::regclass
           AND conname = 'reconsiderations_v2_append_only_shape'
           AND pg_get_constraintdef(oid)
               LIKE '%review_summary IS NOT NULL%'
           AND pg_get_constraintdef(oid)
               LIKE '%resolution IS NOT NULL%'
           AND pg_get_constraintdef(oid)
               LIKE '%corrected_event_type IS NOT NULL%'
           AND pg_get_constraintdef(oid)
               LIKE '%corrected_payload IS NOT NULL%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid =
               'public.campaign_fork_materializations'::regclass
           AND pg_get_constraintdef(oid) LIKE '%child_snapshot_hash%'
           AND pg_get_constraintdef(oid) LIKE '%child_state_json%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'public.growth_events'::regclass
           AND pg_get_constraintdef(oid) LIKE '%improvement_check_roll%'
           AND pg_get_constraintdef(oid) LIKE '%increase_roll%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'public.growth_events'::regclass
           AND pg_get_constraintdef(oid) LIKE '%increase_roll_id%'
           AND pg_get_constraintdef(oid) LIKE '%increase_roll IS NULL%'
           AND pg_get_constraintdef(oid) LIKE '%server_roll_id%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid =
               'public.gameplay_roll_consumptions'::regclass
           AND contype = 'p'
           AND pg_get_constraintdef(oid) LIKE '%roll_id%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid =
               'public.gameplay_roll_consumptions'::regclass
           AND contype = 'c'
           AND pg_get_constraintdef(oid) LIKE '%aggregate_kind%'
           AND pg_get_constraintdef(oid) LIKE '%GROWTH%'
    ) OR NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid =
               'public.gameplay_roll_consumptions'::regclass
           AND contype = 'c'
           AND pg_get_constraintdef(oid) LIKE '%GROWTH_PERCENTILE%'
           AND pg_get_constraintdef(oid) LIKE '%GROWTH_INCREASE_D10%'
    ) OR to_regclass(
        'public.event_store_one_fork_lineage_per_child_idx'
    ) IS NULL OR to_regclass(
        'public.campaign_forks_child_lineage_unique'
    ) IS NULL
    OR NOT EXISTS (
        SELECT 1
          FROM pg_indexes
         WHERE schemaname = 'public'
           AND indexname =
               'event_store_one_fork_lineage_per_child_idx'
           AND indexdef LIKE '%projection_targets%'
           AND indexdef LIKE '%public.campaign_fork_materializations%'
    )
    OR NOT EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_namespace AS namespace
            ON namespace.oid = procedure.pronamespace
         WHERE namespace.nspname = 'public'
           AND procedure.proname =
               'enforce_campaign_fork_empty_child_history'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%p08-campaign-fork-empty:%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%CampaignForkRecorded%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%public.campaign_fork_materializations%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%CampaignInviteAccepted%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%campaign fork child canonical history is not empty%'
    )
    OR NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid = 'public.event_store'::regclass
           AND tgname =
               'event_store_campaign_fork_empty_child_guard'
           AND NOT tgisinternal
    )
    OR NOT EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_namespace AS namespace
            ON namespace.oid = procedure.pronamespace
         WHERE namespace.nspname = 'public'
           AND procedure.proname = 'enforce_core_projection_event'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%trpg.p08_projection_rebuild%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%CharacterGrowthApplied%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%suffix_event.event_type <> ''CharacterGrowthApplied''%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%growth_rewind_allowed%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%version_target%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%public.characters%'
    )
    OR NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgrelid =
               'public.gameplay_roll_consumptions'::regclass
           AND tgname = 'gameplay_roll_consumptions_event_guard'
           AND NOT tgisinternal
           AND encode(tgargs, 'escape') LIKE '%CombatStateRecorded%'
           AND encode(tgargs, 'escape') LIKE '%ChaseStateRecorded%'
           AND encode(tgargs, 'escape') LIKE '%CharacterGrowthApplied%'
    )
    OR to_regprocedure(
        'core_domain.gameplay_roll_reservation_projection_id(jsonb)'
    ) IS NULL
    OR NOT EXISTS (
        SELECT 1
          FROM pg_proc AS procedure
          JOIN pg_namespace AS namespace
            ON namespace.oid = procedure.pronamespace
         WHERE namespace.nspname = 'core_domain'
           AND procedure.proname =
               'reserve_gameplay_roll_consumptions'
           AND procedure.prosecdef
           AND pg_get_functiondef(procedure.oid)
               LIKE '%formal_commits%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%gameplay roll reservation is not HMAC-bound%'
           AND pg_get_functiondef(procedure.oid)
               LIKE '%ON CONFLICT (roll_id) DO NOTHING%'
    ) THEN
        RAISE EXCEPTION 'P08 snapshot, fork serialization, growth, or global roll evidence is not physical';
    END IF;
    IF NOT has_function_privilege(
           'trpg_canonical_service',
           'core_domain.reserve_gameplay_roll_consumptions(text,jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_canonical_service',
           'core_domain.gameplay_roll_reservation_projection_id(jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_api_service',
           'core_domain.reserve_gameplay_roll_consumptions(text,jsonb)',
           'EXECUTE'
       )
       OR NOT has_function_privilege(
           'trpg_api_service',
           'core_domain.gameplay_roll_reservation_projection_id(jsonb)',
           'EXECUTE'
       )
       OR has_function_privilege(
           'trpg_worker_service',
           'core_domain.reserve_gameplay_roll_consumptions(text,jsonb)',
           'EXECUTE'
       )
       OR EXISTS (
           SELECT 1
             FROM pg_proc AS procedure
             JOIN pg_namespace AS namespace
               ON namespace.oid = procedure.pronamespace,
                  LATERAL aclexplode(procedure.proacl) AS privilege
            WHERE namespace.nspname = 'core_domain'
              AND procedure.proname IN (
                  'reserve_gameplay_roll_consumptions',
                  'gameplay_roll_reservation_projection_id'
              )
              AND privilege.grantee = 0
              AND privilege.privilege_type = 'EXECUTE'
       ) THEN
        RAISE EXCEPTION 'P08 canonical roll reservation privilege boundary drifted';
    END IF;

    BEGIN
        CREATE TEMP TABLE p08_fork_shape_probe (
            LIKE public.campaign_forks
            INCLUDING DEFAULTS
            INCLUDING CONSTRAINTS
        );
        INSERT INTO p08_fork_shape_probe (
            fork_id, campaign_id, parent_campaign_id, child_campaign_id,
            source_session_id, source_snapshot_hash, reason, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by,
            last_event_sequence, materialization_version,
            child_snapshot_hash, copy_scope_json, snapshot_json
        ) VALUES (
            'fork_probe', 'child_probe', 'parent_probe', 'child_probe',
            'session_probe',
            'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            'constraint probe', 1, 'keeper_only', 'not_applicable',
            'system_fixture', 'constraint_probe', 'schema_assertion',
            1, 2, NULL, NULL, NULL
        );
        RAISE EXCEPTION 'P08 v2 fork NULL shape bypassed its CHECK constraint';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;

    BEGIN
        CREATE TEMP TABLE p08_reviewed_shape_probe (
            LIKE public.reconsiderations
            INCLUDING DEFAULTS
            INCLUDING CONSTRAINTS
        );
        INSERT INTO p08_reviewed_shape_probe (
            reconsideration_id, campaign_id, original_event_sequence,
            requested_by, reason, state, resolution, event_chain, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by,
            last_event_sequence, review_workflow_version, review_summary,
            outcome, corrected_event_type, corrected_payload
        ) VALUES (
            'reviewed_probe', 'campaign_probe', 1, 'user_probe',
            'constraint probe', 'REVIEWED', NULL, '["event_probe"]'::JSONB,
            1, 'keeper_only', 'not_applicable', 'system_fixture',
            'constraint_probe', 'schema_assertion', 1, 2, NULL,
            NULL, NULL, NULL
        );
        RAISE EXCEPTION 'P08 REVIEWED NULL evidence bypassed its CHECK constraint';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;

    BEGIN
        CREATE TEMP TABLE p08_upheld_shape_probe (
            LIKE public.reconsiderations
            INCLUDING DEFAULTS
            INCLUDING CONSTRAINTS
        );
        INSERT INTO p08_upheld_shape_probe (
            reconsideration_id, campaign_id, original_event_sequence,
            requested_by, reason, state, resolution, event_chain, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by,
            last_event_sequence, review_workflow_version, review_summary,
            outcome, corrected_event_type, corrected_payload
        ) VALUES (
            'upheld_probe', 'campaign_probe', 1, 'user_probe',
            'constraint probe', 'RESOLVED', NULL, '["event_probe"]'::JSONB,
            1, 'keeper_only', 'not_applicable', 'system_fixture',
            'constraint_probe', 'schema_assertion', 1, 2,
            'reviewed evidence', 'UPHELD', NULL, NULL
        );
        RAISE EXCEPTION 'P08 UPHELD NULL resolution bypassed its CHECK constraint';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;

    BEGIN
        CREATE TEMP TABLE p08_corrected_shape_probe (
            LIKE public.reconsiderations
            INCLUDING DEFAULTS
            INCLUDING CONSTRAINTS
        );
        INSERT INTO p08_corrected_shape_probe (
            reconsideration_id, campaign_id, original_event_sequence,
            requested_by, reason, state, resolution, event_chain, version,
            visibility_label, visibility_subject, provenance_kind,
            provenance_reference, provenance_recorded_by,
            last_event_sequence, review_workflow_version, review_summary,
            outcome, corrected_event_type, corrected_payload
        ) VALUES (
            'corrected_probe', 'campaign_probe', 1, 'user_probe',
            'constraint probe', 'RESOLVED', 'corrected',
            '["event_probe"]'::JSONB, 1, 'keeper_only', 'not_applicable',
            'system_fixture', 'constraint_probe', 'schema_assertion',
            1, 2, 'reviewed evidence', 'CORRECTED',
            'CorrectedEvent', NULL
        );
        RAISE EXCEPTION 'P08 CORRECTED NULL payload bypassed its CHECK constraint';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

SELECT 'P07_SCHEMA_ASSERTION_OK' AS schema_assertion;
SELECT 'P08_SCHEMA_ASSERTION_OK' AS schema_assertion;

ROLLBACK;
