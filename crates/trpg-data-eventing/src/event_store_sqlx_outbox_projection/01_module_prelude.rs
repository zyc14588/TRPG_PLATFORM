crate::define_data_event_module!(
    SqlxOutboxProjectionCommand,
    SqlxOutboxProjectionOperation,
    append_sqlx_outbox_projection_event,
    "event_store_sqlx_outbox_projection",
    "SqlxOutboxProjectionRecorded",
    "data_eventing.event_store_sqlx_outbox_projection.event_schema",
    crate::DataEventOperation::EventStoreAppend,
    [
        "event_outbox",
        "projection_view",
        "sqlx_transaction_boundary"
    ]
);

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};
use trpg_domain_core::command_cqrs::CommandAcceptedPayload;
use trpg_domain_core::{CommittedFactEvidence, PersistedFactEvidenceRecord};
use trpg_shared_kernel::{
    CanonicalCommitKey, CanonicalCommitPort, CanonicalCommitReceipt, CanonicalCommitRequest,
    CanonicalCommittedEvent, EntityId, EventActorOriginWire, FactProvenance, KernelResult,
    ProvenanceKind, TrpgError, Visibility,
};
use zeroize::{Zeroize, Zeroizing};

const GENESIS_HASH: &str =
    "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";
const ZERO_REQUEST_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const CANONICAL_IDEMPOTENCY_OPERATION: &str = "canonical_commit";
const CURRENT_EVENT_INTEGRITY_VERSION: i32 = 3;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadProtectionError {
    InvalidInput,
    Cryptography,
}

impl PayloadProtectionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "EVENT_PAYLOAD_PROTECTION_INVALID_INPUT",
            Self::Cryptography => "EVENT_PAYLOAD_PROTECTION_CRYPTOGRAPHY_ERROR",
        }
    }
}

impl fmt::Display for PayloadProtectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PayloadProtectionError {}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedPayload {
    algorithm: String,
    key_reference: String,
    nonce: String,
    ciphertext: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedPayloadEnvelope {
    protected_payload: ProtectedPayload,
}

/// AES-256-GCM field protection for canonical event/outbox payloads. The key
/// and decrypted bytes are zeroized and this type deliberately has no Debug or
/// Clone implementation.
pub struct PayloadCipher {
    key_reference: EntityId,
    key: Zeroizing<[u8; 32]>,
}

/// Zeroizing plaintext view with no Debug/Clone implementation.
pub struct DecryptedPayload(Zeroizing<Vec<u8>>);

/// Ciphertext metadata suitable for separate database columns. It contains no
/// plaintext and deliberately has no Debug implementation.
pub struct EncryptedPayload {
    envelope: serde_json::Value,
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
    key_reference: EntityId,
}

impl EncryptedPayload {
    pub fn envelope(&self) -> &serde_json::Value {
        &self.envelope
    }

    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    pub const fn nonce(&self) -> &[u8; 12] {
        &self.nonce
    }

    pub fn key_reference(&self) -> &EntityId {
        &self.key_reference
    }

    pub fn into_envelope(self) -> serde_json::Value {
        self.envelope
    }
}

impl DecryptedPayload {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl PayloadCipher {
    pub fn new(
        key_reference: impl Into<String>,
        key: &[u8],
    ) -> Result<Self, PayloadProtectionError> {
        if key.len() != 32 {
            return Err(PayloadProtectionError::InvalidInput);
        }
        let mut protected_key = Zeroizing::new([0_u8; 32]);
        protected_key.copy_from_slice(key);
        Ok(Self {
            key_reference: EntityId::new(key_reference)
                .map_err(|_| PayloadProtectionError::InvalidInput)?,
            key: protected_key,
        })
    }

    pub fn key_reference(&self) -> &EntityId {
        &self.key_reference
    }

    pub fn encrypt_json(
        &self,
        plaintext_json: &[u8],
        associated_fields: &[&str],
    ) -> Result<serde_json::Value, PayloadProtectionError> {
        self.encrypt_json_field(plaintext_json, associated_fields)
            .map(EncryptedPayload::into_envelope)
    }

