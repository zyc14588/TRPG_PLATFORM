use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, EventStore, FactProvenance, FactSource, ProvenanceKind,
};
use trpg_domain_core::visibility_fact_provenance::CommittedFactEvidence;
use trpg_security_governance::cloud_egress::{
    authorize_cloud_egress, CloudContextFact, CloudEgressDenial, CloudEgressOutcome,
    CloudEgressRequest, ConsentVisibilityScope, ProviderBoundary,
};
use trpg_security_governance::secret::SecretReference;
use trpg_security_governance::security_privacy::{
    CloudConsentGrant, PostgresCloudEgressLedger, PostgresDeletionRepository,
};
use trpg_shared_kernel::{EntityId, PrincipalScope, TrpgError, Visibility, VisibilityLabel};

fn id(value: impl Into<String>) -> EntityId {
    EntityId::new(value).unwrap()
}

fn assert_append_only_enforcement(error: sqlx::Error) {
    let database_error = error
        .as_database_error()
        .expect("append-only tampering must be rejected by PostgreSQL");
    match database_error.code().as_deref() {
        Some("P0001") => assert_eq!(
            database_error.message(),
            "cloud egress route and audit evidence is append-only"
        ),
        Some("42501") => {}
        code => panic!("unexpected append-only enforcement SQLSTATE: {code:?}"),
    }
}

async fn seed_consent_fixture(pool: &PgPool, grant: &CloudConsentGrant) -> Result<(), TrpgError> {
    let expires_at =
        i64::try_from(grant.expires_at_unix_ms).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
    sqlx::query(
        "INSERT INTO cloud_egress_consents (\
         consent_id, subject_id, target_provider, purpose, policy_version, \
         visibility_scope, granted, expires_at_unix_ms) \
         VALUES ($1, $2, $3, $4, $5, $6, true, $7) \
         ON CONFLICT (consent_id) DO NOTHING",
    )
    .bind(grant.consent_id.as_str())
    .bind(grant.subject_id.as_str())
    .bind(grant.target_provider.as_str())
    .bind(grant.purpose.as_str())
    .bind(grant.policy_version.as_str())
    .bind(grant.visibility_scope.as_str())
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|_| TrpgError::PolicyUnavailable)?;
    let exact: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM cloud_egress_consents \
         WHERE consent_id = $1 AND subject_id = $2 AND target_provider = $3 \
           AND purpose = $4 AND policy_version = $5 AND visibility_scope = $6 \
           AND granted AND expires_at_unix_ms = $7)",
    )
    .bind(grant.consent_id.as_str())
    .bind(grant.subject_id.as_str())
    .bind(grant.target_provider.as_str())
    .bind(grant.purpose.as_str())
    .bind(grant.policy_version.as_str())
    .bind(grant.visibility_scope.as_str())
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|_| TrpgError::PolicyUnavailable)?;
    exact
        .then_some(())
        .ok_or(TrpgError::PolicyEvidenceUntrusted)
}

