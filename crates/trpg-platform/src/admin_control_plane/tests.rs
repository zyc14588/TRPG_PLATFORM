use super::*;

use std::sync::atomic::{AtomicU64, Ordering};

const BOOTSTRAP_TOKEN: &str = "bootstrap-token-0123456789-unique";
const ADMIN_PASSWORD: &str = "admin-password-0123456789";
const BUSINESS_PASSWORD: &str = "business-password-012345";
const PROVIDER_CANARY: &str = "provider-secret-canary-never-persist";

#[derive(Default)]
struct TestOperations;

impl AdminOperations for TestOperations {
    fn provision_workflow_policy(
        &self,
        _campaign_id: &str,
    ) -> Result<AdminOperationEvidence, String> {
        Ok(AdminOperationEvidence::completed(
            "WORKFLOW_POLICY_CONFIGURED",
        ))
    }

    fn probe_provider(
        &self,
        _configuration: &AdminProviderConfiguration,
        credential: &str,
    ) -> Result<AdminOperationEvidence, String> {
        if credential != PROVIDER_CANARY {
            return Err("credential mismatch".to_owned());
        }
        Ok(AdminOperationEvidence {
            code: "PROVIDER_REACHABLE".to_owned(),
            artifact_reference: Some("provider-probe-evidence".to_owned()),
            digest: Some(format!("sha256:{}", "b".repeat(64))),
        })
    }

    fn request_model_certification(
        &self,
        request: &AdminModelCertificationRequest,
    ) -> Result<AdminOperationEvidence, String> {
        Ok(AdminOperationEvidence {
            code: "CERTIFICATION_REQUESTED".to_owned(),
            artifact_reference: Some(request.request_id.clone()),
            digest: None,
        })
    }

    fn create_backup(
        &self,
        request: &AdminBackupRequest,
    ) -> Result<AdminOperationEvidence, String> {
        Ok(AdminOperationEvidence {
            code: "BACKUP_CREATED".to_owned(),
            artifact_reference: Some(request.backup_id.clone()),
            digest: None,
        })
    }

    fn restore_backup(
        &self,
        request: &AdminRestoreRequest,
    ) -> Result<AdminOperationEvidence, String> {
        Ok(AdminOperationEvidence {
            code: "RESTORE_COMPLETED".to_owned(),
            artifact_reference: Some(request.safety_point_id.clone()),
            digest: None,
        })
    }

    fn diagnostics(&self) -> Result<AdminOperationEvidence, String> {
        Ok(AdminOperationEvidence::completed("DIAGNOSTICS_HEALTHY"))
    }
}

struct TestControl {
    control: AdminControlPlane,
    root: PathBuf,
}

impl TestControl {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "trpg-admin-control-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let secrets = root.join("secrets");
        fs::create_dir_all(&secrets).expect("create secret mount");
        set_private_permissions(&secrets, 0o700).expect("protect secret mount");
        write_secret(&secrets, "bootstrap_token", BOOTSTRAP_TOKEN.as_bytes());
        write_secret(&secrets, "provider_credential", PROVIDER_CANARY.as_bytes());
        let resolver = MountedFileSecretResolver::new(&secrets).expect("secret resolver");
        let secret_manager = Arc::new(SecretManager::new(resolver));
        let bootstrap_reference =
            SecretReference::mounted("bootstrap_token", 1).expect("bootstrap reference");
        secret_manager
            .register(&bootstrap_reference)
            .expect("register bootstrap secret");
        let bootstrap_token = secret_manager
            .resolve(&bootstrap_reference)
            .expect("resolve bootstrap secret");
        let state_path = root.join("state/admin.json");
        prepare_private_parent(&state_path).expect("prepare state path");
        let audit_path = root.join("audit/admin.jsonl");
        prepare_private_parent(&audit_path).expect("prepare audit path");
        let audit =
            FileAuditLog::open(audit_path, "test-audit-key", &[0x33; 32]).expect("audit log");
        let identity = IdentityService::new(&[0x44; 32], 60_000).expect("identity service");
        Self {
            control: AdminControlPlane {
                state_path,
                bootstrap_token,
                identity,
                secret_manager,
                audit,
                operations: Arc::new(TestOperations),
            },
            root,
        }
    }

    fn bootstrap(&mut self) -> String {
        let response = self.control.handle(request(
            "POST",
            "/admin/v1/bootstrap/complete",
            BOOTSTRAP_TOKEN,
            Some(0),
            json!({
                "administrator": {
                    "user_id": "server-owner-1",
                    "login": "owner@example.test",
                    "password": ADMIN_PASSWORD
                },
                "business_account": {
                    "user_id": "business-user-1",
                    "login": "business@example.test",
                    "password": BUSINESS_PASSWORD
                }
            }),
        ));
        assert_eq!(response.expect("bootstrap response").status, 201);
        self.login("owner@example.test", ADMIN_PASSWORD)
    }

    fn login(&mut self, login: &str, password: &str) -> String {
        let response = self
            .control
            .handle(request(
                "POST",
                "/admin/v1/sessions",
                "",
                None,
                json!({"login": login, "password": password}),
            ))
            .expect("login response");
        assert_eq!(response.status, 200);
        response.body["access_token"]
            .as_str()
            .expect("access token")
            .to_owned()
    }
}

