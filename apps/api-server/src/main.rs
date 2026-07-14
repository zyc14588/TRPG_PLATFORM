use std::process::ExitCode;

use api_server::ApiApplication;
use trpg_contracts::{run_service_with_handler, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_identity::IdentityService;
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;

fn main() -> ExitCode {
    let database_url = match std::env::var("TRPG_DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            eprintln!("service=api-server error=TRPG_DATABASE_URL_REQUIRED");
            return ExitCode::FAILURE;
        }
    };
    let signing_key = match std::env::var("TRPG_IDENTITY_SIGNING_KEY_HEX")
        .ok()
        .and_then(|value| decode_signing_key(&value))
    {
        Some(key) => key,
        None => {
            eprintln!("service=api-server error=IDENTITY_SIGNING_KEY_INVALID");
            return ExitCode::FAILURE;
        }
    };
    let session_ttl_ms = std::env::var("TRPG_IDENTITY_SESSION_TTL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8 * 60 * 60 * 1_000);
    let identity = match IdentityService::from_postgres(&database_url, &signing_key, session_ttl_ms)
    {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("service=api-server error={}", error.code());
            return ExitCode::FAILURE;
        }
    };
    let (policy, audit) = match policy_and_audit_from_environment() {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("service=api-server error={error}");
            return ExitCode::FAILURE;
        }
    };
    let application = ApiApplication::new_governed(identity, policy, audit);
    let readiness_application = application.clone();
    run(
        ServiceKind::ApiServer,
        RoleRuntimeProbe::spawn("api_runtime", move || {
            trpg_api::contract_core::validate_primary_adapter_boundaries()
                .map_err(|error| error.code().to_owned())?;
            readiness_application.readiness()
        }),
        application,
    )
}

fn decode_signing_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        key[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(key)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn policy_and_audit_from_environment(
) -> Result<(OpenFgaOpaPolicyAdapter, FileAuditLog), &'static str> {
    let openfga_address = required_environment("TRPG_OPENFGA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPENFGA_ADDRESS_INVALID")?;
    let openfga_store_id = required_environment("TRPG_OPENFGA_STORE_ID")?;
    let openfga_model_id = required_environment("TRPG_OPENFGA_MODEL_ID")?;
    let opa_address = required_environment("TRPG_OPA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPA_ADDRESS_INVALID")?;
    let opa_revision = required_environment("TRPG_OPA_POLICY_REVISION")?;
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store_id}/check"),
            PolicyBackend::OpenFga,
            openfga_model_id,
        )
        .map_err(|_| "OPENFGA_POLICY_CONFIGURATION_INVALID")?,
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/allow",
            PolicyBackend::Opa,
            opa_revision,
        )
        .map_err(|_| "OPA_POLICY_CONFIGURATION_INVALID")?,
    )
    .map_err(|_| "POLICY_CONFIGURATION_INVALID")?;

    let audit_path = required_environment("TRPG_AUDIT_LOG_PATH")?;
    let audit_key_id = required_environment("TRPG_AUDIT_HMAC_KEY_ID")?;
    let audit_key = required_environment("TRPG_AUDIT_HMAC_KEY_HEX")
        .ok()
        .and_then(|value| decode_signing_key(&value))
        .ok_or("AUDIT_HMAC_KEY_INVALID")?;
    let audit = FileAuditLog::open(audit_path, audit_key_id, &audit_key)
        .map_err(|_| "AUDIT_LOG_CONFIGURATION_INVALID")?;
    Ok((policy, audit))
}

fn required_environment(name: &str) -> Result<String, &'static str> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or("REQUIRED_ENVIRONMENT_MISSING")
}

fn run(
    kind: ServiceKind,
    runtime: Result<RoleRuntimeProbe, trpg_contracts::ServiceError>,
    application: ApiApplication,
) -> ExitCode {
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    let spec = match ServiceSpec::from_environment(kind, env!("CARGO_PKG_VERSION")) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    match run_service_with_handler(
        spec,
        vec![runtime],
        Box::new(move |request| application.handle(request)),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            ExitCode::FAILURE
        }
    }
}
