#[derive(Debug)]
enum CanonicalReplayError {
    Identity(IdentityError),
    Store(CanonicalStoreError),
    StoredEventInvalid,
}

impl CanonicalCustody {
    fn check_readiness(&self) -> Result<(), String> {
        if !self.runtime_events.has_canonical_custody()
            || !self.agent_events.has_canonical_custody()
        {
            return Err("formal runtime/agent canonical custody missing".to_owned());
        }
        self.runtime
            .lock()
            .map_err(|_| "canonical runtime lock poisoned".to_owned())?
            .block_on(self.store.verify_integrity())
            .map_err(|error| error.to_string())?;
        if let Some(agent_jobs) = &self.agent_jobs {
            self.runtime
                .lock()
                .map_err(|_| "agent job runtime lock poisoned".to_owned())?
                .block_on(agent_jobs.workflow.check_agent_job_readiness())
                .map_err(|error| format!("AGENT_JOB_SCHEMA_NOT_READY:{error}"))?;
        }
        self.privacy_runtime
            .lock()
            .map_err(|_| "privacy runtime lock poisoned".to_owned())?
            .block_on(self.deletion_repository.check_readiness())
            .map_err(|error| error.code().to_owned())
    }

    fn replay_visible(
        &self,
        authorization: &ReplayAuthorization,
        now_unix_ms: u64,
        after_sequence: i64,
        limit: i64,
    ) -> Result<VisibleReplayPage, CanonicalReplayError> {
        let records = self
            .runtime
            .lock()
            .map_err(|_| {
                CanonicalReplayError::Store(CanonicalStoreError::Connection {
                    component: "primary",
                })
            })?
            .block_on(self.store.load_replay_page(
                authorization.campaign_id().as_str(),
                after_sequence,
                limit,
            ))
            .map_err(CanonicalReplayError::Store)?;
        let scanned_through_sequence = records
            .last()
            .map_or(after_sequence, |event| event.sequence);
        let mut visible = Vec::with_capacity(records.len());
        for event in records {
            if event.campaign_id != authorization.campaign_id().as_str() {
                return Err(CanonicalReplayError::StoredEventInvalid);
            }
            let visibility = stored_visibility(&event)?;
            if authorization
                .can_view(authorization.campaign_id(), &visibility, now_unix_ms)
                .map_err(CanonicalReplayError::Identity)?
            {
                visible.push(canonical_event_json(event));
            }
        }
        Ok(VisibleReplayPage {
            events: visible,
            scanned_through_sequence,
        })
    }
}

fn stored_visibility(event: &CanonicalReplayEvent) -> Result<Visibility, CanonicalReplayError> {
    let subject =
        (event.visibility_subject != "not_applicable").then_some(event.visibility_subject.as_str());
    Visibility::try_from_parts(&event.visibility_label, subject)
        .map_err(|_| CanonicalReplayError::StoredEventInvalid)
}

fn canonical_event_json(event: CanonicalReplayEvent) -> serde_json::Value {
    json!({
        "sequence": event.sequence,
        "stream_version": event.stream_version,
        "stream_id": event.stream_id,
        "event_type": event.event_type,
        "campaign_id": event.campaign_id,
        "authenticated_actor_id": event.authenticated_actor_id,
        "resource_type": event.resource_type,
        "resource_id": event.resource_id,
        "authority_contract_id": event.authority_contract_id,
        "authority_owner": event.authority_owner,
        "command_id": event.command_id,
        "idempotency_key": event.idempotency_key,
        "authority_contract_version": event.authority_contract_version,
        "visibility_label": event.visibility_label,
        "visibility_subject": event.visibility_subject,
        "provenance_kind": event.provenance_kind,
        "provenance_reference": event.provenance_reference,
        "provenance_recorded_by": event.provenance_recorded_by,
        "correlation_id": event.correlation_id,
        "causation_id": event.causation_id,
        "trace_id": event.trace_id,
        "payload": event.payload,
        "event_integrity_hash": event.event_integrity_hash,
        "request_hash_source": event.request_hash_source,
        "integrity_status": event.integrity_status,
    })
}

fn replay_page_parameters(query: &str) -> Result<(i64, i64), HttpResponse> {
    let mut after_sequence = 0_i64;
    let mut limit = 100_i64;
    let mut saw_after_sequence = false;
    let mut saw_limit = false;
    if query.is_empty() {
        return Ok((after_sequence, limit));
    }
    for parameter in query.split('&') {
        let Some((name, value)) = parameter.split_once('=') else {
            return Err(HttpResponse::json(
                400,
                json!({"error": "INVALID_REPLAY_CURSOR"}),
            ));
        };
        match name {
            "after_sequence" if !saw_after_sequence => {
                after_sequence = value
                    .parse::<i64>()
                    .ok()
                    .filter(|value| *value >= 0)
                    .ok_or_else(|| {
                        HttpResponse::json(400, json!({"error": "INVALID_REPLAY_CURSOR"}))
                    })?;
                saw_after_sequence = true;
            }
            "limit" if !saw_limit => {
                limit = value
                    .parse::<i64>()
                    .ok()
                    .filter(|value| (1..=500).contains(value))
                    .ok_or_else(|| {
                        HttpResponse::json(400, json!({"error": "INVALID_REPLAY_LIMIT"}))
                    })?;
                saw_limit = true;
            }
            _ => {
                return Err(HttpResponse::json(
                    400,
                    json!({"error": "INVALID_REPLAY_CURSOR"}),
                ));
            }
        }
    }
    Ok((after_sequence, limit))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipRequest {
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataDeletionRequest {
    job_id: String,
    subject_id: String,
    retention_policy: String,
    reason: String,
    command_id: String,
    correlation_id: String,
    causation_id: String,
    expected_version: u64,
}