    pub fn encrypt_json_field(
        &self,
        plaintext_json: &[u8],
        associated_fields: &[&str],
    ) -> Result<EncryptedPayload, PayloadProtectionError> {
        if plaintext_json.is_empty() || plaintext_json.len() > 1_048_576 {
            return Err(PayloadProtectionError::InvalidInput);
        }
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, self.key.as_slice())
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let key = aead::LessSafeKey::new(key);
        let random = SystemRandom::new();
        let mut nonce_bytes = [0_u8; 12];
        random
            .fill(&mut nonce_bytes)
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
        let aad = payload_associated_data(associated_fields)?;
        let mut ciphertext = plaintext_json.to_vec();
        key.seal_in_place_append_tag(nonce, aead::Aad::from(aad.as_slice()), &mut ciphertext)
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let envelope = ProtectedPayloadEnvelope {
            protected_payload: ProtectedPayload {
                algorithm: "AES-256-GCM".to_owned(),
                key_reference: self.key_reference.to_string(),
                nonce: BASE64.encode(nonce_bytes),
                ciphertext: BASE64.encode(&ciphertext),
            },
        };
        Ok(EncryptedPayload {
            envelope: serde_json::to_value(envelope)
                .map_err(|_| PayloadProtectionError::Cryptography)?,
            ciphertext,
            nonce: nonce_bytes,
            key_reference: self.key_reference.clone(),
        })
    }

    pub fn decrypt_json(
        &self,
        envelope: &serde_json::Value,
        associated_fields: &[&str],
    ) -> Result<DecryptedPayload, PayloadProtectionError> {
        let envelope: ProtectedPayloadEnvelope = serde_json::from_value(envelope.clone())
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let protected = envelope.protected_payload;
        if protected.algorithm != "AES-256-GCM"
            || protected.key_reference != self.key_reference.as_str()
        {
            return Err(PayloadProtectionError::Cryptography);
        }
        let nonce = BASE64
            .decode(protected.nonce)
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let nonce: [u8; 12] = nonce
            .try_into()
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let mut ciphertext = Zeroizing::new(
            BASE64
                .decode(protected.ciphertext)
                .map_err(|_| PayloadProtectionError::Cryptography)?,
        );
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, self.key.as_slice())
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let key = aead::LessSafeKey::new(key);
        let aad = payload_associated_data(associated_fields)?;
        let plaintext = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad.as_slice()),
                ciphertext.as_mut_slice(),
            )
            .map_err(|_| PayloadProtectionError::Cryptography)?;
        let plaintext_len = plaintext.len();
        ciphertext.truncate(plaintext_len);
        Ok(DecryptedPayload(ciphertext))
    }
}

impl Drop for PayloadCipher {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

fn payload_associated_data(fields: &[&str]) -> Result<Zeroizing<Vec<u8>>, PayloadProtectionError> {
    if fields.is_empty() || fields.iter().any(|field| field.len() > 65_536) {
        return Err(PayloadProtectionError::InvalidInput);
    }
    let mut result = Zeroizing::new(Vec::new());
    for field in fields {
        let length =
            u32::try_from(field.len()).map_err(|_| PayloadProtectionError::InvalidInput)?;
        result.extend_from_slice(&length.to_be_bytes());
        result.extend_from_slice(field.as_bytes());
    }
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEventDraft {
    pub event_type: String,
    pub payload_json: String,
    /// A narrowly scoped event-level visibility envelope. This is used by a
    /// keeper-authorized campaign fork when one atomic command materializes
    /// rows that retain different source visibility labels. The command audit
    /// remains keeper-only; each override is request-hash and HMAC protected.
    pub visibility: Option<CanonicalEventVisibility>,
    /// Exact read-model rows this event is allowed to insert or advance.
    /// The list is persisted beside the encrypted payload and covered by the
    /// versioned event HMAC; projection triggers reject every other row.
    pub projection_targets: Vec<CanonicalProjectionTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEventVisibility {
    pub label: String,
    pub subject: String,
    /// Payload-erasure subject for this event. Private player materializations
    /// must use the same player identifier as the visibility subject so their
    /// ciphertext is covered by that subject's crypto-erasure key.
    pub data_subject_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalProjectionTarget {
    pub relation: String,
    pub row_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedCanonicalProjectionTarget {
    relation: String,
    row_id: String,
    capability_hash: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RagChunkDerivationPayload {
    source_event_sequence: i64,
    snapshot_id: String,
    chunk_id: String,
    content_hash: String,
    source_type: String,
    copyright_status: String,
    allowed_use: String,
    embedding_model: String,
    embedding_dimensions: i32,
    embedding_hash: String,
}

#[derive(Default)]
struct RagDerivationFields {
    source_event_sequence: Option<i64>,
    snapshot_id: Option<String>,
    chunk_id: Option<String>,
    content_hash: Option<String>,
    source_type: Option<String>,
    copyright_status: Option<String>,
    allowed_use: Option<String>,
    embedding_model: Option<String>,
    embedding_dimensions: Option<i32>,
    embedding_hash: Option<String>,
}
