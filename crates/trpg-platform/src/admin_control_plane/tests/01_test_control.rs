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
                local_provider_network_policy: LocalProviderNetworkPolicy::loopback_only(),
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
