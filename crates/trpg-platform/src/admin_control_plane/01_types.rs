use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trpg_identity::{
    AuthenticationContext, CampaignRole, GlobalRole, IdentityError, IdentityService,
    PrincipalKind,
};
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretManager, SecretReference, SecretValue,
};
use trpg_security_governance::tamper_evident_audit::{
    AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog,
};
use trpg_security_governance::{
    validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint,
};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
};

const ADMIN_STATE_SCHEMA: &str = "trpg-admin-control-v1";
const ADMIN_AUDIT_POLICY: &str = "admin-bootstrap-ops-v1";
const MAX_REQUEST_BODY_BYTES: usize = 32 * 1024;
const MAX_RECEIPTS: usize = 1024;
const LOCK_RETRIES: usize = 500;
const TUTORIAL_RULESET_VERSION: &str = "coc7_rules_1";
const TUTORIAL_HOUSE_RULES_VERSION: &str = "coc7_house_rules_none_1";
const TUTORIAL_SCENARIO_VERSION: &str = "tutorial_mist_archive_0_1_0";
const TUTORIAL_PROMPT_VERSION: &str = "tutorial_prompt_1";
const TUTORIAL_AGENT_PACK_VERSION: &str = "tutorial_agent_pack_1";
const TUTORIAL_TOOL_SCHEMA_VERSION: &str = "tutorial_tool_schema_1";
const TUTORIAL_SAFETY_PROFILE_VERSION: &str = "tutorial_safety_profile_1";
const TUTORIAL_CHARACTER_TEMPLATE_VERSION: &str = "coc7_investigator_1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminHttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl AdminHttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdminHttpResponse {
    pub status: u16,
    pub body: Value,
}

impl AdminHttpResponse {
    pub fn error(status: u16, code: &str) -> Self {
        Self {
            status,
            body: json!({"error": code}),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminOperationEvidence {
    pub code: String,
    pub artifact_reference: Option<String>,
    pub digest: Option<String>,
}

impl AdminOperationEvidence {
    pub fn completed(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            artifact_reference: None,
            digest: None,
        }
    }
}

pub trait AdminOperations: Send + Sync {
    fn provision_workflow_policy(
        &self,
        campaign_id: &str,
    ) -> Result<AdminOperationEvidence, String>;

    fn probe_provider(
        &self,
        configuration: &AdminProviderConfiguration,
        credential: &str,
    ) -> Result<AdminOperationEvidence, String>;

    fn request_model_certification(
        &self,
        request: &AdminModelCertificationRequest,
    ) -> Result<AdminOperationEvidence, String>;

    fn create_backup(
        &self,
        request: &AdminBackupRequest,
    ) -> Result<AdminOperationEvidence, String>;

    fn restore_backup(
        &self,
        request: &AdminRestoreRequest,
    ) -> Result<AdminOperationEvidence, String>;

    fn diagnostics(&self) -> Result<AdminOperationEvidence, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminProviderConfiguration {
    pub provider_type: String,
    pub base_url: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub credential_secret_id: String,
    pub credential_secret_version: u64,
    pub security_snapshot_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapAccountRequest {
    user_id: String,
    login: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapCompleteRequest {
    administrator: BootstrapAccountRequest,
    business_account: BootstrapAccountRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapTutorialAuthorityRequest {
    campaign_id: String,
    contract_id: String,
    created_at_unix_ms: u64,
    ai_provider_snapshot: String,
    model_route_snapshot: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionLoginRequest {
    login: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderConfigureRequest {
    provider_type: String,
    base_url: String,
    model_id: String,
    model_artifact_sha256: String,
    credential_secret_id: String,
    credential_secret_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminModelCertificationRequest {
    pub request_id: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminBackupRequest {
    pub backup_id: String,
    pub schema_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminRestoreRequest {
    pub manifest_path: String,
    pub expected_schema_version: String,
    pub safety_point_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminReceipt {
    action: String,
    descriptor: String,
    result_code: String,
    resource_id: String,
    #[serde(default)]
    response_fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminState {
    schema_version: String,
    version: u64,
    bootstrap_token_consumed: bool,
    administrator_user_id: Option<String>,
    administrator_login: Option<String>,
    business_user_id: Option<String>,
    business_login: Option<String>,
    provider: Option<AdminProviderConfiguration>,
    last_backup_reference: Option<String>,
    last_restore_reference: Option<String>,
    receipts: BTreeMap<String, AdminReceipt>,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            schema_version: ADMIN_STATE_SCHEMA.to_owned(),
            version: 0,
            bootstrap_token_consumed: false,
            administrator_user_id: None,
            administrator_login: None,
            business_user_id: None,
            business_login: None,
            provider: None,
            last_backup_reference: None,
            last_restore_reference: None,
            receipts: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MutationMetadata {
    idempotency_key: String,
    expected_version: u64,
    correlation_id: String,
    causation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AdminActor {
    actor_id: String,
    authentication_reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminControlPlaneError {
    Configuration(&'static str),
    Persistence(&'static str),
    InvalidRequest(&'static str),
    Authentication(&'static str),
    Authorization(&'static str),
    Conflict(&'static str),
    OperationUnavailable(String),
    AuditIntegrity,
}

impl AdminControlPlaneError {
    pub const fn code(&self) -> &str {
        match self {
            Self::Configuration(code)
            | Self::Persistence(code)
            | Self::InvalidRequest(code)
            | Self::Authentication(code)
            | Self::Authorization(code)
            | Self::Conflict(code) => code,
            Self::OperationUnavailable(_) => "ADMIN_OPERATION_UNAVAILABLE",
            Self::AuditIntegrity => "ADMIN_AUDIT_INTEGRITY_VIOLATION",
        }
    }

    const fn http_status(&self) -> u16 {
        match self {
            Self::Authentication(_) => 401,
            Self::Authorization(_) => 403,
            Self::Conflict(_) => 409,
            Self::InvalidRequest(_) => 400,
            Self::OperationUnavailable(_) => 503,
            Self::Configuration(_) | Self::Persistence(_) | Self::AuditIntegrity => 500,
        }
    }
}

impl fmt::Display for AdminControlPlaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdminControlPlaneError {}

pub struct AdminControlPlane {
    state_path: PathBuf,
    bootstrap_token: SecretValue,
    identity: IdentityService,
    secret_manager: Arc<SecretManager<MountedFileSecretResolver>>,
    audit: FileAuditLog,
    operations: Arc<dyn AdminOperations>,
}
