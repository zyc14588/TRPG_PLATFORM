use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::agent_runtime::{AgentError, AgentResult};

type HmacSha256 = Hmac<Sha256>;
const MAX_CERTIFICATE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LocalModelLevel {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
}

impl LocalModelLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Level0 => "LOCAL_MODEL_LEVEL_0",
            Self::Level1 => "LOCAL_MODEL_LEVEL_1",
            Self::Level2 => "LOCAL_MODEL_LEVEL_2",
            Self::Level3 => "LOCAL_MODEL_LEVEL_3",
            Self::Level4 => "LOCAL_MODEL_LEVEL_4",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificationInput {
    pub model_id: String,
    pub json_schema_support: bool,
    pub tool_call_support: bool,
    pub visibility_tests_pass: bool,
    pub prompt_injection_tests_pass: bool,
    pub rules_eval_pass: bool,
    pub latency_ms: u64,
}

/// Computes an assessment only. A caller-created assessment is deliberately
/// not accepted by the AI Keeper gate; only a signed, registry-active
/// `LocalModelCertificate` can cross that boundary.
pub fn certify_local_model(input: &CertificationInput) -> LocalModelLevel {
    if input.json_schema_support
        && input.tool_call_support
        && input.visibility_tests_pass
        && input.prompt_injection_tests_pass
        && input.rules_eval_pass
        && input.latency_ms <= 2_000
    {
        LocalModelLevel::Level4
    } else if input.json_schema_support && input.tool_call_support && input.visibility_tests_pass {
        LocalModelLevel::Level3
    } else if input.json_schema_support || input.tool_call_support {
        LocalModelLevel::Level2
    } else if !input.model_id.trim().is_empty() {
        LocalModelLevel::Level1
    } else {
        LocalModelLevel::Level0
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalModelCertificate {
    certificate_id: String,
    model_id: String,
    model_artifact_sha256: String,
    suite_id: String,
    level: LocalModelLevel,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    signing_key_id: String,
    signature: String,
}

impl std::fmt::Debug for LocalModelCertificate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalModelCertificate")
            .field("certificate_id", &self.certificate_id)
            .field("model_id", &self.model_id)
            .field("model_artifact_sha256", &self.model_artifact_sha256)
            .field("suite_id", &self.suite_id)
            .field("level", &self.level)
            .field("issued_at_unix_ms", &self.issued_at_unix_ms)
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("signing_key_id", &self.signing_key_id)
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

impl LocalModelCertificate {
    pub fn certificate_id(&self) -> &str {
        &self.certificate_id
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn model_artifact_sha256(&self) -> &str {
        &self.model_artifact_sha256
    }

    pub const fn level(&self) -> LocalModelLevel {
        self.level
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum RegistryState {
    Active,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RegistryEntry {
    certificate: LocalModelCertificate,
    state: RegistryState,
    registry_mac: String,
}

/// Signing authority plus append-only durable registry. The HMAC key is
/// zeroized, certificate fields are model/artifact/suite/time bound, and every
/// registry state transition carries a second MAC so editing a registry file
/// cannot reactivate a revoked certificate.
pub struct LocalModelCertificationAuthority {
    signing_key_id: String,
    signing_key: Zeroizing<[u8; 32]>,
    registry_path: PathBuf,
}

impl std::fmt::Debug for LocalModelCertificationAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalModelCertificationAuthority")
            .field("signing_key_id", &self.signing_key_id)
            .field("signing_key", &"[REDACTED]")
            .field("registry_path", &self.registry_path)
            .finish()
    }
}
