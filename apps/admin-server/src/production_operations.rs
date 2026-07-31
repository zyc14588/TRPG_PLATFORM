use std::fs;
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::{
    optional_regular_file, required_environment, required_private_directory, required_regular_file,
    set_private_file_permissions, sync_directory,
};
use trpg_ops::backup_restore_runbook::{PostgresBackupExecutor, PostgresBackupRestoreError};
use trpg_platform::admin_control_plane::{
    openfga_check_allows, provider_probe_confirms_model, AdminBackupRequest,
    AdminModelCertificationRequest, AdminOperationEvidence, AdminOperations,
    AdminProviderConfiguration, AdminRestoreRequest,
};

pub struct ProductionAdminOperations {
    curl_path: PathBuf,
    psql_path: PathBuf,
    provider_ca_path: PathBuf,
    openfga_address: SocketAddr,
    openfga_store_id_path: PathBuf,
    openfga_model_id_path: PathBuf,
    backup_executor: PostgresBackupExecutor,
    backup_source_service: String,
    restore_target_service: String,
    backup_directory: PathBuf,
    safety_directory: PathBuf,
    certification_directory: PathBuf,
}

impl ProductionAdminOperations {
    pub fn from_environment() -> Result<Self, &'static str> {
        let curl_path = required_regular_file("TRPG_ADMIN_CURL_PATH")?;
        let psql_path = required_regular_file("TRPG_ADMIN_PSQL_PATH")?;
        let provider_ca_path = required_regular_file("TRPG_ADMIN_PROVIDER_CA_PATH")?;
        let openfga_address = required_environment("TRPG_ADMIN_OPENFGA_ADDRESS")?
            .parse::<SocketAddr>()
            .map_err(|_| "ADMIN_OPENFGA_ADDRESS_INVALID")?;
        if !openfga_address.ip().is_loopback() {
            return Err("ADMIN_OPENFGA_LOOPBACK_REQUIRED");
        }
        let openfga_store_id_path = required_regular_file("TRPG_ADMIN_OPENFGA_STORE_ID_FILE")?;
        let openfga_model_id_path = required_regular_file("TRPG_ADMIN_OPENFGA_MODEL_ID_FILE")?;
        let pg_dump = required_regular_file("TRPG_ADMIN_PG_DUMP_PATH")?;
        let pg_restore = required_regular_file("TRPG_ADMIN_PG_RESTORE_PATH")?;
        let service_file = required_regular_file("TRPG_ADMIN_PG_SERVICE_FILE_PATH")?;
        let passfile = optional_regular_file("TRPG_ADMIN_PG_PASSFILE_PATH")?;
        let backup_executor =
            PostgresBackupExecutor::new(pg_dump, pg_restore, service_file, passfile)
                .map_err(|_| "ADMIN_BACKUP_CONFIGURATION_INVALID")?;
        let backup_source_service = required_environment("TRPG_ADMIN_BACKUP_SOURCE_SERVICE")?;
        let restore_target_service = required_environment("TRPG_ADMIN_RESTORE_TARGET_SERVICE")?;
        if backup_source_service == restore_target_service {
            return Err("ADMIN_BACKUP_AND_RESTORE_SERVICES_MUST_DIFFER");
        }
        let backup_directory = required_private_directory("TRPG_ADMIN_BACKUP_DIRECTORY")?;
        let safety_directory = required_private_directory("TRPG_ADMIN_SAFETY_DIRECTORY")?;
        let certification_directory =
            required_private_directory("TRPG_ADMIN_CERTIFICATION_DIRECTORY")?;
        Ok(Self {
            curl_path,
            psql_path,
            provider_ca_path,
            openfga_address,
            openfga_store_id_path,
            openfga_model_id_path,
            backup_executor,
            backup_source_service,
            restore_target_service,
            backup_directory,
            safety_directory,
            certification_directory,
        })
    }

    fn openfga_post(&self, path: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let url = format!("http://{}{}", self.openfga_address, path);
        let mut child = Command::new(&self.curl_path)
            .arg("--fail")
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg("10")
            .arg("--max-filesize")
            .arg("1048576")
            .arg("--proto")
            .arg("=http")
            .arg("--request")
            .arg("POST")
            .arg("--header")
            .arg("Content-Type: application/json")
            .arg("--data-binary")
            .arg("@-")
            .arg("--url")
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "ADMIN_OPENFGA_REQUEST_START_FAILED".to_owned())?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| "ADMIN_OPENFGA_REQUEST_INPUT_FAILED".to_owned())?;
        input
            .write_all(body)
            .map_err(|_| "ADMIN_OPENFGA_REQUEST_INPUT_FAILED".to_owned())?;
        drop(input);
        let output = child
            .wait_with_output()
            .map_err(|_| "ADMIN_OPENFGA_REQUEST_WAIT_FAILED".to_owned())?;
        if !output.status.success() || output.stdout.len() > 1024 * 1024 {
            return Err("ADMIN_OPENFGA_REQUEST_FAILED".to_owned());
        }
        Ok(output.stdout)
    }

    fn workflow_policy_fields(
        &self,
        campaign_id: &str,
    ) -> Result<(String, String, String), String> {
        validate_file_identifier(campaign_id)?;
        let store_id = fs::read_to_string(&self.openfga_store_id_path)
            .map_err(|_| "ADMIN_OPENFGA_STORE_ID_UNAVAILABLE".to_owned())?;
        let model_id = fs::read_to_string(&self.openfga_model_id_path)
            .map_err(|_| "ADMIN_OPENFGA_MODEL_ID_UNAVAILABLE".to_owned())?;
        let store_id = store_id.trim().to_owned();
        let model_id = model_id.trim().to_owned();
        validate_file_identifier(&store_id)?;
        validate_file_identifier(&model_id)?;
        let tuple = format!(
            "{{\"user\":\"principal:api_core_workflow\",\"relation\":\"workflow\",\"object\":\"campaign:{campaign_id}\"}}"
        );
        Ok((store_id, model_id, tuple))
    }

    fn existing_backup(
        &self,
        request: &AdminBackupRequest,
    ) -> Result<AdminOperationEvidence, String> {
        let manifest_path = self
            .backup_directory
            .join(format!("{}.manifest.json", request.backup_id));
        let manifest = self
            .backup_executor
            .verify_backup(&manifest_path, &request.schema_version)
            .map_err(operation_error)?;
        Ok(AdminOperationEvidence {
            code: "BACKUP_CREATED".to_owned(),
            artifact_reference: Some(manifest_path.display().to_string()),
            digest: Some(manifest.sha256),
        })
    }

    fn persist_certification_request(
        &self,
        request: &AdminModelCertificationRequest,
    ) -> Result<PathBuf, String> {
        validate_file_identifier(&request.request_id)?;
        let path = self
            .certification_directory
            .join(format!("{}.request", request.request_id));
        let encoded = encode_certification_request(request);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(&encoded)
                    .and_then(|()| file.sync_all())
                    .map_err(|_| "ADMIN_CERTIFICATION_WRITE_FAILED".to_owned())?;
                set_private_file_permissions(&path)?;
                sync_directory(&self.certification_directory)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing =
                    fs::read(&path).map_err(|_| "ADMIN_CERTIFICATION_READ_FAILED".to_owned())?;
                if existing != encoded {
                    return Err("ADMIN_CERTIFICATION_REQUEST_CONFLICT".to_owned());
                }
            }
            Err(_) => return Err("ADMIN_CERTIFICATION_CREATE_FAILED".to_owned()),
        }
        Ok(path)
    }

    fn persist_restore_intent(
        &self,
        request: &AdminRestoreRequest,
        archive_digest: &str,
    ) -> Result<(), String> {
        validate_file_identifier(&request.safety_point_id)?;
        let path = self
            .safety_directory
            .join(format!("{}.restore-intent", request.safety_point_id));
        let encoded = encode_fields(&[
            request.manifest_path.as_bytes(),
            request.expected_schema_version.as_bytes(),
            archive_digest.as_bytes(),
        ]);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(&encoded)
                    .and_then(|()| file.sync_all())
                    .map_err(|_| "ADMIN_RESTORE_INTENT_WRITE_FAILED".to_owned())?;
                set_private_file_permissions(&path)?;
                sync_directory(&self.safety_directory)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(path).map_err(|_| "ADMIN_RESTORE_INTENT_READ_FAILED".to_owned())?
                    != encoded
                {
                    return Err("ADMIN_RESTORE_SAFETY_POINT_CONFLICT".to_owned());
                }
            }
            Err(_) => return Err("ADMIN_RESTORE_INTENT_CREATE_FAILED".to_owned()),
        }
        Ok(())
    }
}

