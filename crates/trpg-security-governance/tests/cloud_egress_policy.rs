mod common;

use std::sync::Mutex;

use async_trait::async_trait;
use trpg_security_governance::cloud_egress::{
    authorize_cloud_egress, CloudConsentQuery, CloudEgressAttempt, CloudEgressAuditRecord,
    CloudEgressDecision, CloudEgressDenial, CloudEgressLedger, CloudEgressOutcome,
    CloudEgressRequest, CloudRouteSnapshotRecord, ConsentVisibilityScope, PersistedCloudConsent,
    ProviderBoundary,
};
use trpg_security_governance::secret::SecretReference;
use trpg_shared_kernel::{EntityId, KernelResult, PrincipalScope, Visibility, VisibilityLabel};

#[derive(Default)]
struct MemoryLedger {
    consent: Option<PersistedCloudConsent>,
    decisions: Mutex<Vec<(CloudRouteSnapshotRecord, CloudEgressAuditRecord)>>,
}

#[async_trait]
impl CloudEgressLedger for MemoryLedger {
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64> {
        Ok(10_000)
    }

    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        _subject_id: &EntityId,
        _policy_version: &EntityId,
    ) -> KernelResult<bool> {
        Ok(!notice_reference.as_str().is_empty())
    }

    async fn load_active_consent(
        &self,
        _query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>> {
        Ok(self.consent.clone())
    }

    async fn record_route_decision(
        &self,
        snapshot: CloudRouteSnapshotRecord,
        audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool> {
        self.decisions.lock().unwrap().push((snapshot, audit));
        Ok(true)
    }
}

fn id(value: &str) -> EntityId {
    EntityId::new(value).unwrap()
}

fn consent(scope: ConsentVisibilityScope) -> PersistedCloudConsent {
    PersistedCloudConsent::loaded_from_repository(
        id("consent-1"),
        id("player-1"),
        id("cloud-provider"),
        id("gameplay"),
        id("privacy-v1"),
        scope,
        20_000,
    )
}

fn request(snapshot: &str, visibility: Visibility) -> CloudEgressRequest {
    CloudEgressRequest {
        snapshot_id: id(snapshot),
        audit_id: id(&format!("audit-{snapshot}")),
        subject_id: id("player-1"),
        source_provider: id("ollama"),
        target_provider: id("cloud-provider"),
        source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
        target_endpoint: "https://cloud.example.test/v1".to_owned(),
        model_id: id("cloud-model-v1"),
        source_credential: SecretReference::development("ollama_dev", 1).unwrap(),
        target_credential: SecretReference::development("cloud_dev", 1).unwrap(),
        source_boundary: ProviderBoundary::Local,
        target_boundary: ProviderBoundary::Cloud,
        fallback_policy: id("explicit_audited_only"),
        privacy_boundary: id("explicit_consent_no_silent_fallback"),
        purpose: id("gameplay"),
        policy_version: id("privacy-v1"),
        notice_reference: Some(id("notice-1")),
        target_audience: PrincipalScope::Player(id("player-1")),
        context: vec![common::verified_cloud_fact(
            "fact-1",
            visibility,
            vec![b'x'; 128],
        )],
    }
}

#[tokio::test]
async fn local_to_cloud_requires_persisted_consent_and_records_denial() {
    let ledger = MemoryLedger::default();
    let outcome = authorize_cloud_egress(
        &ledger,
        request(
            "snapshot-no-consent",
            Visibility::new(VisibilityLabel::Public),
        ),
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
    let decisions = ledger.decisions.lock().unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].0.decision, CloudEgressDecision::Deny);
    assert_eq!(decisions[0].1.decision, CloudEgressDecision::Deny);
}

#[tokio::test]
async fn persisted_consent_notice_and_minimal_public_context_create_authorization() {
    let ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::PublicOnly)),
        ..MemoryLedger::default()
    };
    let request = request("snapshot-public", Visibility::new(VisibilityLabel::Public));
    let context = request.context.clone();
    let outcome = authorize_cloud_egress(&ledger, request).await.unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        panic!("expected an authorization produced by the governance policy");
    };

    assert!(authorization.permits_context(CloudEgressAttempt {
        source_provider: "ollama",
        target_provider: "cloud-provider",
        source_endpoint: "http://127.0.0.1:11434/v1",
        target_endpoint: "https://cloud.example.test/v1",
        model_id: "cloud-model-v1",
        source_credential: &SecretReference::development("ollama_dev", 1).unwrap(),
        target_credential: &SecretReference::development("cloud_dev", 1).unwrap(),
        fallback_policy: "explicit_audited_only",
        privacy_boundary: "explicit_consent_no_silent_fallback",
        context: &context,
    }));
    assert!(authorization.allows_fact(&id("fact-1")));
    assert_eq!(authorization.context_manifest_hash().len(), 64);
    let decisions = ledger.decisions.lock().unwrap();
    assert_eq!(decisions[0].0.decision, CloudEgressDecision::Allow);
    assert_eq!(decisions[0].0.allowed_fact_ids, vec![id("fact-1")]);
}

