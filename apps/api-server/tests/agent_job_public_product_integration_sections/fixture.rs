pub(super) fn seed_public_character_fixture(
    runtime: &tokio::runtime::Runtime,
    fixture_pool: &sqlx::PgPool,
    job_id: String,
    campaign_id: String,
    owner_id: String,
    character_id: String,
    input_event_sequence: i64,
) {
    runtime.block_on(async {
        let mut transaction = fixture_pool.begin().await.expect("begin fixture");
        sqlx::query("SET LOCAL session_replication_role = 'replica'")
            .execute(&mut *transaction)
            .await
            .expect("bound fixture trigger bypass");
        sqlx::query(
            r#"
                UPDATE workflow_instances
                   SET state = 'TERMINAL_FAILED',
                       lease_owner = NULL,
                       claim_token = NULL,
                       lease_expires_at = NULL,
                       heartbeat_at = NULL,
                       next_attempt_at = NULL
                 WHERE workflow_type = 'agent_job'
                   AND workflow_id <> $1
                   AND state NOT IN ('COMPLETED', 'TERMINAL_FAILED')
                "#,
        )
        .bind(&job_id)
        .execute(&mut *transaction)
        .await
        .expect("retire stale jobs in the dedicated fixture database");
        sqlx::query(
            r#"
                INSERT INTO campaigns (
                    campaign_id, owner_user_id, authority_contract_id, title,
                    state, version, created_at, visibility_label,
                    visibility_subject, provenance_kind, provenance_reference,
                    provenance_recorded_by, last_event_sequence
                ) VALUES (
                    $1, $2, $3, 'AR09 public product', 'ACTIVE', 1, now(),
                    'party_visible', 'not_applicable', 'system_fixture',
                    $4, $2, $5
                )
                "#,
        )
        .bind(&campaign_id)
        .bind(&owner_id)
        .bind(format!("authority_{campaign_id}"))
        .bind(format!("projection_{job_id}"))
        .bind(input_event_sequence)
        .execute(&mut *transaction)
        .await
        .expect("seed campaign projection");
        sqlx::query(
            r#"
                INSERT INTO characters (
                    character_id, campaign_id, owner_user_id, display_name,
                    state, current_sheet_version, initial_version_locked,
                    version, visibility_label, visibility_subject,
                    provenance_kind, provenance_reference,
                    provenance_recorded_by, last_event_sequence
                ) VALUES (
                    $1, $2, $3, 'AR09 Investigator', 'APPROVED', 1, TRUE, 1,
                    'party_visible', 'not_applicable', 'system_fixture',
                    $4, $3, $5
                )
                "#,
        )
        .bind(&character_id)
        .bind(&campaign_id)
        .bind(&owner_id)
        .bind(format!("character_{job_id}"))
        .bind(input_event_sequence)
        .execute(&mut *transaction)
        .await
        .expect("seed approved character");
        sqlx::query(
            r#"
                INSERT INTO character_sheet_versions (
                    sheet_version_id, character_id, version, sheet_json,
                    locked, visibility_label, visibility_subject,
                    provenance_kind, provenance_reference,
                    provenance_recorded_by, campaign_id, last_event_sequence
                ) VALUES (
                    $1, $2, 1, '{"skills":{"Library Use":72}}'::jsonb, TRUE,
                    'party_visible', 'not_applicable', 'system_fixture',
                    $3, $4, $5, $6
                )
                "#,
        )
        .bind(format!("sheet_{character_id}"))
        .bind(&character_id)
        .bind(format!("sheet_{job_id}"))
        .bind(&owner_id)
        .bind(&campaign_id)
        .bind(input_event_sequence)
        .execute(&mut *transaction)
        .await
        .expect("seed locked character sheet");
        transaction.commit().await.expect("commit fixture");
    });
}
