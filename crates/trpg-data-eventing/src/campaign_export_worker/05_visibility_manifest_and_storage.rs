fn normalized_view(audience: &str) -> Result<&'static str, CampaignExportWorkerError> {
    match audience {
        "PLAYER" => Ok("PLAYER"),
        "KEEPER_PRIVATE" | "CAMPAIGN_ARCHIVE" => Ok("KEEPER_PRIVATE"),
        "AUDIT" => Ok("AUDIT"),
        _ => Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_AUDIENCE_INVALID",
        )),
    }
}

fn export_record(
    view: &str,
    request: &ExportRequest,
    groups: &BTreeSet<String>,
    event: &CanonicalReplayEvent,
) -> Option<Value> {
    let include = match view {
        "PLAYER" => match event.visibility_label.as_str() {
            "public" | "party_visible" | "spectator_visible" | "spectator_hidden" => true,
            "private_to_player" | "investigator_private" => {
                event.visibility_subject == request.requested_by
            }
            "private_to_group" => groups.contains(&event.visibility_subject),
            _ => false,
        },
        "KEEPER_PRIVATE" => !matches!(
            event.visibility_label.as_str(),
            "ai_internal" | "system_only" | "system_private"
        ),
        "AUDIT" => true,
        _ => false,
    };
    if !include {
        return None;
    }
    let payload_hash = serde_json::to_vec(&event.payload)
        .ok()
        .map(|bytes| sha256_prefixed(&bytes))?;
    let restricted_audit_payload = view == "AUDIT"
        && matches!(
            event.visibility_label.as_str(),
            "private_to_player"
                | "investigator_private"
                | "private_to_group"
                | "keeper_only"
                | "ai_internal"
                | "system_only"
                | "system_private"
        );
    let payload = if restricted_audit_payload {
        json!({"payload_hash": payload_hash, "redacted": true})
    } else {
        event.payload.clone()
    };
    Some(json!({
        "event_integrity_hash": event.event_integrity_hash,
        "event_schema_version": event.event_schema_version,
        "event_type": event.event_type,
        "payload": payload,
        "payload_hash": payload_hash,
        "provenance": {
            "kind": event.provenance_kind,
            "recorded_by": event.provenance_recorded_by,
            "reference": event.provenance_reference,
        },
        "recorded_at": event.recorded_at.to_rfc3339(),
        "request_hash": event.request_hash,
        "resource": {"id": event.resource_id, "type": event.resource_type},
        "sequence": event.sequence,
        "stream": {"id": event.stream_id, "version": event.stream_version},
        "visibility": {"label": event.visibility_label, "subject": event.visibility_subject},
    }))
}

fn export_sections(view: &str, records: &[Value], request: &ExportRequest) -> Value {
    let event_refs = |needle: &str| {
        records
            .iter()
            .filter(|record| {
                record["event_type"]
                    .as_str()
                    .is_some_and(|event_type| event_type.contains(needle))
            })
            .map(|record| record["sequence"].clone())
            .collect::<Vec<_>>()
    };
    match view {
        "PLAYER" => json!({
            "discovered_clues": event_refs("Clue"),
            "public_scene_summary": event_refs("Scene"),
            "visible_dice_rolls": event_refs("Dice"),
        }),
        "KEEPER_PRIVATE" => json!({
            "all_public_events": records.iter().filter(|record| {
                matches!(record["visibility"]["label"].as_str(), Some("public" | "party_visible"))
            }).map(|record| record["sequence"].clone()).collect::<Vec<_>>(),
            "hidden_clues": event_refs("Clue"),
            "keeper_truth": records.iter().filter(|record| {
                record["visibility"]["label"] == "keeper_only"
            }).map(|record| record["sequence"].clone()).collect::<Vec<_>>(),
            "npc_secrets": event_refs("Npc"),
        }),
        "AUDIT" => json!({
            "decision_records": event_refs("Decision"),
            "dice_rolls": event_refs("Dice"),
            "model_route_snapshot": request.model_route_snapshot,
            "tool_calls": event_refs("Tool"),
            "visibility_labels": records.iter().filter_map(|record| {
                record["visibility"]["label"].as_str().map(str::to_owned)
            }).collect::<BTreeSet<_>>(),
        }),
        _ => Value::Null,
    }
}

fn utc_from_unix_ms(value: i64) -> Result<DateTime<Utc>, CampaignExportWorkerError> {
    Utc.timestamp_millis_opt(value)
        .single()
        .ok_or_else(|| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_TIME_INVALID"))
}

pub fn artifact_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    artifact_sha256(bytes)
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn safe_relative_key(key: &str) -> bool {
    let path = Path::new(key);
    !path.is_absolute()
        && !key.is_empty()
        && key.len() <= 512
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component.as_os_str().to_str().is_some_and(|part| {
                    !part.is_empty()
                        && part.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                        })
                })
        })
}

pub fn checked_artifact_path(root: &Path, key: &str) -> Result<PathBuf, CampaignExportWorkerError> {
    if !safe_relative_key(key) {
        return Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID",
        ));
    }
    Ok(root.join(key))
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), CampaignExportWorkerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))
}

pub fn remove_artifact(root: &Path, key: &str) -> Result<(), CampaignExportWorkerError> {
    let path = checked_artifact_path(root, key)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(CampaignExportWorkerError::new(
            "CAMPAIGN_EXPORT_DELETE_FAILED",
        )),
    }
}
