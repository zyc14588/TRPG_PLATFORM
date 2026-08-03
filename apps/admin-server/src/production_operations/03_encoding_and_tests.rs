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

fn workflow_policy_tuples(campaign_id: &str) -> Vec<String> {
    [
        "api_core_workflow",
        "api_player_action_workflow",
        "api_privacy_workflow",
        "agent-worker-primary",
    ]
        .into_iter()
        .map(|principal| {
            format!(
                "{{\"user\":\"principal:{principal}\",\"relation\":\"workflow\",\"object\":\"campaign:{campaign_id}\"}}"
            )
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::workflow_policy_tuples;

    #[test]
    fn campaign_policy_provisions_all_formal_workflow_principals() {
        let tuples = workflow_policy_tuples("campaign_tutorial");

        assert_eq!(tuples.len(), 4);
        assert!(tuples.iter().any(|tuple| {
            tuple.contains("\"user\":\"principal:api_core_workflow\"")
                && tuple.contains("\"object\":\"campaign:campaign_tutorial\"")
        }));
        assert!(tuples.iter().any(|tuple| {
            tuple.contains("\"user\":\"principal:api_player_action_workflow\"")
                && tuple.contains("\"object\":\"campaign:campaign_tutorial\"")
        }));
        assert!(tuples.iter().any(|tuple| {
            tuple.contains("\"user\":\"principal:api_privacy_workflow\"")
                && tuple.contains("\"object\":\"campaign:campaign_tutorial\"")
        }));
        assert!(tuples.iter().any(|tuple| {
            tuple.contains("\"user\":\"principal:agent-worker-primary\"")
                && tuple.contains("\"object\":\"campaign:campaign_tutorial\"")
        }));
    }
}