impl Drop for TestControl {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bootstrap_token_is_one_time_and_creates_exactly_two_distinct_roles() {
    let mut fixture = TestControl::new();
    let denied = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/bootstrap/status",
            "wrong-bootstrap-token",
            None,
            Value::Null,
        ))
        .expect("denied response");
    assert_eq!(denied.status, 401);
    let owner_token = fixture.bootstrap();
    let replay = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/complete",
            BOOTSTRAP_TOKEN,
            Some(0),
            json!({
                "administrator": {
                    "user_id": "server-owner-1",
                    "login": "owner@example.test",
                    "password": ADMIN_PASSWORD
                },
                "business_account": {
                    "user_id": "business-user-1",
                    "login": "business@example.test",
                    "password": BUSINESS_PASSWORD
                }
            }),
        ))
        .expect("replay response");
    assert_eq!(replay.status, 401);
    assert_eq!(replay.body["error"], "BOOTSTRAP_TOKEN_CONSUMED");
    let status = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/bootstrap/status",
            &owner_token,
            None,
            Value::Null,
        ))
        .expect("status response");
    assert_eq!(status.status, 200);
    assert_eq!(status.body["administrator_count"], 1);
    assert_eq!(status.body["business_account_count"], 1);
}

#[test]
fn tutorial_authority_is_locked_to_the_business_keeper_and_replays() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let body = json!({
        "campaign_id": "tutorial_campaign",
        "contract_id": "tutorial_authority",
        "created_at_unix_ms": 1_700_000_000_000_u64,
        "ai_provider_snapshot": "tutorial_provider",
        "model_route_snapshot": "tutorial_route"
    });
    let created = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(1),
            body.clone(),
        ))
        .expect("tutorial authority response");
    assert_eq!(created.status, 201);
    assert_eq!(created.body["authority_mode"], "HUMAN_KP");
    assert_eq!(created.body["authority_owner"], "business-user-1");

    let campaign_id =
        trpg_shared_kernel::EntityId::new("tutorial_campaign").expect("campaign id");
    let contract = fixture
        .control
        .identity
        .authority_contract(&campaign_id)
        .expect("authority lookup")
        .expect("tutorial authority");
    assert!(contract.is_locked());
    assert_eq!(contract.change_policy(), trpg_shared_kernel::ChangePolicy::ForkOnly);
    assert_eq!(contract.authority_owner().as_str(), "business-user-1");
    assert_eq!(contract.snapshot().ruleset_version().as_str(), "coc7_rules_1");

    let replayed = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(2),
            body,
        ))
        .expect("tutorial authority replay");
    assert_eq!(replayed.status, 200);
    assert_eq!(replayed.body["replayed"], true);
    assert_eq!(replayed.body["state_version"], 2);
}

