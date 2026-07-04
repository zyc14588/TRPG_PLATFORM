use crate::agent_runtime::{AgentError, AgentResult};
use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::{EntityId, PrincipalScope, TrpgError};

pub const PROMPT_ID: &str = "CODEX-0485-04-AI-AGENT-SYSTEM-962b774429";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RagSnapshotImpl {
    pub snapshot_id: EntityId,
    pub embedding_model: String,
    pub source_event_count: usize,
    chunks: Vec<RagChunk>,
}

impl RagSnapshotImpl {
    pub fn new(
        snapshot_id: impl Into<String>,
        embedding_model: impl Into<String>,
        source_event_count: usize,
        chunks: Vec<RagChunk>,
    ) -> AgentResult<Self> {
        let embedding_model = embedding_model.into();
        if embedding_model.trim().is_empty() {
            return Err(AgentError::Core(TrpgError::InvalidConfiguration(
                "embedding model is required",
            )));
        }
        if !chunks.iter().all(RagChunk::has_required_metadata) {
            return Err(AgentError::RagVisibilityScopeViolation);
        }

        Ok(Self {
            snapshot_id: EntityId::new(snapshot_id)?,
            embedding_model,
            source_event_count,
            chunks,
        })
    }

    pub fn visible_chunks(&self, principal: &PrincipalScope) -> Vec<RagChunk> {
        query_visible_chunks(&self.chunks, principal)
    }

    pub fn is_rebuildable_from_event_count(&self, event_count: usize) -> bool {
        event_count >= self.source_event_count
    }
}
