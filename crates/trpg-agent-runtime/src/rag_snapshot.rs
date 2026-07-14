use crate::agent_runtime::{AgentError, AgentResult};
use trpg_shared_kernel::{EntityId, PrincipalScope, TrpgError, Visibility};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RagChunk {
    pub chunk_id: EntityId,
    pub source_type: String,
    pub visibility: Visibility,
    pub version: String,
    pub allowed_use: String,
}

impl RagChunk {
    pub fn new(
        chunk_id: impl Into<String>,
        source_type: impl Into<String>,
        visibility: Visibility,
        version: impl Into<String>,
        allowed_use: impl Into<String>,
    ) -> Result<Self, TrpgError> {
        Ok(Self {
            chunk_id: EntityId::new(chunk_id)?,
            source_type: source_type.into(),
            visibility,
            version: version.into(),
            allowed_use: allowed_use.into(),
        })
    }

    pub fn has_required_metadata(&self) -> bool {
        !self.source_type.trim().is_empty()
            && !self.version.trim().is_empty()
            && !self.allowed_use.trim().is_empty()
    }
}

pub fn query_visible_chunks(chunks: &[RagChunk], principal: &PrincipalScope) -> Vec<RagChunk> {
    chunks
        .iter()
        .filter(|chunk| chunk.visibility.can_view(principal))
        .cloned()
        .collect()
}

pub fn require_visible_chunk<'a>(
    chunks: &'a [RagChunk],
    principal: &PrincipalScope,
    chunk_id: &str,
) -> AgentResult<&'a RagChunk> {
    chunks
        .iter()
        .find(|chunk| chunk.chunk_id.as_str() == chunk_id && chunk.visibility.can_view(principal))
        .ok_or(AgentError::RagVisibilityScopeViolation)
}
