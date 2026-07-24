crate::define_data_event_module!(
    PostgreSqlSqLxPgvectorCommand,
    PostgreSqlSqLxPgvectorOperation,
    append_postgre_sql_sq_lx_pgvector_event,
    "postgre_sql_sq_lx_pgvector",
    "PostgreSqlPgvectorIndexed",
    "data_eventing.postgre_sql_sq_lx_pgvector.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["rag_index", "event_store", "snapshot_store"]
);

crate::define_data_event_artifacts!(
    PostgreSqlSqLxPgvectorService,
    PostgreSqlSqLxPgvectorRepository,
    PostgreSqlSqLxPgvectorEvent,
    PostgreSqlSqLxPgvectorError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const RAG_INDEX_FIELDS: &[&str] = &["source_event_sequence", "visibility", "fact_provenance"];

use std::fmt;

use sqlx::types::Json;
use sqlx::{PgPool, Row};
use trpg_identity::ReplayAuthorization;
use trpg_shared_kernel::{EntityId, Visibility, VisibilityKind};

use crate::rag_snapshot::{
    validate_snapshot_identity, PersistedRagSnapshotChunk, RagFactProvenance, RagSearchHit,
    RagSnapshotChunk, RagSnapshotChunkDraft, RagSnapshotValidationError,
};

const MAX_SNAPSHOT_CHUNKS: usize = 10_000;

#[derive(Clone)]
pub struct PostgresRagSnapshotRepository {
    pool: PgPool,
}

pub struct RagVisibleSearch<'a> {
    pub campaign_id: &'a str,
    pub snapshot_id: &'a str,
    pub authorization: Option<&'a ReplayAuthorization>,
    pub now_unix_ms: u64,
    pub embedding_model: &'a str,
    pub embedding: &'a [f32],
    pub limit: i64,
}

impl fmt::Debug for PostgresRagSnapshotRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresRagSnapshotRepository")
            .field("pool", &"[POSTGRESQL POOL]")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PgvectorRagError {
    Validation(RagSnapshotValidationError),
    SourceEventNotFound { sequence: i64 },
    Authorization,
    Database(&'static str),
}

impl fmt::Display for PgvectorRagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "invalid RAG snapshot: {error:?}"),
            Self::SourceEventNotFound { sequence } => {
                write!(formatter, "canonical source event {sequence} was not found")
            }
            Self::Authorization => formatter.write_str("RAG search authorization failed"),
            Self::Database(operation) => write!(formatter, "RAG database failed: {operation}"),
        }
    }
}

impl std::error::Error for PgvectorRagError {}

impl From<RagSnapshotValidationError> for PgvectorRagError {
    fn from(value: RagSnapshotValidationError) -> Self {
        Self::Validation(value)
    }
}