#[test]
fn server_owner_creates_users_and_forks_ai_authority_without_mutating_parent() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let tutorial = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(1),
            json!({
                "campaign_id": "tutorial_campaign",
                "contract_id": "tutorial_authority",
                "created_at_unix_ms": 1_700_000_000_000_u64,
                "ai_provider_snapshot": "tutorial_provider",
                "model_route_snapshot": "tutorial_route"
            }),
        ))
        .expect("tutorial authority response");
    assert_eq!(tutorial.status, 201);

    let user_body = json!({
        "user_id": "player-user-1",
        "login": "player@example.test",
        "password": "player-password-012345"
    });
    let created_user = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/users",
            &owner_token,
            Some(2),
            user_body.clone(),
        ))
        .expect("managed user response");
    assert_eq!(created_user.status, 201);
    assert_eq!(created_user.body["global_role"], "USER");
    let replayed_user = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/users",
            &owner_token,
            Some(3),
            user_body,
        ))
        .expect("managed user replay");
    assert_eq!(replayed_user.status, 200);
    assert_eq!(replayed_user.body["replayed"], true);

    let fork_body = json!({
        "parent_campaign_id": "tutorial_campaign",
        "child_campaign_id": "tutorial_campaign_ai",
        "authority_mode": "AI_KP",
        "authority_owner": "ai_keeper_tutorial",
        "campaign_manager_user_id": "player-user-1"
    });
    let forked = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(3),
            fork_body.clone(),
        ))
        .expect("authority fork response");
    assert_eq!(forked.status, 201);
    assert_eq!(forked.body["authority_mode"], "AI_KP");
    assert_eq!(forked.body["contract_id"], "authority_contract_tutorial_campaign_ai_1");

    let parent_id = EntityId::new("tutorial_campaign").expect("parent campaign id");
    let child_id = EntityId::new("tutorial_campaign_ai").expect("child campaign id");
    let parent = fixture
        .control
        .identity
        .authority_contract(&parent_id)
        .expect("parent lookup")
        .expect("parent authority");
    let child = fixture
        .control
        .identity
        .authority_contract(&child_id)
        .expect("child lookup")
        .expect("child authority");
    assert_eq!(parent.mode(), &AuthorityMode::HumanKp);
    assert_eq!(parent.authority_owner().as_str(), "business-user-1");
    assert_eq!(child.mode(), &AuthorityMode::AiKp);
    assert_eq!(child.authority_owner().as_str(), "ai_keeper_tutorial");
    assert_eq!(child.created_at_unix_ms(), parent.created_at_unix_ms() + 1);
    assert_eq!(child.snapshot(), parent.snapshot());

    let replayed_fork = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(4),
            fork_body,
        ))
        .expect("authority fork replay");
    assert_eq!(replayed_fork.status, 200);
    assert_eq!(replayed_fork.body["replayed"], true);

    let state = fs::read_to_string(fixture.root.join("state/admin.json"))
        .expect("read admin state");
    let audit = fs::read_to_string(fixture.root.join("audit/admin.jsonl"))
        .expect("read admin audit");
    assert!(!state.contains("player-password-012345"));
    assert!(!audit.contains("player-password-012345"));
}

#[test]
fn authority_fork_rejects_in_place_or_unprivileged_requests() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/bootstrap/tutorial-authority",
            &owner_token,
            Some(1),
            json!({
                "campaign_id": "tutorial_campaign",
                "contract_id": "tutorial_authority",
                "created_at_unix_ms": 1_700_000_000_000_u64,
                "ai_provider_snapshot": "tutorial_provider",
                "model_route_snapshot": "tutorial_route"
            }),
        ))
        .expect("tutorial authority response");
    let business_token = fixture
        .control
        .identity
        .login(
            "business@example.test",
            BUSINESS_PASSWORD,
            now_unix_ms().expect("test time"),
        )
        .expect("business login")
        .token
        .expose()
        .to_owned();
    let request_body = json!({
        "parent_campaign_id": "tutorial_campaign",
        "child_campaign_id": "tutorial_campaign",
        "authority_mode": "AI_KP",
        "authority_owner": "ai_keeper_tutorial",
        "campaign_manager_user_id": "business-user-1"
    });
    let denied = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &business_token,
            Some(2),
            request_body.clone(),
        ))
        .expect("unprivileged authority fork response");
    assert_eq!(denied.status, 403);
    let in_place = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/authority-forks",
            &owner_token,
            Some(2),
            request_body,
        ))
        .expect("in-place authority fork response");
    assert_eq!(in_place.status, 400);
    assert_eq!(in_place.body["error"], "AUTHORITY_FORK_REQUEST_INVALID");
}

#[test]
fn business_user_cannot_use_admin_operations() {
    let mut fixture = TestControl::new();
    fixture.bootstrap();
    let business_token = fixture
        .control
        .identity
        .login(
            "business@example.test",
            BUSINESS_PASSWORD,
            now_unix_ms().expect("test time"),
        )
        .expect("business login")
        .token
        .expose()
        .to_owned();
    let response = fixture
        .control
        .handle(request(
            "GET",
            "/admin/v1/diagnostics",
            &business_token,
            None,
            Value::Null,
        ))
        .expect("diagnostics response");
    assert_eq!(response.status, 403);
    assert_eq!(response.body["error"], "ADMIN_SERVER_OWNER_REQUIRED");
}

