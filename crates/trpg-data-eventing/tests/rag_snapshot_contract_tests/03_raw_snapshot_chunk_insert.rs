
struct RawSnapshotChunkInsert<'a> {
    campaign_id: &'a str,
    snapshot_id: &'a str,
    embedding_model: &'a str,
    source_event_sequence: i64,
    derivation_event_sequence: i64,
    chunk_id: &'a str,
    content: &'a str,
    embedding_dimensions: i32,
    embedding: &'a str,
}

async fn insert_raw_snapshot_chunk(
    pool: &sqlx::PgPool,
    insert: RawSnapshotChunkInsert<'_>,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO rag_snapshot_chunk (
            campaign_id, snapshot_id, chunk_id, source_event_sequence,
            derivation_event_sequence,
            source_type, visibility, visibility_subject, copyright_status,
            version, owner, allowed_use, fact_provenance, chunk_hash, content,
            embedding_model, embedding_dimensions, embedding
        )
        SELECT event.campaign_id, $2, $3, event.sequence, $9,
               'campaign_memory', event.visibility_label,
               event.visibility_subject, 'original', event.stream_version,
               event.authority_owner, 'campaign_only',
               jsonb_build_object(
                   'kind', event.fact_provenance_kind,
                   'reference', event.fact_provenance_reference,
                   'recorded_by', event.fact_recorded_by
               ),
               encode(sha256(convert_to($4, 'UTF8')), 'hex'), $4,
               $5, $6, $7::vector
          FROM event_store AS event
         WHERE event.campaign_id = $1 AND event.sequence = $8
        "#,
    )
    .bind(insert.campaign_id)
    .bind(insert.snapshot_id)
    .bind(insert.chunk_id)
    .bind(insert.content)
    .bind(insert.embedding_model)
    .bind(insert.embedding_dimensions)
    .bind(insert.embedding)
    .bind(insert.source_event_sequence)
    .bind(insert.derivation_event_sequence)
    .execute(&mut *transaction)
    .await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    transaction.commit().await?;
    Ok(())
}
