impl AdminOperations for ProductionAdminOperations {
    fn provision_workflow_policy(
        &self,
        campaign_id: &str,
    ) -> Result<AdminOperationEvidence, String> {
        let (store_id, model_id, tuples) = self.workflow_policy_fields(campaign_id)?;
        let check_path = format!("/stores/{store_id}/check");
        let mut missing = Vec::new();
        for tuple in &tuples {
            let check_body =
                format!("{{\"authorization_model_id\":\"{model_id}\",\"tuple_key\":{tuple}}}");
            if !openfga_check_allows(&self.openfga_post(&check_path, check_body.as_bytes())?) {
                missing.push(tuple.as_str());
            }
        }
        if missing.is_empty() {
            return Ok(AdminOperationEvidence::completed(
                "WORKFLOW_POLICY_CONFIGURED",
            ));
        }
        let write_body = format!(
            "{{\"authorization_model_id\":\"{model_id}\",\"writes\":{{\"tuple_keys\":[{}]}}}}",
            missing.join(",")
        );
        self.openfga_post(&format!("/stores/{store_id}/write"), write_body.as_bytes())?;
        for tuple in &tuples {
            let check_body =
                format!("{{\"authorization_model_id\":\"{model_id}\",\"tuple_key\":{tuple}}}");
            if !openfga_check_allows(&self.openfga_post(&check_path, check_body.as_bytes())?) {
                return Err("ADMIN_OPENFGA_POLICY_VERIFY_FAILED".to_owned());
            }
        }
        Ok(AdminOperationEvidence::completed(
            "WORKFLOW_POLICY_CONFIGURED",
        ))
    }

    fn probe_provider(
        &self,
        configuration: &AdminProviderConfiguration,
        credential: &str,
    ) -> Result<AdminOperationEvidence, String> {
        if credential.is_empty()
            || credential.len() > 16_384
            || credential.bytes().any(|byte| matches!(byte, b'\r' | b'\n'))
        {
            return Err("ADMIN_PROVIDER_CREDENTIAL_INVALID".to_owned());
        }
        let probe_path = if configuration.provider_type == "ollama" {
            "api/tags"
        } else {
            "models"
        };
        let url = format!(
            "{}/{probe_path}",
            configuration.base_url.trim_end_matches('/')
        );
        let mut child = Command::new(&self.curl_path)
            .arg("--fail")
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg("15")
            .arg("--max-filesize")
            .arg("1048576")
            .arg("--proto")
            .arg("=https")
            .arg("--cacert")
            .arg(&self.provider_ca_path)
            .arg("--header")
            .arg("@-")
            .arg("--url")
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "ADMIN_PROVIDER_PROBE_START_FAILED".to_owned())?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "ADMIN_PROVIDER_PROBE_INPUT_FAILED".to_owned())?;
        stdin
            .write_all(b"Authorization: Bearer ")
            .and_then(|()| stdin.write_all(credential.as_bytes()))
            .and_then(|()| stdin.write_all(b"\n"))
            .map_err(|_| "ADMIN_PROVIDER_PROBE_INPUT_FAILED".to_owned())?;
        drop(stdin);
        let output = child
            .wait_with_output()
            .map_err(|_| "ADMIN_PROVIDER_PROBE_WAIT_FAILED".to_owned())?;
        if !output.status.success()
            || !provider_probe_confirms_model(&output.stdout, &configuration.model_id)
        {
            return Err("ADMIN_PROVIDER_PROBE_FAILED".to_owned());
        }
        Ok(AdminOperationEvidence::completed("PROVIDER_REACHABLE"))
    }

    fn request_model_certification(
        &self,
        request: &AdminModelCertificationRequest,
    ) -> Result<AdminOperationEvidence, String> {
        let path = self.persist_certification_request(request)?;
        Ok(AdminOperationEvidence {
            code: "CERTIFICATION_REQUESTED".to_owned(),
            artifact_reference: Some(path.display().to_string()),
            digest: None,
        })
    }

    fn create_backup(
        &self,
        request: &AdminBackupRequest,
    ) -> Result<AdminOperationEvidence, String> {
        match self.backup_executor.create_backup(
            &self.backup_source_service,
            &self.backup_directory,
            &request.backup_id,
            &request.schema_version,
        ) {
            Ok(artifact) => Ok(AdminOperationEvidence {
                code: "BACKUP_CREATED".to_owned(),
                artifact_reference: Some(artifact.manifest_path.display().to_string()),
                digest: Some(artifact.manifest.sha256),
            }),
            Err(PostgresBackupRestoreError::OutputAlreadyExists) => self.existing_backup(request),
            Err(error) => Err(operation_error(error)),
        }
    }

    fn restore_backup(
        &self,
        request: &AdminRestoreRequest,
    ) -> Result<AdminOperationEvidence, String> {
        let source_manifest = self
            .backup_executor
            .verify_backup(&request.manifest_path, &request.expected_schema_version)
            .map_err(operation_error)?;
        self.persist_restore_intent(request, &source_manifest.sha256)?;
        let safety_manifest = self
            .safety_directory
            .join(format!("{}.manifest.json", request.safety_point_id));
        if safety_manifest.exists() {
            self.backup_executor
                .verify_backup(&safety_manifest, &request.expected_schema_version)
                .map_err(operation_error)?;
        } else {
            self.backup_executor
                .create_backup(
                    &self.restore_target_service,
                    &self.safety_directory,
                    &request.safety_point_id,
                    &request.expected_schema_version,
                )
                .map_err(operation_error)?;
        }
        self.backup_executor
            .restore_backup_filtered_after_reset(
                &self.psql_path,
                &self.restore_target_service,
                &request.manifest_path,
            )
            .map_err(operation_error)?;
        Ok(AdminOperationEvidence {
            code: "RESTORE_COMPLETED".to_owned(),
            artifact_reference: Some(safety_manifest.display().to_string()),
            digest: Some(source_manifest.sha256),
        })
    }

    fn diagnostics(&self) -> Result<AdminOperationEvidence, String> {
        for path in [
            &self.curl_path,
            &self.psql_path,
            &self.provider_ca_path,
            &self.openfga_store_id_path,
            &self.openfga_model_id_path,
            &self.backup_directory,
            &self.safety_directory,
            &self.certification_directory,
        ] {
            fs::symlink_metadata(path)
                .map_err(|_| "ADMIN_DIAGNOSTIC_PATH_UNAVAILABLE".to_owned())?;
        }
        Ok(AdminOperationEvidence::completed("DIAGNOSTICS_HEALTHY"))
    }
}