impl PostgresRagSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Production dependency gate for the pgvector-backed read model. This
    /// deliberately checks the database enforcement boundary, not just table
    /// existence, so a worker cannot report ready with the snapshot lock or
    /// provenance trigger missing.
    pub async fn check_readiness(&self) -> Result<(), PgvectorRagError> {
        let ready: bool = sqlx::query_scalar(
            r#"
            SELECT to_regclass('public.rag_snapshot_chunk') IS NOT NULL
               AND to_regprocedure('public.lock_rag_snapshot(text,text)') IS NOT NULL
               AND EXISTS (
                   SELECT 1
                     FROM pg_extension
                    WHERE extname = 'vector'
               )
               AND EXISTS (
                   SELECT 1
                     FROM pg_trigger
                    WHERE tgrelid = 'public.rag_snapshot_chunk'::regclass
                      AND tgname = 'rag_snapshot_chunk_source_guard'
                      AND tgenabled = 'O'
                      AND NOT tgisinternal
               )
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| PgvectorRagError::Database("check_rag_readiness"))?;
        if ready {
            Ok(())
        } else {
            Err(PgvectorRagError::Database("rag_schema_missing"))
        }
    }

    /// Atomically replaces one rebuildable snapshot. Canonical metadata is
    /// selected from Event Store inside the transaction rather than trusted
    /// from the projector input.
    #[tracing::instrument(
        name = "rag_snapshot_rebuild",
        skip_all,
        fields(campaign_id = campaign_id, snapshot_id = snapshot_id, chunk_count = chunks.len())
    )]
    pub async fn replace_snapshot(
        &self,
        campaign_id: &str,
        snapshot_id: &str,
        chunks: &[RagSnapshotChunkDraft],
    ) -> Result<usize, PgvectorRagError> {
        validate_snapshot_identity(campaign_id, snapshot_id)?;
        if chunks.is_empty() {
            return Err(RagSnapshotValidationError::EmptySnapshot.into());
        }
        if chunks.len() > MAX_SNAPSHOT_CHUNKS {
            return Err(RagSnapshotValidationError::TooManyChunks.into());
        }
        let expected_dimensions = chunks[0].embedding.len();
        let expected_model = &chunks[0].embedding_model;
        for chunk in chunks {
            chunk.validate()?;
            if chunk.embedding.len() != expected_dimensions {
                return Err(RagSnapshotValidationError::InconsistentEmbeddingDimensions.into());
            }
            if chunk.embedding_model != *expected_model {
                return Err(RagSnapshotValidationError::InconsistentEmbeddingModel.into());
            }
        }

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| PgvectorRagError::Database("begin_snapshot_rebuild"))?;
        // Serialize all delete-and-replace operations for the same logical
        // snapshot before the first mutating statement. Under PostgreSQL READ
        // COMMITTED, acquiring this transaction lock in a separate statement
        // ensures the following DELETE sees the preceding rebuilder's commit.
        sqlx::query("SELECT public.lock_rag_snapshot($1, $2)")
            .bind(campaign_id)
            .bind(snapshot_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PgvectorRagError::Database("lock_snapshot_rebuild"))?;
        sqlx::query(
            "DELETE FROM public.rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2",
        )
        .bind(campaign_id)
        .bind(snapshot_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| PgvectorRagError::Database("delete_previous_snapshot"))?;

        for chunk in chunks {
            let embedding = pgvector_literal(&chunk.embedding);
            let inserted: Option<i64> = sqlx::query_scalar(
                r#"
                INSERT INTO public.rag_snapshot_chunk (
                    campaign_id, snapshot_id, chunk_id, source_event_sequence,
                    derivation_event_sequence,
                    source_type, visibility, visibility_subject,
                    copyright_status, version, owner, allowed_use,
                    fact_provenance, chunk_hash, content,
                    embedding_model, embedding_dimensions, embedding
                )
                SELECT event.campaign_id, $2, $3, event.sequence, $5, $6,
                       event.visibility_label, event.visibility_subject,
                       $7, event.stream_version, event.authority_owner, $8,
                       jsonb_build_object(
                           'kind', event.fact_provenance_kind,
                           'reference', event.fact_provenance_reference,
                           'recorded_by', event.fact_recorded_by
                       ),
                       $9, $10, $11, $12, $13::vector
                  FROM public.event_store AS event
                 WHERE event.campaign_id = $1 AND event.sequence = $4
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND event.payload_json ? 'protected_payload'
                RETURNING source_event_sequence
                "#,
            )
            .bind(campaign_id)
            .bind(snapshot_id)
            .bind(&chunk.chunk_id)
            .bind(chunk.source_event_sequence)
            .bind(chunk.derivation_event_sequence)
            .bind(&chunk.source_type)
            .bind(&chunk.copyright_status)
            .bind(&chunk.allowed_use)
            .bind(chunk.content_hash())
            .bind(&chunk.content)
            .bind(&chunk.embedding_model)
            .bind(i32::try_from(chunk.embedding.len()).map_err(|_| {
                PgvectorRagError::Validation(RagSnapshotValidationError::InvalidEmbedding)
            })?)
            .bind(embedding)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| PgvectorRagError::Database("insert_snapshot_chunk"))?;
            if inserted.is_none() {
                return Err(PgvectorRagError::SourceEventNotFound {
                    sequence: chunk.source_event_sequence,
                });
            }
        }

        transaction
            .commit()
            .await
            .map_err(|_| PgvectorRagError::Database("commit_snapshot_rebuild"))?;
        Ok(chunks.len())
    }

    /// Uses a live, campaign-bound replay authorization. The first query reads
    /// classification metadata only; restricted content and embeddings are
    /// fetched in the second query solely for chunk IDs that the identity
    /// verifier just approved. `None` is an anonymous/public-only search.
    #[tracing::instrument(
        name = "rag_snapshot_search",
        skip_all,
        fields(campaign_id = query.campaign_id, snapshot_id = query.snapshot_id)
    )]
    pub async fn search_visible(
        &self,
        query: RagVisibleSearch<'_>,
    ) -> Result<Vec<RagSearchHit>, PgvectorRagError> {
        let RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization,
            now_unix_ms,
            embedding_model,
            embedding,
            limit,
        } = query;
        validate_snapshot_identity(campaign_id, snapshot_id)?;
        if !(1..=100).contains(&limit) {
            return Err(RagSnapshotValidationError::InvalidSearchLimit.into());
        }
        if embedding_model.trim().is_empty() || embedding_model.len() > 256 {
            return Err(RagSnapshotValidationError::InvalidMetadata.into());
        }
        validate_search_embedding(embedding)?;
        let campaign = EntityId::new(campaign_id).map_err(|_| {
            PgvectorRagError::Validation(RagSnapshotValidationError::InvalidCampaignId)
        })?;
        let classifications = sqlx::query(
            r#"
            SELECT chunk_id, visibility, visibility_subject
              FROM public.rag_snapshot_chunk
             WHERE campaign_id = $1
               AND snapshot_id = $2
               AND embedding_dimensions = $3
               AND embedding_model = $4
            "#,
        )
        .bind(campaign_id)
        .bind(snapshot_id)
        .bind(i32::try_from(embedding.len()).map_err(|_| {
            PgvectorRagError::Validation(RagSnapshotValidationError::InvalidEmbedding)
        })?)
        .bind(embedding_model)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PgvectorRagError::Database("classify_snapshot_search"))?;
        let mut allowed_chunk_ids = Vec::with_capacity(classifications.len());
        for row in classifications {
            let chunk_id: String = row.get("chunk_id");
            let label: String = row.get("visibility");
            let subject: String = row.get("visibility_subject");
            let visibility = Visibility::try_from_parts(
                &label,
                (subject != "not_applicable").then_some(subject.as_str()),
            )
            .map_err(|_| {
                PgvectorRagError::Validation(RagSnapshotValidationError::InvalidMetadata)
            })?;
            let allowed = match authorization {
                Some(authorization) => authorization
                    .can_view(&campaign, &visibility, now_unix_ms)
                    .map_err(|_| PgvectorRagError::Authorization)?,
                None => visibility.label().kind() == VisibilityKind::Public,
            };
            if allowed {
                allowed_chunk_ids.push(chunk_id);
            }
        }
        if allowed_chunk_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT campaign_id, snapshot_id, chunk_id, source_event_sequence,
                   derivation_event_sequence,
                   source_type, visibility, visibility_subject,
                   copyright_status, version, owner, allowed_use,
                   fact_provenance, chunk_hash, content,
                   embedding_model,
                   (embedding <=> $6::vector)::double precision AS cosine_distance
              FROM public.rag_snapshot_chunk
             WHERE campaign_id = $1
               AND snapshot_id = $2
               AND embedding_dimensions = $3
               AND embedding_model = $4
               AND chunk_id = ANY($5)
             ORDER BY embedding <=> $6::vector, chunk_id
             LIMIT $7
            "#,
        )
        .bind(campaign_id)
        .bind(snapshot_id)
        .bind(i32::try_from(embedding.len()).map_err(|_| {
            PgvectorRagError::Validation(RagSnapshotValidationError::InvalidEmbedding)
        })?)
        .bind(embedding_model)
        .bind(&allowed_chunk_ids)
        .bind(pgvector_literal(embedding))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PgvectorRagError::Database("search_snapshot"))?;

        rows.iter().map(search_hit_from_row).collect()
    }
}

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
