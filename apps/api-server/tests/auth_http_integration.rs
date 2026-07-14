use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api_server::ApiApplication;
use serde_json::{json, Value};
use trpg_contracts::HttpRequest;
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::{AuditDecision, FileAuditLog};
use trpg_shared_kernel::AuthorityMode;

const KEY: [u8; 32] = [0x31; 32];
const AUDIT_KEY: [u8; 32] = [0x71; 32];

fn application() -> ApiApplication {
    let now = now_unix_ms();
    let mut identity = IdentityService::new(&KEY, 60_000).unwrap();
    identity
        .create_user(
            "owner_a",
            "owner@example.test",
            "owner password long enough",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    identity
        .create_user(
            "player_a",
            "player@example.test",
            "player password long enough",
            GlobalRole::User,
        )
        .unwrap();
    let owner_session = identity
        .login("owner@example.test", "owner password long enough", now)
        .unwrap();
    let owner = identity
        .authenticate_session(Some(owner_session.token.expose()), now + 1)
        .unwrap();
    identity
        .grant_membership(&owner, "campaign_a", "owner_a", CampaignRole::HumanKeeper)
        .unwrap();
    identity
        .grant_membership(&owner, "campaign_a", "player_a", CampaignRole::Player)
        .unwrap();
    let application = ApiApplication::new(identity);
    application
        .register_authority(
            trpg_test_support::authority_contract_with_owner(
                "campaign_a",
                AuthorityMode::HumanKp,
                "owner_a",
                1,
            )
            .unwrap(),
        )
        .unwrap();
    application
}

fn governed_application() -> Option<(ApiApplication, PathBuf)> {
    let openfga_address = std::env::var("P02_OPENFGA_ADDRESS").ok()?.parse().ok()?;
    let store_id = std::env::var("P02_OPENFGA_STORE_ID").ok()?;
    let model_id = std::env::var("P02_OPENFGA_MODEL_ID").ok()?;
    let opa_address = std::env::var("P02_OPA_ADDRESS").ok()?.parse().ok()?;
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{store_id}/check"),
            PolicyBackend::OpenFga,
            model_id,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/allow",
            PolicyBackend::Opa,
            "opa-security-governance-v1",
        )
        .unwrap(),
    )
    .unwrap();
    let path = std::env::temp_dir().join(format!(
        "p02-api-membership-audit-{}-{:?}.jsonl",
        std::process::id(),
        thread::current().id()
    ));
    let _ = std::fs::remove_file(&path);
    let audit = FileAuditLog::open(&path, "api-test-audit-v1", &AUDIT_KEY).unwrap();

    let now = now_unix_ms();
    let mut identity = IdentityService::new(&KEY, 60_000).unwrap();
    identity
        .create_user(
            "owner_a",
            "owner@example.test",
            "owner password long enough",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    identity
        .create_user(
            "player_a",
            "player@example.test",
            "player password long enough",
            GlobalRole::User,
        )
        .unwrap();
    let owner_session = identity
        .login("owner@example.test", "owner password long enough", now)
        .unwrap();
    let owner = identity
        .authenticate_session(Some(owner_session.token.expose()), now + 1)
        .unwrap();
    identity
        .grant_membership(&owner, "campaign_a", "owner_a", CampaignRole::HumanKeeper)
        .unwrap();
    identity
        .grant_membership(&owner, "campaign_a", "player_a", CampaignRole::Player)
        .unwrap();
    identity
        .register_authority_contract(
            trpg_test_support::authority_contract_with_owner(
                "campaign_a",
                AuthorityMode::HumanKp,
                "owner_a",
                1,
            )
            .unwrap(),
        )
        .unwrap();
    Some((ApiApplication::new_governed(identity, policy, audit), path))
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

fn exchange(application: ApiApplication, request: String) -> (u16, Value) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let request = read_request(&mut stream);
        let response = application.handle(&request).unwrap_or_else(|| {
            trpg_contracts::HttpResponse::json(404, json!({"error": "NOT_FOUND"}))
        });
        let body = response.body.to_string();
        write!(
            stream,
            "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.status,
            body.len(),
            body
        )
        .unwrap();
    });

    let mut stream = TcpStream::connect(address).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, serde_json::from_str(body).unwrap())
}

fn read_request(stream: &mut TcpStream) -> HttpRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let (boundary, content_length) = loop {
        let count = stream.read(&mut buffer).unwrap();
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..boundary]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if bytes.len() >= boundary + 4 + content_length {
                break (boundary, content_length);
            }
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..boundary]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap().split_whitespace().collect::<Vec<_>>();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<HashMap<_, _>>();
    HttpRequest {
        method: request_line[0].to_owned(),
        path: request_line[1].to_owned(),
        headers,
        body: bytes[boundary + 4..boundary + 4 + content_length].to_vec(),
    }
}

