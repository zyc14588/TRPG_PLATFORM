
fn validate_search_embedding(embedding: &[f32]) -> Result<(), PgvectorRagError> {
    if embedding.is_empty()
        || embedding.len() > 4_096
        || embedding.iter().any(|component| !component.is_finite())
        || embedding.iter().all(|component| *component == 0.0)
    {
        Err(RagSnapshotValidationError::InvalidEmbedding.into())
    } else {
        Ok(())
    }
}

fn pgvector_literal(embedding: &[f32]) -> String {
    let values = embedding
        .iter()
        .map(|component| component.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn search_hit_from_row(row: &sqlx::postgres::PgRow) -> Result<RagSearchHit, PgvectorRagError> {
    let fact_provenance: Json<RagFactProvenance> = row.get("fact_provenance");
    let cosine_distance: f64 = row.get("cosine_distance");
    if !cosine_distance.is_finite() {
        return Err(PgvectorRagError::Database("non_finite_cosine_distance"));
    }
    let chunk = RagSnapshotChunk::from_persisted(PersistedRagSnapshotChunk {
        campaign_id: row.get("campaign_id"),
        snapshot_id: row.get("snapshot_id"),
        chunk_id: row.get("chunk_id"),
        source_event_sequence: row.get("source_event_sequence"),
        derivation_event_sequence: row.get("derivation_event_sequence"),
        source_type: row.get("source_type"),
        visibility: row.get("visibility"),
        visibility_subject: row.get("visibility_subject"),
        copyright_status: row.get("copyright_status"),
        version: row.get("version"),
        owner: row.get("owner"),
        allowed_use: row.get("allowed_use"),
        fact_provenance: fact_provenance.0,
        chunk_hash: row.get("chunk_hash"),
        content: row.get("content"),
        embedding_model: row.get("embedding_model"),
    })?;
    Ok(RagSearchHit {
        chunk,
        cosine_distance,
    })
}
