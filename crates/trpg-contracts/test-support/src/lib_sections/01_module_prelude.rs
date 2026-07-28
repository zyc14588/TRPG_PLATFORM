use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use trpg_shared_kernel::{
    Actor, ActorRole, AgentClass, AuthenticatedCommandContext, AuthorityContract,
    AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft, CanonicalCommitKey,
    CanonicalCommitPort, CanonicalCommitReceipt, CanonicalCommitRequest, CanonicalCommittedEvent,
    CommandEnvelope, CommandMetadata, EntityId, FactProvenance, FormalWritePath, KernelResult,
    ProvenanceKind, ResourceRef, TrpgError, Visibility, VisibilityLabel, WorkloadRole,
};

const TEST_IDENTITY_SIGNING_KEY: [u8; 32] = [0x5a; 32];

#[derive(Debug, Default)]
struct TestCanonicalCommitPort {
    state: Mutex<TestCanonicalState>,
}

#[derive(Debug, Default)]
struct TestCanonicalState {
    stream_versions: HashMap<(String, String), u64>,
    next_sequence: u64,
    idempotency_results:
        HashMap<(String, String, String), (CanonicalCommitRequest, CanonicalCommitReceipt)>,
}

impl CanonicalCommitPort for TestCanonicalCommitPort {
    fn load_receipt(
        &self,
        key: &CanonicalCommitKey,
    ) -> KernelResult<Option<CanonicalCommitReceipt>> {
        let state = self
            .state
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let scope = (
            key.campaign_id.clone(),
            key.stream_id.clone(),
            key.idempotency_key.clone(),
        );
        let Some((request, receipt)) = state.idempotency_results.get(&scope) else {
            let actual_version = state
                .stream_versions
                .get(&(key.campaign_id.clone(), key.stream_id.clone()))
                .copied()
                .unwrap_or(0);
            if actual_version != key.expected_version {
                return Err(TrpgError::ExpectedVersionConflict {
                    expected: key.expected_version,
                    actual: actual_version,
                });
            }
            return Ok(None);
        };
        if request.commit_id != key.commit_id {
            return Err(TrpgError::DuplicateCommand);
        }
        Ok(Some(receipt.clone()))
    }

    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt> {
        if request.events.is_empty() {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let stream_scope = (
            request.campaign_id.clone(),
            request.audit.resource_id.clone(),
        );
        let idempotency_scope = (
            request.campaign_id.clone(),
            request.audit.resource_id.clone(),
            request.idempotency_key.clone(),
        );
        if let Some((original_request, original_receipt)) =
            state.idempotency_results.get(&idempotency_scope)
        {
            return if original_request == request {
                Ok(original_receipt.clone())
            } else {
                Err(TrpgError::DuplicateCommand)
            };
        }
        let actual_version = state
            .stream_versions
            .get(&stream_scope)
            .copied()
            .unwrap_or(0);
        if request.expected_version != actual_version {
            return Err(TrpgError::ExpectedVersionConflict {
                expected: request.expected_version,
                actual: actual_version,
            });
        }
        let event_count =
            u64::try_from(request.events.len()).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let first_stream_version = request
            .expected_version
            .checked_add(1)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        let last_stream_version = request
            .expected_version
            .checked_add(event_count)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        let first_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        let committed_events = request
            .events
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let offset =
                    u64::try_from(index).map_err(|_| TrpgError::AuditIntegrityViolation)?;
                Ok(CanonicalCommittedEvent {
                    sequence: first_sequence
                        .checked_add(offset)
                        .ok_or(TrpgError::AuditIntegrityViolation)?,
                    stream_version: first_stream_version
                        .checked_add(offset)
                        .ok_or(TrpgError::AuditIntegrityViolation)?,
                    event_type: event.event_type.clone(),
                    payload_json: event.payload_json.clone(),
                    command_id: request.command_id.clone(),
                    idempotency_key: format!("{}:{index:04}", request.idempotency_key),
                    occurred_at_unix_ms: first_sequence
                        .checked_add(offset)
                        .ok_or(TrpgError::AuditIntegrityViolation)?,
                    event_integrity_hash: format!("hmac-sha256:{:064x}", first_sequence + offset),
                })
            })
            .collect::<KernelResult<Vec<_>>>()?;
        let receipt = CanonicalCommitReceipt {
            first_stream_version,
            last_stream_version,
            events: committed_events,
        };
        state.next_sequence = state
            .next_sequence
            .checked_add(event_count)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        state
            .idempotency_results
            .insert(idempotency_scope, (request.clone(), receipt.clone()));
        state
            .stream_versions
            .insert(stream_scope, last_stream_version);
        Ok(receipt)
    }

    fn verify_receipt(
        &self,
        request: &CanonicalCommitRequest,
        receipt: &CanonicalCommitReceipt,
    ) -> KernelResult<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let scope = (
            request.campaign_id.clone(),
            request.audit.resource_id.clone(),
            request.idempotency_key.clone(),
        );
        match state.idempotency_results.get(&scope) {
            Some((stored_request, stored_receipt))
                if stored_request == request && stored_receipt == receipt =>
            {
                Ok(())
            }
            _ => Err(TrpgError::AuditIntegrityViolation),
        }
    }
}