#[tokio::test]
async fn endpoint_secret_carriers_are_denied_and_debug_output_is_redacted() {
    for (suffix, source_endpoint, target_endpoint) in [
        (
            "source-query",
            "http://127.0.0.1:11434/v1?api_key=source-secret",
            "https://cloud.example.test/v1",
        ),
        (
            "target-query",
            "http://127.0.0.1:11434/v1",
            "https://cloud.example.test/v1?api_key=target-secret",
        ),
        (
            "target-fragment",
            "http://127.0.0.1:11434/v1",
            "https://cloud.example.test/v1#signed-secret",
        ),
    ] {
        let ledger = MemoryLedger {
            consent: Some(consent(ConsentVisibilityScope::PublicOnly)),
            ..MemoryLedger::default()
        };
        let mut cloud_request = request(
            &format!("snapshot-{suffix}"),
            Visibility::new(VisibilityLabel::Public),
        );
        cloud_request.source_endpoint = source_endpoint.to_owned();
        cloud_request.target_endpoint = target_endpoint.to_owned();
        let request_debug = format!("{cloud_request:?}");
        assert!(!request_debug.contains("api_key"));
        assert!(!request_debug.contains("signed-secret"));
        assert!(!request_debug.contains(source_endpoint));
        assert!(!request_debug.contains(target_endpoint));

        let outcome = authorize_cloud_egress(&ledger, cloud_request)
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            CloudEgressOutcome::Denied {
                reason: CloudEgressDenial::InvalidRoute,
                ..
            }
        ));
        let decisions = ledger.decisions.lock().unwrap();
        let persisted_debug = format!("{:?} {:?}", decisions[0].0, decisions[0].1);
        assert!(!persisted_debug.contains("api_key"));
        assert!(!persisted_debug.contains("signed-secret"));
    }

    let ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::PublicOnly)),
        ..MemoryLedger::default()
    };
    let outcome = authorize_cloud_egress(
        &ledger,
        request(
            "snapshot-redacted-authorization",
            Visibility::new(VisibilityLabel::Public),
        ),
    )
    .await
    .unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        panic!("valid public route should authorize");
    };
    let authorization_debug = format!("{authorization:?}");
    assert!(!authorization_debug.contains("127.0.0.1"));
    assert!(!authorization_debug.contains("cloud.example.test"));
}

#[tokio::test]
async fn public_only_rejects_spectator_visible_and_authorization_binds_exact_bytes() {
    let spectator_ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::PublicOnly)),
        ..MemoryLedger::default()
    };
    let spectator = authorize_cloud_egress(
        &spectator_ledger,
        request(
            "snapshot-spectator",
            Visibility::new(VisibilityLabel::SpectatorVisible),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        spectator,
        CloudEgressOutcome::Denied {
            reason: CloudEgressDenial::RestrictedContext,
            ..
        }
    ));

    let ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::PublicOnly)),
        ..MemoryLedger::default()
    };
    let request = request(
        "snapshot-byte-binding",
        Visibility::new(VisibilityLabel::Public),
    );
    let outcome = authorize_cloud_egress(&ledger, request).await.unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        panic!("public context should authorize");
    };
    let changed = vec![common::verified_cloud_fact(
        "fact-1",
        Visibility::new(VisibilityLabel::Public),
        vec![b'y'; 128],
    )];
    assert!(!authorization.permits_context(CloudEgressAttempt {
        source_provider: "ollama",
        target_provider: "cloud-provider",
        source_endpoint: "http://127.0.0.1:11434/v1",
        target_endpoint: "https://cloud.example.test/v1",
        model_id: "cloud-model-v1",
        source_credential: &SecretReference::development("ollama_dev", 1).unwrap(),
        target_credential: &SecretReference::development("cloud_dev", 1).unwrap(),
        fallback_policy: "explicit_audited_only",
        privacy_boundary: "explicit_consent_no_silent_fallback",
        context: &changed,
    }));
}

#[tokio::test]
async fn keeper_system_and_group_context_remain_default_deny_across_cloud_boundary() {
    for (suffix, visibility) in [
        ("keeper", Visibility::new(VisibilityLabel::KeeperOnly)),
        ("system", Visibility::new(VisibilityLabel::SystemOnly)),
        (
            "group",
            Visibility::private_to_group(id("investigator-group")),
        ),
    ] {
        let ledger = MemoryLedger {
            consent: Some(consent(ConsentVisibilityScope::SubjectPrivate)),
            ..MemoryLedger::default()
        };
        let outcome =
            authorize_cloud_egress(&ledger, request(&format!("snapshot-{suffix}"), visibility))
                .await
                .unwrap();
        assert!(matches!(
            outcome,
            CloudEgressOutcome::Denied {
                reason: CloudEgressDenial::ContextAudienceDenied
                    | CloudEgressDenial::RestrictedContext,
                ..
            }
        ));
        assert_eq!(
            ledger.decisions.lock().unwrap()[0].0.decision,
            CloudEgressDecision::Deny
        );
    }
}

#[tokio::test]
async fn subject_private_scope_only_authorizes_the_consented_players_private_fact() {
    let ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::SubjectPrivate)),
        ..MemoryLedger::default()
    };
    let outcome = authorize_cloud_egress(
        &ledger,
        request(
            "snapshot-private",
            Visibility::private_to_player(id("player-1")),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CloudEgressOutcome::Authorized(_)));

    let ledger = MemoryLedger {
        consent: Some(consent(ConsentVisibilityScope::SubjectPrivate)),
        ..MemoryLedger::default()
    };
    let outcome = authorize_cloud_egress(
        &ledger,
        request(
            "snapshot-other-private",
            Visibility::private_to_player(id("player-2")),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome,
        CloudEgressOutcome::Denied {
            reason: CloudEgressDenial::ContextAudienceDenied,
            ..
        }
    ));
}