async fn seed_notice_fixture(
    pool: &PgPool,
    notice_reference: &EntityId,
    subject_id: &EntityId,
    policy_version: &EntityId,
    document: &[u8],
) {
    let digest = format!("sha256:{:x}", Sha256::digest(document));
    sqlx::query(
        "INSERT INTO cloud_egress_notices (\
         notice_reference, subject_id, policy_version, notice_digest) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(notice_reference.as_str())
    .bind(subject_id.as_str())
    .bind(policy_version.as_str())
    .bind(digest)
    .execute(pool)
    .await
    .unwrap();
}

fn verified_cloud_fact(
    fact_id: &str,
    visibility: Visibility,
    content: Vec<u8>,
) -> CloudContextFact {
    let mut command = trpg_test_support::governed_command(
        "privacy cloud-context fixture",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = visibility;
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        format!("decision_{fact_id}"),
        "rules_engine_privacy_context",
    )
    .unwrap();
    let mut store = EventStore::default();
    let event = store
        .append(
            &command,
            "DecisionCommitted",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: FactSource::DecisionRecord,
                target_fact_id: fact_id.to_owned(),
            },
        )
        .unwrap();
    let evidence = CommittedFactEvidence::load(&store, event.sequence, fact_id).unwrap();
    CloudContextFact::from_committed_fact(&evidence, content).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persisted_consent_controls_route_snapshot_and_audit_across_revocation() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    PostgresDeletionRepository::new(pool.clone())
        .migrate()
        .await
        .expect("apply privacy and cloud egress schema");
    let ledger = PostgresCloudEgressLedger::new(pool.clone());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let nonce = now.to_string();
    let subject_id = id(format!("egress_subject_{nonce}"));
    let consent_id = id(format!("egress_consent_{nonce}"));
    let target_provider = id("cloud_provider");
    let purpose = id("gameplay_context");
    let policy_version = id("privacy_v1");
    let grant = CloudConsentGrant {
        consent_id: consent_id.clone(),
        subject_id: subject_id.clone(),
        target_provider: target_provider.clone(),
        purpose: purpose.clone(),
        policy_version: policy_version.clone(),
        visibility_scope: ConsentVisibilityScope::PublicOnly,
        expires_at_unix_ms: now + 60_000,
    };
    seed_consent_fixture(&pool, &grant).await.unwrap();
    seed_consent_fixture(&pool, &grant)
        .await
        .expect("an exact consent delivery retry is idempotent");
    let mut conflicting_grant = grant.clone();
    conflicting_grant.subject_id = id(format!("different_subject_{nonce}"));
    assert_eq!(
        seed_consent_fixture(&pool, &conflicting_grant)
            .await
            .unwrap_err(),
        TrpgError::PolicyEvidenceUntrusted
    );

    let snapshot_id = id(format!("route_allow_{nonce}"));
    let allow_notice = id(format!("notice_{nonce}"));
    seed_notice_fixture(
        &pool,
        &allow_notice,
        &subject_id,
        &policy_version,
        b"cloud processing notice v1",
    )
    .await;
    let outcome = authorize_cloud_egress(
        &ledger,
        CloudEgressRequest {
            snapshot_id: snapshot_id.clone(),
            audit_id: id(format!("audit_allow_{nonce}")),
            subject_id: subject_id.clone(),
            source_provider: id("ollama"),
            target_provider: target_provider.clone(),
            source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
            target_endpoint: "https://cloud.example.test/v1".to_owned(),
            model_id: id("cloud-model-v1"),
            source_credential: SecretReference::mounted("ollama_provider", 1).unwrap(),
            target_credential: SecretReference::kms("cloud_provider", 7).unwrap(),
            source_boundary: ProviderBoundary::Local,
            target_boundary: ProviderBoundary::Cloud,
            fallback_policy: id("explicit_audited_only"),
            privacy_boundary: id("explicit_consent_no_silent_fallback"),
            purpose: purpose.clone(),
            policy_version: policy_version.clone(),
            notice_reference: Some(allow_notice),
            target_audience: PrincipalScope::Player(subject_id.clone()),
            context: vec![verified_cloud_fact(
                &format!("public_fact_{nonce}"),
                Visibility::new(VisibilityLabel::Public),
                vec![b'x'; 96],
            )],
        },
    )
    .await
    .unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        panic!("durable consent must produce an audited authorization");
    };
    assert_eq!(authorization.snapshot_id(), &snapshot_id);
    let allowed_record = ledger.load_recorded_decision(&snapshot_id).await.unwrap();
    assert_eq!(allowed_record.0, "allow");
    assert_eq!(allowed_record.1, None);
    assert_eq!(allowed_record.2, authorization.context_manifest_hash());
    let route_tampering = sqlx::query(
        "UPDATE cloud_egress_route_snapshots SET purpose = 'tampered' WHERE snapshot_id = $1",
    )
    .bind(snapshot_id.as_str())
    .execute(&pool)
    .await
    .expect_err("route snapshot mutation must be rejected");
    assert_append_only_enforcement(route_tampering);

    sqlx::query(
        "UPDATE cloud_egress_consents SET granted = false, updated_at = now() \
         WHERE consent_id = $1",
    )
    .bind(consent_id.as_str())
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        seed_consent_fixture(&pool, &grant).await.unwrap_err(),
        TrpgError::PolicyEvidenceUntrusted
    );
    let denied_snapshot_id = id(format!("route_denied_{nonce}"));
    let denied_notice = id(format!("notice_after_revoke_{nonce}"));
    seed_notice_fixture(
        &pool,
        &denied_notice,
        &subject_id,
        &policy_version,
        b"cloud processing notice v1",
    )
    .await;
    let outcome = authorize_cloud_egress(
        &ledger,
        CloudEgressRequest {
            snapshot_id: denied_snapshot_id.clone(),
            audit_id: id(format!("audit_denied_{nonce}")),
            subject_id: subject_id.clone(),
            source_provider: id("ollama"),
            target_provider,
            source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
            target_endpoint: "https://cloud.example.test/v1".to_owned(),
            model_id: id("cloud-model-v1"),
            source_credential: SecretReference::mounted("ollama_provider", 1).unwrap(),
            target_credential: SecretReference::kms("cloud_provider", 7).unwrap(),
            source_boundary: ProviderBoundary::Local,
            target_boundary: ProviderBoundary::Cloud,
            fallback_policy: id("explicit_audited_only"),
            privacy_boundary: id("explicit_consent_no_silent_fallback"),
            purpose,
            policy_version,
            notice_reference: Some(denied_notice),
            target_audience: PrincipalScope::Player(subject_id),
            context: vec![verified_cloud_fact(
                &format!("fact_after_revoke_{nonce}"),
                Visibility::new(VisibilityLabel::Public),
                vec![b'x'; 64],
            )],
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome,
        CloudEgressOutcome::Denied {
            reason: CloudEgressDenial::ConsentRequired,
            ..
        }
    ));
    let denied_record = ledger
        .load_recorded_decision(&denied_snapshot_id)
        .await
        .unwrap();
    assert_eq!(denied_record.0, "deny");
    assert_eq!(
        denied_record.1.as_deref(),
        Some(CloudEgressDenial::ConsentRequired.code())
    );
    assert_eq!(denied_record.2.len(), 64);
    let audit_tampering = sqlx::query("DELETE FROM cloud_egress_audit WHERE snapshot_id = $1")
        .bind(denied_snapshot_id.as_str())
        .execute(&pool)
        .await
        .expect_err("cloud egress audit deletion must be rejected");
    assert_append_only_enforcement(audit_tampering);
}
