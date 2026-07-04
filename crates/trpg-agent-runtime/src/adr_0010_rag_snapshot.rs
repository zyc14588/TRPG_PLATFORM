use crate::agent_runtime::{AgentError, AgentResult};
use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::{EntityId, PrincipalScope, TrpgError};

pub const PROMPT_ID: &str = "CODEX-0508-04-AI-AGENT-SYSTEM-f2ee9f2b79";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenRagSnapshot {
    pub snapshot_id: EntityId,
    pub embedding_model: String,
    pub source_event_count: usize,
    chunks: Vec<RagChunk>,
}

impl FrozenRagSnapshot {
    pub fn new(
        snapshot_id: impl Into<String>,
        embedding_model: impl Into<String>,
        source_event_count: usize,
        chunks: Vec<RagChunk>,
    ) -> AgentResult<Self> {
        let embedding_model = embedding_model.into();
        if embedding_model.trim().is_empty() || source_event_count == 0 {
            return Err(AgentError::Core(TrpgError::InvalidConfiguration(
                "rag snapshot requires embedding model and source events",
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

    pub fn can_rebuild_from_event_count(&self, event_count: usize) -> bool {
        event_count >= self.source_event_count
    }
}
