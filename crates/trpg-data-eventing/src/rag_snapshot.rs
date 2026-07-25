crate::define_data_event_module!(
    RagSnapshotCommand,
    RagSnapshotOperation,
    append_rag_snapshot_event,
    "rag_snapshot",
    "RagSnapshotRecorded",
    "data_eventing.rag_snapshot.event_schema",
    crate::DataEventOperation::SnapshotCreate,
    ["event_store", "rag_snapshot_store", "rag_index"]
);

crate::define_data_event_artifacts!(
    RagSnapshotService,
    RagSnapshotRepository,
    RagSnapshotEvent,
    RagSnapshotError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const RAG_SNAPSHOT_METADATA_FIELDS: &[&str] = &[
    "source_type",
    "visibility",
    "visibility_subject",
    "copyright_status",
    "version",
    "owner",
    "allowed_use",
    "fact_provenance",
    "source_event_sequence",
    "derivation_event_sequence",
    "chunk_hash",
    "embedding_model",
    "embedding_dimensions",
];

pub const RAG_REBUILD_SOURCE: &str = crate::EVENT_STORE_TABLE;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RagFactProvenance {
    kind: String,
    reference: String,
    recorded_by: String,
}

impl RagFactProvenance {
    pub fn new(
        kind: impl Into<String>,
        reference: impl Into<String>,
        recorded_by: impl Into<String>,
    ) -> Result<Self, RagSnapshotValidationError> {
        let provenance = Self {
            kind: kind.into(),
            reference: reference.into(),
            recorded_by: recorded_by.into(),
        };
        provenance.validate()?;
        Ok(provenance)
    }

    pub fn validate(&self) -> Result<(), RagSnapshotValidationError> {
        if !bounded_nonblank(&self.kind, 128)
            || !bounded_nonblank(&self.reference, 512)
            || !bounded_nonblank(&self.recorded_by, 160)
        {
            return Err(RagSnapshotValidationError::InvalidFactProvenance);
        }
        Ok(())
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RagSnapshotChunkDraft {
    pub chunk_id: String,
    pub source_event_sequence: i64,
    pub derivation_event_sequence: i64,
    pub source_type: String,
    pub copyright_status: String,
    pub allowed_use: String,
    pub content: String,
    pub embedding_model: String,
    pub embedding: Vec<f32>,
}

impl RagSnapshotChunkDraft {
    pub fn validate(&self) -> Result<(), RagSnapshotValidationError> {
        if !valid_identifier(&self.chunk_id) {
            return Err(RagSnapshotValidationError::InvalidChunkId);
        }
        if self.source_event_sequence <= 0 || self.derivation_event_sequence <= 0 {
            return Err(RagSnapshotValidationError::InvalidSourceEventSequence);
        }
        if !valid_policy_token(&self.source_type, 128)
            || !valid_policy_token(&self.copyright_status, 128)
            || !valid_policy_token(&self.allowed_use, 128)
            || !bounded_nonblank(&self.embedding_model, 256)
        {
            return Err(RagSnapshotValidationError::InvalidMetadata);
        }
        if !valid_content(&self.content) {
            return Err(RagSnapshotValidationError::InvalidContent);
        }
        if self.embedding.is_empty()
            || self.embedding.len() > 4_096
            || self
                .embedding
                .iter()
                .any(|component| !component.is_finite())
            || self.embedding.iter().all(|component| *component == 0.0)
        {
            return Err(RagSnapshotValidationError::InvalidEmbedding);
        }
        Ok(())
    }

    pub fn content_hash(&self) -> String {
        canonical_content_hash(&self.content)
    }

    /// SHA-256 over the exact big-endian IEEE-754 components persisted by
    /// pgvector (the vector protocol header is deliberately excluded).
    pub fn embedding_hash(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut digest = Sha256::new();
        for component in &self.embedding {
            digest.update(component.to_bits().to_be_bytes());
        }
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RagSnapshotChunk {
    campaign_id: String,
    snapshot_id: String,
    chunk_id: String,
    source_event_sequence: i64,
    derivation_event_sequence: i64,
    source_type: String,
    visibility: String,
    visibility_subject: String,
    copyright_status: String,
    version: i64,
    owner: String,
    allowed_use: String,
    fact_provenance: RagFactProvenance,
    chunk_hash: String,
    content: String,
    embedding_model: String,
}

pub(crate) struct PersistedRagSnapshotChunk {
    pub campaign_id: String,
    pub snapshot_id: String,
    pub chunk_id: String,
    pub source_event_sequence: i64,
    pub derivation_event_sequence: i64,
    pub source_type: String,
    pub visibility: String,
    pub visibility_subject: String,
    pub copyright_status: String,
    pub version: i64,
    pub owner: String,
    pub allowed_use: String,
    pub fact_provenance: RagFactProvenance,
    pub chunk_hash: String,
    pub content: String,
    pub embedding_model: String,
}

impl RagSnapshotChunk {
    pub(crate) fn from_persisted(
        persisted: PersistedRagSnapshotChunk,
    ) -> Result<Self, RagSnapshotValidationError> {
        validate_snapshot_identity(&persisted.campaign_id, &persisted.snapshot_id)?;
        if !valid_identifier(&persisted.chunk_id) {
            return Err(RagSnapshotValidationError::InvalidChunkId);
        }
        if persisted.source_event_sequence <= 0 || persisted.derivation_event_sequence <= 0 {
            return Err(RagSnapshotValidationError::InvalidSourceEventSequence);
        }
        if !valid_policy_token(&persisted.source_type, 128)
            || !valid_policy_token(&persisted.visibility, 128)
            || !bounded_nonblank(&persisted.visibility_subject, 160)
            || !valid_policy_token(&persisted.copyright_status, 128)
            || persisted.version <= 0
            || !bounded_nonblank(&persisted.owner, 160)
            || !valid_policy_token(&persisted.allowed_use, 128)
            || !bounded_nonblank(&persisted.embedding_model, 256)
        {
            return Err(RagSnapshotValidationError::InvalidMetadata);
        }
        persisted.fact_provenance.validate()?;
        if !valid_content(&persisted.content) {
            return Err(RagSnapshotValidationError::InvalidContent);
        }
        if persisted.chunk_hash != canonical_content_hash(&persisted.content) {
            return Err(RagSnapshotValidationError::InvalidChunkHash);
        }
        Ok(Self {
            campaign_id: persisted.campaign_id,
            snapshot_id: persisted.snapshot_id,
            chunk_id: persisted.chunk_id,
            source_event_sequence: persisted.source_event_sequence,
            derivation_event_sequence: persisted.derivation_event_sequence,
            source_type: persisted.source_type,
            visibility: persisted.visibility,
            visibility_subject: persisted.visibility_subject,
            copyright_status: persisted.copyright_status,
            version: persisted.version,
            owner: persisted.owner,
            allowed_use: persisted.allowed_use,
            fact_provenance: persisted.fact_provenance,
            chunk_hash: persisted.chunk_hash,
            content: persisted.content,
            embedding_model: persisted.embedding_model,
        })
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub fn chunk_id(&self) -> &str {
        &self.chunk_id
    }

    pub const fn source_event_sequence(&self) -> i64 {
        self.source_event_sequence
    }

    pub const fn derivation_event_sequence(&self) -> i64 {
        self.derivation_event_sequence
    }

    pub fn source_type(&self) -> &str {
        &self.source_type
    }

    pub fn visibility(&self) -> &str {
        &self.visibility
    }

    pub fn visibility_subject(&self) -> &str {
        &self.visibility_subject
    }

    pub fn copyright_status(&self) -> &str {
        &self.copyright_status
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn allowed_use(&self) -> &str {
        &self.allowed_use
    }

    pub fn fact_provenance(&self) -> &RagFactProvenance {
        &self.fact_provenance
    }

    pub fn chunk_hash(&self) -> &str {
        &self.chunk_hash
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn embedding_model(&self) -> &str {
        &self.embedding_model
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RagSearchHit {
    pub chunk: RagSnapshotChunk,
    pub cosine_distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RagSnapshotValidationError {
    InvalidSnapshotId,
    InvalidCampaignId,
    InvalidChunkId,
    InvalidSourceEventSequence,
    InvalidMetadata,
    InvalidFactProvenance,
    InvalidChunkHash,
    InvalidContent,
    InvalidEmbedding,
    EmptySnapshot,
    TooManyChunks,
    InconsistentEmbeddingDimensions,
    InconsistentEmbeddingModel,
    InvalidSearchLimit,
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn valid_content(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1_048_576
}

fn canonical_content_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn bounded_nonblank(value: &str, maximum_length: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum_length
}

fn valid_policy_token(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

pub fn validate_snapshot_identity(
    campaign_id: &str,
    snapshot_id: &str,
) -> Result<(), RagSnapshotValidationError> {
    if !valid_identifier(campaign_id) {
        return Err(RagSnapshotValidationError::InvalidCampaignId);
    }
    if !valid_identifier(snapshot_id) {
        return Err(RagSnapshotValidationError::InvalidSnapshotId);
    }
    Ok(())
}
