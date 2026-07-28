
fn evaluate_context(
    request: &CloudEgressRequest,
    consent: &PersistedCloudConsent,
    allowed_fact_ids: &mut Vec<EntityId>,
) -> Option<CloudEgressDenial> {
    if request.context.is_empty() {
        return Some(CloudEgressDenial::ContextNotMinimized);
    }
    let mut total_bytes = 0_u64;
    let mut seen = HashSet::new();
    for fact in &request.context {
        total_bytes = match total_bytes.checked_add(fact.serialized_bytes()) {
            Some(total) if total <= MAX_CLOUD_CONTEXT_BYTES => total,
            _ => return Some(CloudEgressDenial::ContextNotMinimized),
        };
        if !seen.insert(fact.fact_id.clone()) {
            return Some(CloudEgressDenial::ContextNotMinimized);
        }
        if !fact.visibility.can_view(&request.target_audience) {
            return Some(CloudEgressDenial::ContextAudienceDenied);
        }
        let allowed = match fact.visibility.label().kind() {
            VisibilityKind::Public => true,
            VisibilityKind::SpectatorVisible => false,
            VisibilityKind::PrivateToPlayer | VisibilityKind::InvestigatorPrivate => {
                consent.visibility_scope == ConsentVisibilityScope::SubjectPrivate
                    && fact.visibility.subject_id() == Some(&request.subject_id)
            }
            VisibilityKind::PartyVisible
            | VisibilityKind::PrivateToGroup
            | VisibilityKind::KeeperOnly
            | VisibilityKind::AiInternal
            | VisibilityKind::SystemOnly
            | VisibilityKind::SpectatorHidden
            | VisibilityKind::SystemPrivate => false,
        };
        if !allowed {
            return Some(CloudEgressDenial::RestrictedContext);
        }
        allowed_fact_ids.push(fact.fact_id.clone());
    }
    None
}

fn context_manifest_hash(context: &[CloudContextFact]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"trpg-cloud-context-manifest-v2\0");
    for fact in context {
        update_length_prefixed(&mut digest, fact.fact_id.as_str().as_bytes());
        update_length_prefixed(&mut digest, fact.visibility.label().as_str().as_bytes());
        update_length_prefixed(
            &mut digest,
            fact.visibility
                .subject_id()
                .map(EntityId::as_str)
                .unwrap_or("")
                .as_bytes(),
        );
        digest.update(fact.serialized_bytes().to_be_bytes());
        fact.expose_serialized_to(|bytes| update_length_prefixed(&mut digest, bytes));
    }
    format!("{:x}", digest.finalize())
}

fn valid_local_source_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn valid_cloud_target_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn update_length_prefixed(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