#[test]
fn provider_secret_is_reference_only_and_probe_is_idempotent() {
    let mut fixture = TestControl::new();
    let owner_token = fixture.bootstrap();
    let rejected = fixture
        .control
        .handle(request(
            "PUT",
            "/admin/v1/providers/configuration",
            &owner_token,
            Some(1),
            json!({
                "provider_type": "openai",
                "base_url": "https://provider.example.test/v1",
                "model_id": "model-1",
                "model_artifact_sha256": format!("sha256:{}", "a".repeat(64)),
                "credential_secret_id": "provider_credential",
                "credential_secret_version": 1,
                "api_key": PROVIDER_CANARY
            }),
        ))
        .expect("rejected response");
    assert_eq!(rejected.status, 400);
    let configured = fixture
        .control
        .handle(request(
            "PUT",
            "/admin/v1/providers/configuration",
            &owner_token,
            Some(1),
            json!({
                "provider_type": "openai",
                "base_url": "https://provider.example.test/v1",
                "model_id": "model-1",
                "model_artifact_sha256": format!("sha256:{}", "a".repeat(64)),
                "credential_secret_id": "provider_credential",
                "credential_secret_version": 1
            }),
        ))
        .expect("configure response");
    assert_eq!(configured.status, 200);
    let probed = fixture
        .control
        .handle(request(
            "POST",
            "/admin/v1/providers/probe",
            &owner_token,
            Some(2),
            Value::Null,
        ))
        .expect("probe response");
    assert_eq!(probed.body["result"], "PROVIDER_REACHABLE");
    let replayed = fixture
        .control
        .handle(request_with_key(
            "POST",
            "/admin/v1/providers/probe",
            &owner_token,
            Some(2),
            "key-POST-admin-v1-providers-probe",
            Value::Null,
        ))
        .expect("replayed probe");
    assert_eq!(replayed.body["replayed"], true);
    assert_eq!(
        replayed.body["artifact_reference"],
        probed.body["artifact_reference"]
    );
    assert_eq!(replayed.body["digest"], probed.body["digest"]);
    let persisted = read_tree_text(&fixture.root);
    assert!(!persisted.contains(PROVIDER_CANARY));
}

#[test]
fn openfga_policy_check_requires_an_explicit_allow_boolean() {
    assert!(openfga_check_allows(br#"{"allowed":true}"#));
    assert!(!openfga_check_allows(br#"{"allowed":false}"#));
    assert!(!openfga_check_allows(br#"{"result":true}"#));
}

fn request(
    method: &str,
    path: &str,
    bearer: &str,
    expected_version: Option<u64>,
    body: Value,
) -> AdminHttpRequest {
    request_with_key(
        method,
        path,
        bearer,
        expected_version,
        &format!(
            "key-{}-{}",
            method,
            path.trim_matches('/').replace('/', "-")
        ),
        body,
    )
}

fn request_with_key(
    method: &str,
    path: &str,
    bearer: &str,
    expected_version: Option<u64>,
    idempotency_key: &str,
    body: Value,
) -> AdminHttpRequest {
    let mut headers = HashMap::new();
    if !bearer.is_empty() {
        headers.insert("authorization".to_owned(), format!("Bearer {bearer}"));
    }
    if let Some(version) = expected_version {
        headers.insert("idempotency-key".to_owned(), idempotency_key.to_owned());
        headers.insert("x-expected-version".to_owned(), version.to_string());
        headers.insert("x-correlation-id".to_owned(), "test-correlation".to_owned());
        headers.insert("x-causation-id".to_owned(), "test-causation".to_owned());
    }
    AdminHttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body: if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).expect("encode request")
        },
    }
}

fn write_secret(root: &Path, id: &str, value: &[u8]) {
    let path = root.join(format!("{id}.v1"));
    fs::write(&path, value).expect("write secret");
    set_private_permissions(&path, 0o600).expect("protect secret");
}

fn read_tree_text(root: &Path) -> String {
    let mut output = String::new();
    for directory in ["state", "audit"] {
        let path = root.join(directory);
        for entry in fs::read_dir(path).expect("read evidence directory") {
            let path = entry.expect("directory entry").path();
            if path.is_file() {
                output.push_str(&fs::read_to_string(path).expect("read evidence file"));
            }
        }
    }
    output
}
