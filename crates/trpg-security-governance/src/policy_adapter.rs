use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trpg_shared_kernel::{KernelResult, TrpgError};

const MAX_POLICY_RESPONSE_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyAuthorizationRequest {
    pub actor_id: String,
    pub principal_role: String,
    pub campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub authority_mode: String,
    pub target_visibility: String,
    pub trace_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecisionEvidence {
    pub allowed: bool,
    pub decision_id: String,
    pub policy_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyEvidence {
    pub openfga: PolicyDecisionEvidence,
    pub opa: PolicyDecisionEvidence,
}

impl PolicyEvidence {
    pub fn validate(&self) -> KernelResult<()> {
        for decision in [&self.openfga, &self.opa] {
            if decision.decision_id.trim().is_empty() || decision.policy_revision.trim().is_empty()
            {
                return Err(TrpgError::PolicyEvidenceUntrusted);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyBackend {
    OpenFga,
    Opa,
}

#[derive(Clone, Debug)]
pub struct HttpPolicyEndpoint {
    address: SocketAddr,
    path: String,
    backend: PolicyBackend,
    policy_revision: String,
    timeout: Duration,
}

impl HttpPolicyEndpoint {
    pub fn new(
        address: SocketAddr,
        path: impl Into<String>,
        backend: PolicyBackend,
        policy_revision: impl Into<String>,
    ) -> KernelResult<Self> {
        let path = path.into();
        let policy_revision = policy_revision.into();
        if !address.ip().is_loopback()
            || !path.starts_with('/')
            || policy_revision.trim().is_empty()
        {
            return Err(TrpgError::InvalidConfiguration(
                "policy_endpoint_configuration_invalid",
            ));
        }
        Ok(Self {
            address,
            path,
            backend,
            policy_revision,
            timeout: Duration::from_secs(2),
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> KernelResult<Self> {
        if timeout.is_zero() {
            return Err(TrpgError::InvalidConfiguration(
                "policy_timeout_must_be_positive",
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    fn check(
        &self,
        request: &PolicyAuthorizationRequest,
        openfga_allowed: Option<bool>,
    ) -> KernelResult<PolicyDecisionEvidence> {
        let body = match self.backend {
            PolicyBackend::OpenFga => json!({
                "authorization_model_id": self.policy_revision,
                "tuple_key": {
                    "user": format!("principal:{}", request.actor_id),
                    "relation": format!("can_{}", request.action),
                    "object": format!("campaign:{}", request.campaign_id),
                },
                "context": {
                    "campaign_id": request.campaign_id,
                    "target_visibility": request.target_visibility,
                    "trace_id": request.trace_id,
                }
            }),
            PolicyBackend::Opa => json!({"input": {
                "actor_id": request.actor_id,
                "principal_role": request.principal_role,
                "campaign_id": request.campaign_id,
                "resource_type": request.resource_type,
                "resource_id": request.resource_id,
                "action": request.action,
                "authority_mode": request.authority_mode,
                "source_visibility": request.target_visibility,
                "target_output": target_output(&request.action),
                "trace_id": request.trace_id,
                "openfga_decision": if openfga_allowed == Some(true) { "PERMIT" } else { "DENY" },
            }}),
        };
        let response = post_json(self.address, &self.path, &body, self.timeout)?;
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let allowed = match self.backend {
            PolicyBackend::OpenFga => value.get("allowed").and_then(Value::as_bool),
            PolicyBackend::Opa => value.get("result").and_then(|result| {
                result
                    .get("allow")
                    .and_then(Value::as_bool)
                    .or_else(|| result.as_bool())
            }),
        }
        .ok_or(TrpgError::PolicyEvidenceUntrusted)?;

        let decision_id = decision_id(&value, &response.headers).unwrap_or_else(|| {
            response_decision_id(self.backend, &response.body, &request.trace_id)
        });
        let response_revision = response_policy_revision(&value);
        if response_revision
            .as_deref()
            .is_some_and(|revision| revision != self.policy_revision)
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }

        Ok(PolicyDecisionEvidence {
            allowed,
            decision_id,
            policy_revision: self.policy_revision.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct OpenFgaOpaPolicyAdapter {
    openfga: HttpPolicyEndpoint,
    opa: HttpPolicyEndpoint,
}

impl OpenFgaOpaPolicyAdapter {
    pub fn new(openfga: HttpPolicyEndpoint, opa: HttpPolicyEndpoint) -> KernelResult<Self> {
        if openfga.backend != PolicyBackend::OpenFga || opa.backend != PolicyBackend::Opa {
            return Err(TrpgError::InvalidConfiguration(
                "policy_backend_pair_invalid",
            ));
        }
        Ok(Self { openfga, opa })
    }

    pub(crate) fn evaluate(
        &self,
        request: &PolicyAuthorizationRequest,
    ) -> KernelResult<PolicyEvidence> {
        let openfga = normalize_policy_error(self.openfga.check(request, None))?;
        let opa = normalize_policy_error(self.opa.check(request, Some(openfga.allowed)))?;
        Ok(PolicyEvidence { openfga, opa })
    }

    pub(crate) fn revision_snapshot(&self) -> (&str, &str) {
        (&self.openfga.policy_revision, &self.opa.policy_revision)
    }
}

fn normalize_policy_error<T>(result: KernelResult<T>) -> KernelResult<T> {
    result.map_err(|error| match error {
        TrpgError::PolicyEvidenceUntrusted => TrpgError::PolicyEvidenceUntrusted,
        _ => TrpgError::PolicyUnavailable,
    })
}

struct HttpResponse {
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn post_json(
    address: SocketAddr,
    path: &str,
    body: &Value,
    timeout: Duration,
) -> KernelResult<HttpResponse> {
    let mut stream =
        TcpStream::connect_timeout(&address, timeout).map_err(|_| TrpgError::PolicyUnavailable)?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|_| TrpgError::PolicyUnavailable)?;
    let body = serde_json::to_vec(body).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .map_err(|_| TrpgError::PolicyUnavailable)?;

    let mut response = Vec::new();
    stream
        .take(MAX_POLICY_RESPONSE_BYTES)
        .read_to_end(&mut response)
        .map_err(|_| TrpgError::PolicyUnavailable)?;
    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(TrpgError::PolicyEvidenceUntrusted)?;
    let header_bytes = &response[..boundary];
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
    let mut lines = header_text.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or(TrpgError::PolicyEvidenceUntrusted)?;
    if !(200..300).contains(&status) {
        return Err(TrpgError::PolicyUnavailable);
    }
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Ok(HttpResponse {
        headers,
        body: response[boundary + 4..].to_vec(),
    })
}

fn decision_id(value: &Value, headers: &[(String, String)]) -> Option<String> {
    value
        .get("decision_id")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("result")
                .and_then(|result| result.get("decision_id"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
        .or_else(|| {
            headers
                .iter()
                .find(|(name, _)| name == "x-decision-id" || name == "x-request-id")
                .map(|(_, value)| value.clone())
        })
        .filter(|value| !value.trim().is_empty())
}

fn response_policy_revision(value: &Value) -> Option<String> {
    value
        .get("policy_revision")
        .or_else(|| value.get("revision"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("result")
                .and_then(|result| {
                    result
                        .get("policy_revision")
                        .or_else(|| result.get("revision"))
                })
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
}

fn target_output(action: &str) -> &'static str {
    match action {
        "export_player_report" => "player_export",
        "generate_party_summary" => "party_summary",
        "index_rag_chunk" => "rag_chunk",
        _ => "debug_log",
    }
}

fn response_decision_id(backend: PolicyBackend, body: &[u8], trace_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(match backend {
        PolicyBackend::OpenFga => b"openfga".as_slice(),
        PolicyBackend::Opa => b"opa".as_slice(),
    });
    digest.update(trace_id.as_bytes());
    digest.update(body);
    format!("response-sha256:{:x}", digest.finalize())
}
