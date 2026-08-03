fn ensure_private_directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("LOCAL_MODEL_CERTIFICATION_STATE_PATH_INVALID".to_owned());
    }
    fs::create_dir_all(path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_CREATE_FAILED".to_owned())?;
    reject_symlink_if_present(path)?;
    let metadata = fs::metadata(path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_INVALID".to_owned())?;
    if !metadata.is_dir() {
        return Err("LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_INVALID".to_owned());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_PERMISSION_FAILED".to_owned())
}

fn validate_absolute_directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("LOCAL_MODEL_CERTIFICATION_DIRECTORY_INVALID".to_owned());
    }
    reject_symlink_if_present(path)?;
    if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        Ok(())
    } else {
        Err("LOCAL_MODEL_CERTIFICATION_DIRECTORY_INVALID".to_owned())
    }
}

fn reject_symlink_if_present(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("LOCAL_MODEL_CERTIFICATION_SYMLINK_FORBIDDEN".to_owned())
        }
        Ok(_) | Err(_) if !path.exists() => Ok(()),
        Ok(_) => Ok(()),
        Err(_) => Err("LOCAL_MODEL_CERTIFICATION_PATH_INSPECTION_FAILED".to_owned()),
    }
}

#[cfg(test)]
mod local_model_certification_process_tests {
    use super::*;

    fn encoded_request(fields: &[&str]) -> Vec<u8> {
        let mut encoded = Vec::new();
        for field in fields {
            encoded.extend_from_slice(&(field.len() as u64).to_be_bytes());
            encoded.extend_from_slice(field.as_bytes());
        }
        encoded
    }

    #[test]
    fn admin_request_artifact_is_decoded_strictly() {
        let encoded = encoded_request(&[
            "bootstrap-model-certification",
            "model-exact",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ]);
        let decoded = decode_certification_request(&encoded).unwrap();
        assert_eq!(decoded.request_id, "bootstrap-model-certification");
        assert_eq!(decoded.model_id, "model-exact");
        assert!(decoded.model_artifact_sha256.starts_with("sha256:"));

        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_certification_request(&trailing).is_err());
    }

    #[test]
    fn startup_mode_is_fail_closed() {
        assert_eq!(
            AgentWorkerStartupMode::parse(None).unwrap(),
            AgentWorkerStartupMode::CertificationService
        );
        assert_eq!(
            AgentWorkerStartupMode::parse(Some("ready")).unwrap(),
            AgentWorkerStartupMode::Ready
        );
        assert_eq!(
            AgentWorkerStartupMode::parse(Some("certification-only")).unwrap(),
            AgentWorkerStartupMode::CertificationOnly
        );
        assert_eq!(
            AgentWorkerStartupMode::parse(Some("certification-service")).unwrap(),
            AgentWorkerStartupMode::CertificationService
        );
        assert!(AgentWorkerStartupMode::parse(Some("unrestricted")).is_err());
        assert!(validate_certification_identifier("../escape").is_err());
    }

    #[test]
    fn certification_service_discovers_only_strict_request_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "trpg-certification-service-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("request-one.request"), b"request").unwrap();
        fs::write(root.join("request-one.status.json"), b"status").unwrap();

        assert_eq!(
            certification_request_ids(&root).unwrap(),
            vec!["request-one".to_owned()]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
