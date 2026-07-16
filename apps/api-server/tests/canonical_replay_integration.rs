use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use api_server::ApiApplication;
use trpg_contracts::HttpRequest;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::AuthorityMode;

const IDENTITY_KEY: [u8; 32] = [0x44; 32];
const CANONICAL_KEY: [u8; 32] = [0x62; 32];

#[test]
fn authenticated_transport_filters_canonical_replay_by_live_campaign_membership() {
    let (Ok(primary_url), Ok(witness_url)) = (
        env::var("P02_API_CANONICAL_DATABASE_URL"),
        env::var("P02_API_CANONICAL_WITNESS_DATABASE_URL"),
    ) else {
        eprintln!("skipped: set independent P02 API canonical primary/witness databases");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(PostgresCanonicalStore::connect(
            &primary_url,
            &witness_url,
            "api-replay-test-key",
            &CANONICAL_KEY,
        ))
        .unwrap();
    runtime.block_on(store.prepare_for_service()).unwrap();

    runtime
        .block_on(store.commit(&draft("api_replay_public", 0, "public", "not_applicable")))
        .unwrap();
    runtime
        .block_on(store.commit(&draft(
            "api_replay_keeper",
            1,
            "keeper_only",
            "not_applicable",
        )))
        .unwrap();
    runtime
        .block_on(store.commit(&draft(
            "api_replay_private",
            2,
            "private_to_player",
            "player_replay",
        )))
        .unwrap();

    let now = now_unix_ms();
    let mut identity = IdentityService::new(&IDENTITY_KEY, 60_000).unwrap();
    identity
        .create_user(
            "keeper_replay",
            "keeper-replay@example.test",
            "keeper replay password long enough",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    identity
        .create_user(
            "player_replay",
            "player-replay@example.test",
            "player replay password long enough",
            GlobalRole::User,
        )
        .unwrap();
    let keeper_session = identity
        .login(
            "keeper-replay@example.test",
            "keeper replay password long enough",
            now,
        )
        .unwrap();
    let keeper_token = keeper_session.token.expose().to_owned();
    let keeper = identity
        .authenticate_session(Some(&keeper_token), now + 1)
        .unwrap();
    identity
        .grant_membership(
            &keeper,
            "campaign_api_replay",
            "keeper_replay",
            CampaignRole::HumanKeeper,
            now + 1,
        )
        .unwrap();
    identity
        .grant_membership(
            &keeper,
            "campaign_api_replay",
            "player_replay",
            CampaignRole::Player,
            now + 1,
        )
        .unwrap();
    identity
        .register_authority_contract(
            &keeper,
            trpg_test_support::authority_contract_with_owner(
                "campaign_api_replay",
                AuthorityMode::HumanKp,
                "keeper_replay",
                1,
            )
            .unwrap(),
            now + 1,
        )
        .unwrap();
    let player_session = identity
        .login(
            "player-replay@example.test",
            "player replay password long enough",
            now + 2,
        )
        .unwrap();
    let player_token = player_session.token.expose().to_owned();

    let audit_path = PathBuf::from(format!(
        "/tmp/p02-api-replay-audit-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&audit_path);
    let _ = std::fs::remove_file(audit_path.with_extension("jsonl.head"));
    let audit = FileAuditLog::open(&audit_path, "api-replay-audit", &[0x75; 32]).unwrap();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            "127.0.0.1:1".parse().unwrap(),
            "/stores/unavailable/check",
            PolicyBackend::OpenFga,
            "unused-model",
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            "127.0.0.1:2".parse().unwrap(),
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            "unused-revision",
        )
        .unwrap(),
    )
    .unwrap();
    let application =
        ApiApplication::new_production_governed(identity, policy, audit, runtime, store);
    let player_response = application
        .handle(&request(
            "/campaigns/campaign_api_replay/events?after_sequence=0&limit=100",
            Some(&player_token),
        ))
        .unwrap();
    assert_eq!(player_response.status, 200);
    let player_events = player_response.body["events"].as_array().unwrap();
    assert_eq!(player_events.len(), 2);
    assert_eq!(player_events[0]["stream_id"], "campaign_api_replay");
    assert_eq!(player_events[0]["request_hash_source"], "formal_commit");
    assert_eq!(player_events[0]["integrity_status"], "verified_hmac");
    assert_eq!(player_events[0]["visibility_label"], "public");
    assert_eq!(player_events[1]["visibility_label"], "private_to_player");

    let keeper_response = application
        .handle(&request(
            "/campaigns/campaign_api_replay/events?after_sequence=0&limit=100",
            Some(&keeper_token),
        ))
        .unwrap();
    assert_eq!(keeper_response.status, 200);
    assert_eq!(keeper_response.body["events"].as_array().unwrap().len(), 3);

    let unauthenticated = application
        .handle(&request("/campaigns/campaign_api_replay/events", None))
        .unwrap();
    assert_eq!(unauthenticated.status, 401);

    let logout = application
        .handle(&HttpRequest {
            method: "POST".to_owned(),
            path: "/auth/logout".to_owned(),
            headers: authorization_headers(&player_token),
            body: Vec::new(),
        })
        .unwrap();
    assert_eq!(logout.status, 204);
    let revoked = application
        .handle(&request(
            "/campaigns/campaign_api_replay/events",
            Some(&player_token),
        ))
        .unwrap();
    assert_eq!(revoked.status, 401);
}

fn draft(
    commit_id: &str,
    expected_version: i64,
    visibility_label: &str,
    visibility_subject: &str,
) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: commit_id.to_owned(),
        campaign_id: "campaign_api_replay".to_owned(),
        stream_id: "campaign_api_replay".to_owned(),
        idempotency_key: format!("idempotency_{commit_id}"),
        expected_version,
        command_id: format!("command_{commit_id}"),
        authenticated_actor_id: "workflow_api_replay".to_owned(),
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_campaign_api_replay_1".to_owned(),
        authority_owner: "keeper_replay".to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: visibility_subject.to_owned(),
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_{commit_id}"),
        provenance_recorded_by: "rules_engine_api_replay".to_owned(),
        correlation_id: format!("correlation_{commit_id}"),
        causation_id: format!("causation_{commit_id}"),
        trace_id: format!("trace_{commit_id}"),
        events: vec![CanonicalEventDraft {
            event_type: "ReplayProbeRecorded".to_owned(),
            payload_json: format!(r#"{{"commit":"{commit_id}"}}"#),
        }],
        audit: PolicyAuditDraft {
            actor_id: "keeper_replay".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_api_replay".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_api_replay".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("fga_{commit_id}"),
            openfga_policy_revision: "fga_api_replay".to_owned(),
            opa_decision_id: format!("opa_{commit_id}"),
            opa_policy_revision: "opa_api_replay".to_owned(),
        },
    }
}

fn request(path: &str, token: Option<&str>) -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        path: path.to_owned(),
        headers: token.map_or_else(HashMap::new, authorization_headers),
        body: Vec::new(),
    }
}

fn authorization_headers(token: &str) -> HashMap<String, String> {
    HashMap::from([("authorization".to_owned(), format!("Bearer {token}"))])
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