fn json_request(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> String {
    let body = body.map_or_else(String::new, |body| body.to_string());
    let authorization = token.map_or_else(String::new, |token| {
        format!("Authorization: Bearer {token}\r\n")
    });
    let content_type = if body.is_empty() {
        ""
    } else {
        "Content-Type: application/json\r\n"
    };
    format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}{content_type}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn http_authentication_and_campaign_authorization_fail_closed() {
    let application = application();
    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_a/authority", None, None),
    );
    assert_eq!(status, 401);

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "player@example.test",
                "password": "player password long enough",
                "role": "SERVER_OWNER"
            })),
        ),
    );
    assert_eq!(status, 400);
    assert_eq!(body["error"], "INVALID_JSON_BODY");

    let (status, body) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "player@example.test",
                "password": "player password long enough"
            })),
        ),
    );
    assert_eq!(status, 200);
    let token = body["access_token"].as_str().unwrap().to_owned();

    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_b/authority", Some(&token), None),
    );
    assert_eq!(status, 403);
    let (status, _) = exchange(
        application.clone(),
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(&token),
            Some(json!({"role": "CAMPAIGN_OWNER"})),
        ),
    );
    assert_eq!(status, 403);

    let (status, owner_login) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "owner@example.test",
                "password": "owner password long enough"
            })),
        ),
    );
    assert_eq!(status, 200);
    let owner_token = owner_login["access_token"].as_str().unwrap();
    let (status, body) = exchange(
        application.clone(),
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(owner_token),
            Some(json!({"role": "SPECTATOR"})),
        ),
    );
    assert_eq!(status, 503);
    assert_eq!(body["error"], "POLICY_UNAVAILABLE");

    let (status, body) = exchange(
        application,
        json_request("GET", "/campaigns/campaign_a/authority", Some(&token), None),
    );
    assert_eq!(status, 200);
    assert_eq!(body["authority_owner"], "owner_a");
    assert_eq!(body["change_policy"], "FORK_ONLY");
}

#[test]
fn refresh_rotates_session_and_logout_revokes_it() {
    let application = application();
    let (_, login) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "player@example.test",
                "password": "player password long enough"
            })),
        ),
    );
    let first = login["access_token"].as_str().unwrap().to_owned();
    let (status, refreshed) = exchange(
        application.clone(),
        json_request("POST", "/auth/refresh", Some(&first), None),
    );
    assert_eq!(status, 200);
    let second = refreshed["access_token"].as_str().unwrap().to_owned();
    assert_ne!(first, second);

    let (status, _) = exchange(
        application.clone(),
        json_request("GET", "/campaigns/campaign_a/authority", Some(&first), None),
    );
    assert_eq!(status, 401);
    let (status, _) = exchange(
        application.clone(),
        json_request("POST", "/auth/logout", Some(&second), None),
    );
    assert_eq!(status, 204);
    let (status, _) = exchange(
        application,
        json_request(
            "GET",
            "/campaigns/campaign_a/authority",
            Some(&second),
            None,
        ),
    );
    assert_eq!(status, 401);
}

#[test]
fn authorized_membership_mutation_requires_real_policy_and_is_audited() {
    let (application, audit_path) = governed_application()
        .expect("P02_OPENFGA_* and P02_OPA_ADDRESS must identify real policy services");
    let (status, login) = exchange(
        application.clone(),
        json_request(
            "POST",
            "/auth/login",
            None,
            Some(json!({
                "login": "owner@example.test",
                "password": "owner password long enough"
            })),
        ),
    );
    assert_eq!(status, 200);
    let owner_token = login["access_token"].as_str().unwrap();
    let (status, body) = exchange(
        application,
        json_request(
            "PUT",
            "/campaigns/campaign_a/memberships/player_a",
            Some(owner_token),
            Some(json!({"role": "SPECTATOR"})),
        ),
    );
    assert_eq!(status, 200, "unexpected policy response: {body}");
    assert_eq!(body["role"], "SPECTATOR");

    let records = FileAuditLog::open(&audit_path, "api-test-audit-v1", &AUDIT_KEY)
        .unwrap()
        .verify()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Permit);
    assert_eq!(records[0].actor_id, "owner_a");
    assert_eq!(records[0].action, "manage_campaign_membership");
    std::fs::remove_file(audit_path).unwrap();
}
