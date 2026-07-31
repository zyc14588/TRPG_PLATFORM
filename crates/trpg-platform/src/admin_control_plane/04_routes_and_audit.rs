impl AdminControlPlane {
    pub fn handle(&mut self, request: AdminHttpRequest) -> Option<AdminHttpResponse> {
        if !request.path.starts_with("/admin/v1/") {
            return None;
        }
        let result = self.route(&request);
        Some(match result {
            Ok(response) => response,
            Err(error) => {
                let decision = if matches!(error, AdminControlPlaneError::OperationUnavailable(_)) {
                    AuditDecision::Unavailable
                } else {
                    AuditDecision::Deny
                };
                if self
                    .append_request_outcome(&request, error.code(), decision)
                    .is_err()
                {
                    return Some(error_response(&AdminControlPlaneError::AuditIntegrity));
                }
                error_response(&error)
            }
        })
    }

    fn route(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/admin/v1/bootstrap/status") => self.bootstrap_status(request),
            ("POST", "/admin/v1/bootstrap/complete") => self.complete_bootstrap(request),
            ("POST", "/admin/v1/bootstrap/tutorial-authority") => {
                self.configure_tutorial_authority(request)
            }
            ("POST", "/admin/v1/sessions") => self.create_session(request),
            ("PUT", "/admin/v1/providers/configuration") => {
                self.configure_provider(request)
            }
            ("POST", "/admin/v1/providers/probe") => self.probe_provider(request),
            ("POST", "/admin/v1/models/certification-requests") => {
                self.request_model_certification(request)
            }
            ("POST", "/admin/v1/backups") => self.create_backup(request),
            ("POST", "/admin/v1/restores") => self.restore_backup(request),
            ("GET", "/admin/v1/diagnostics") => self.diagnostics(request),
            ("GET", "/admin/v1/audit") => self.audit_records(request),
            (
                _,
                "/admin/v1/bootstrap/status"
                | "/admin/v1/bootstrap/complete"
                | "/admin/v1/bootstrap/tutorial-authority"
                | "/admin/v1/sessions"
                | "/admin/v1/providers/configuration"
                | "/admin/v1/providers/probe"
                | "/admin/v1/models/certification-requests"
                | "/admin/v1/backups"
                | "/admin/v1/restores"
                | "/admin/v1/diagnostics"
                | "/admin/v1/audit",
            ) => {
                Err(AdminControlPlaneError::InvalidRequest(
                    "ADMIN_METHOD_NOT_ALLOWED",
                ))
            }
            _ => Err(AdminControlPlaneError::InvalidRequest(
                "ADMIN_ROUTE_NOT_FOUND",
            )),
        }
    }

    fn diagnostics(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        let evidence = self
            .operations
            .diagnostics()
            .map_err(AdminControlPlaneError::OperationUnavailable)?;
        self.append_read_audit(&actor, "diagnostics.read", "deployment", request)?;
        Ok(evidence_response(evidence, false, self.load_state()?.version))
    }

    fn audit_records(
        &mut self,
        request: &AdminHttpRequest,
    ) -> Result<AdminHttpResponse, AdminControlPlaneError> {
        let actor = self.authenticate_owner(request)?;
        self.append_read_audit(&actor, "audit.read", "admin-audit", request)?;
        let records = self
            .audit
            .verify()
            .map_err(|_| AdminControlPlaneError::AuditIntegrity)?;
        let records: Vec<_> = records.into_iter().rev().take(200).collect();
        Ok(AdminHttpResponse {
            status: 200,
            body: json!({"records": records}),
        })
    }

    fn append_audit(
        &mut self,
        actor: &AdminActor,
        action: &str,
        resource_id: &str,
        decision: AuditDecision,
        metadata: &MutationMetadata,
    ) -> Result<(), AdminControlPlaneError> {
        self.audit
            .append(AuditRecordDraft {
                actor_id: actor.actor_id.clone(),
                actor_origin: "admin-api".to_owned(),
                authentication_reference: actor.authentication_reference.clone(),
                campaign_id: "server".to_owned(),
                resource_type: "admin-operation".to_owned(),
                resource_id: resource_id.to_owned(),
                action: action.to_owned(),
                requested_role: "SERVER_OWNER".to_owned(),
                visibility_label: "SERVER_ADMIN_ONLY".to_owned(),
                visibility_subject: actor.actor_id.clone(),
                provenance_kind: "ADMIN_COMMAND".to_owned(),
                provenance_reference: metadata.idempotency_key.clone(),
                provenance_recorded_by: "admin-server".to_owned(),
                decision,
                openfga_decision_id: metadata.correlation_id.clone(),
                openfga_policy_revision: ADMIN_AUDIT_POLICY.to_owned(),
                opa_decision_id: metadata.causation_id.clone(),
                opa_policy_revision: ADMIN_AUDIT_POLICY.to_owned(),
                trace_id: metadata.correlation_id.clone(),
            })
            .map(|_| ())
            .map_err(|_| AdminControlPlaneError::AuditIntegrity)
    }

    fn append_read_audit(
        &mut self,
        actor: &AdminActor,
        action: &str,
        resource_id: &str,
        request: &AdminHttpRequest,
    ) -> Result<(), AdminControlPlaneError> {
        let correlation = request
            .header("x-correlation-id")
            .filter(|value| !value.trim().is_empty() && value.len() <= 256)
            .unwrap_or("admin-read")
            .to_owned();
        let metadata = MutationMetadata {
            idempotency_key: format!("read-{correlation}"),
            expected_version: 0,
            correlation_id: correlation.clone(),
            causation_id: correlation,
        };
        self.append_audit(
            actor,
            action,
            resource_id,
            AuditDecision::Permit,
            &metadata,
        )
    }

    fn append_request_outcome(
        &mut self,
        request: &AdminHttpRequest,
        code: &str,
        decision: AuditDecision,
    ) -> Result<(), AdminControlPlaneError> {
        let correlation = request
            .header("x-correlation-id")
            .filter(|value| !value.trim().is_empty() && value.len() <= 256)
            .unwrap_or("untrusted-request")
            .to_owned();
        let metadata = MutationMetadata {
            idempotency_key: format!("outcome-{correlation}"),
            expected_version: 0,
            correlation_id: correlation.clone(),
            causation_id: correlation,
        };
        self.append_audit(
            &AdminActor {
                actor_id: "untrusted".to_owned(),
                authentication_reference: "unverified".to_owned(),
            },
            request.path.as_str(),
            code,
            decision,
            &metadata,
        )
    }
}

fn error_response(error: &AdminControlPlaneError) -> AdminHttpResponse {
    AdminHttpResponse {
        status: error.http_status(),
        body: json!({"error": error.code()}),
    }
}

fn evidence_response(
    evidence: AdminOperationEvidence,
    replayed: bool,
    state_version: u64,
) -> AdminHttpResponse {
    AdminHttpResponse {
        status: 200,
        body: json!({
            "result": evidence.code,
            "artifact_reference": evidence.artifact_reference,
            "digest": evidence.digest,
            "replayed": replayed,
            "state_version": state_version
        }),
    }
}