impl AdminOperations for ProductionAdminOperations {
    fn provision_workflow_policy(
        &self,
        campaign_id: &str,
    ) -> Result<AdminOperationEvidence, String> {
        let (store_id, model_id, tuple) = self.workflow_policy_fields(campaign_id)?;
        let check_path = format!("/stores/{store_id}/check");
        let check_body =
            format!("{{\"authorization_model_id\":\"{model_id}\",\"tuple_key\":{tuple}}}");
        if openfga_check_allows(&self.openfga_post(&check_path, check_body.as_bytes())?) {
            return Ok(AdminOperationEvidence::completed(
                "WORKFLOW_POLICY_CONFIGURED",
            ));
        }
        let write_body = format!(
            "{{\"authorization_model_id\":\"{model_id}\",\"writes\":{{\"tuple_keys\":[{tuple}]}}}}"
        );
        self.openfga_post(&format!("/stores/{store_id}/write"), write_body.as_bytes())?;
        if !openfga_check_allows(&self.openfga_post(&check_path, check_body.as_bytes())?) {
            return Err("ADMIN_OPENFGA_POLICY_VERIFY_FAILED".to_owned());
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
        let url = format!("{}/models", configuration.base_url.trim_end_matches('/'));
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

fn operation_error(error: PostgresBackupRestoreError) -> String {
    let code = format!("ADMIN_BACKUP_OPERATION_FAILED:{error}");
    eprintln!("service=admin-server operation=backup-restore error={code}");
    code
}

fn validate_file_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("ADMIN_FILE_IDENTIFIER_INVALID".to_owned());
    }
    Ok(())
}

fn encode_certification_request(request: &AdminModelCertificationRequest) -> Vec<u8> {
    encode_fields(&[
        request.request_id.as_bytes(),
        request.model_id.as_bytes(),
        request.model_artifact_sha256.as_bytes(),
    ])
}

fn encode_fields(fields: &[&[u8]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for field in fields {
        encoded.extend_from_slice(&(field.len() as u64).to_be_bytes());
        encoded.extend_from_slice(field);
    }
    encoded
}