pub fn test_canonical_commit_port() -> Arc<dyn CanonicalCommitPort> {
    Arc::new(TestCanonicalCommitPort::default())
}

#[derive(Debug)]
struct CorruptSecondEventReceiptPort {
    inner: Arc<dyn CanonicalCommitPort>,
}

impl CanonicalCommitPort for CorruptSecondEventReceiptPort {
    fn load_receipt(
        &self,
        key: &CanonicalCommitKey,
    ) -> KernelResult<Option<CanonicalCommitReceipt>> {
        self.inner.load_receipt(key)
    }

    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt> {
        let mut receipt = self.inner.commit(request)?;
        let second = receipt
            .events
            .get_mut(1)
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        second.payload_json = r#"{"corrupted":true}"#.to_owned();
        Ok(receipt)
    }

    fn verify_receipt(
        &self,
        request: &CanonicalCommitRequest,
        receipt: &CanonicalCommitReceipt,
    ) -> KernelResult<()> {
        self.inner.verify_receipt(request, receipt)
    }
}

/// Fault-injection adapter proving that consumers validate a complete formal
/// batch before exposing any locally materialized event.
pub fn corrupt_second_event_receipt_port() -> Arc<dyn CanonicalCommitPort> {
    Arc::new(CorruptSecondEventReceiptPort {
        inner: test_canonical_commit_port(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedPromptBinding {
    pub prompt_id: String,
    pub crate_name: String,
    pub rust_module: String,
    pub task_type: String,
    pub output_target: String,
}

pub fn normalized_prompt_bindings() -> Vec<NormalizedPromptBinding> {
    NORMALIZED_PROMPT_MAP
        .lines()
        .filter_map(|line| {
            let cells = line
                .split('|')
                .skip(1)
                .take(6)
                .map(clean_table_cell)
                .collect::<Vec<_>>();
            if cells.len() != 6 || !cells[0].starts_with("CODEX-") {
                return None;
            }
            Some(NormalizedPromptBinding {
                prompt_id: cells[0].clone(),
                crate_name: cells[2].clone(),
                rust_module: cells[3].clone(),
                task_type: cells[4].clone(),
                output_target: cells[5].clone(),
            })
        })
        .collect()
}

pub fn assert_normalized_prompt_binding(crate_name: &str, module_name: &str, prompt_id: &str) {
    let module_suffix = format!("::{module_name}");
    assert!(
        normalized_prompt_bindings().iter().any(|binding| {
            binding.prompt_id == prompt_id
                && binding.crate_name == crate_name
                && binding.rust_module.ends_with(&module_suffix)
                && binding.task_type == "product-code"
        }),
        "missing normalized product-code binding for {crate_name}::{module_name} -> {prompt_id}"
    );
}

pub fn normalized_product_modules(crate_name: &str) -> Vec<String> {
    let mut modules = normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| binding.crate_name == crate_name && binding.task_type == "product-code")
        .map(|binding| binding.rust_module)
        .collect::<Vec<_>>();
    modules.sort();
    modules.dedup();
    modules
}

pub fn normalized_prompt_id(crate_name: &str, module_name: &str) -> String {
    let output_target = format!("crates/{crate_name}/src/{module_name}.rs");
    let matches = normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| {
            binding.crate_name == crate_name
                && binding.task_type == "product-code"
                && binding.output_target == output_target
        })
        .map(|binding| binding.prompt_id)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one normalized source binding for {output_target}, got {matches:?}"
    );
    matches.into_iter().next().unwrap()
}

pub fn normalized_prompt_ids_for_module(crate_name: &str, module_name: &str) -> Vec<String> {
    let module_suffix = format!("::{module_name}");
    normalized_prompt_bindings()
        .into_iter()
        .filter(|binding| {
            binding.crate_name == crate_name
                && binding.task_type == "product-code"
                && binding.rust_module.ends_with(&module_suffix)
        })
        .map(|binding| binding.prompt_id)
        .collect()
}

pub fn assert_normalized_product_module(crate_name: &str, module_name: &str) {
    drop(normalized_prompt_id(crate_name, module_name));
}

pub fn assert_normalized_prompt_id_exists(prompt_id: &str) {
    assert!(
        normalized_prompt_bindings()
            .iter()
            .any(|binding| binding.prompt_id == prompt_id),
        "missing prompt ID from normalized execution map: {prompt_id}"
    );
}

fn clean_table_cell(cell: &str) -> String {
    cell.trim().trim_matches('`').to_owned()
}

pub fn governed_command<T>(
    payload: T,
    actor_role: ActorRole,
    authority_mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let campaign_id = match authority_mode {
        AuthorityMode::HumanKp => "camp_human_archive",
        AuthorityMode::AiKp => "camp_ai_harbor",
    };
    let contract = authority_contract(campaign_id, authority_mode.clone(), 1)
        .expect("valid fixture authority contract");
    governed_command_for_contract(&contract, payload, actor_role)
}
