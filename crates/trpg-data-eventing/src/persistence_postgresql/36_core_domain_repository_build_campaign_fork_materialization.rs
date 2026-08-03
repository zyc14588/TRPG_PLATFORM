
impl CoreDomainRepository {

    async fn build_campaign_fork_materialization(
        &self,
        request: &RecordCampaignForkRequest,
        snapshot: &CampaignForkSnapshotPreview,
        pending_child_room_id: Option<&str>,
    ) -> Result<CampaignForkMaterialization, CoreDomainRepositoryError> {
        let envelope: ForkSnapshotEnvelope =
            serde_json::from_str(&snapshot.canonical_snapshot_json)
                .map_err(|_| CoreDomainRepositoryError::Integrity("fork_snapshot_shape"))?;
        let source = envelope.state;
        if source.source_campaign_id != request.parent_campaign_id
            || source.source_session_id != request.source_session_id
            || source.session_state.state != "ENDED"
            || source.session_state.started_at_unix_ms == 0
            || source.session_state.ended_at_unix_ms < source.session_state.started_at_unix_ms
            || source.world_state.ruleset_id.trim().is_empty()
            || source.scene_state.is_empty()
            || source
                .combat_state
                .iter()
                .any(|combat| combat.status != "ENDED")
            || source
                .chase_state
                .iter()
                .any(|chase| !matches!(chase.status.as_str(), "ESCAPED" | "CAUGHT"))
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_snapshot_materialization_shape",
            ));
        }

        let child_room_id = if let Some(room_id) = pending_child_room_id {
            EntityId::new(room_id)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("fork_child_room"))?;
            room_id.to_owned()
        } else {
            let child_rooms: Vec<String> = sqlx::query_scalar(
                "SELECT room_id FROM public.rooms WHERE campaign_id = $1 ORDER BY room_id LIMIT 2",
            )
            .bind(&request.child_campaign_id)
            .fetch_all(&self.primary)
            .await
            .map_err(database_error("load_fork_child_room"))?;
            if child_rooms.len() != 1 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_child_room_shape",
                ));
            }
            child_rooms[0].clone()
        };
        let child_scenario_id =
            fork_child_id(&request.fork_id, "scenario", &request.source_session_id)?;
        let child_session_id =
            fork_child_id(&request.fork_id, "session", &request.source_session_id)?;
        let mut identity_ids = BTreeMap::<String, String>::new();
        for character in &source.character_state {
            if identity_ids
                .insert(
                    character.character_id.clone(),
                    fork_child_id(&request.fork_id, "character", &character.character_id)?,
                )
                .is_some()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_character_snapshot_identity",
                ));
            }
        }
        let character_identity_ids = identity_ids.clone();
        for npc in &source.npc_state {
            if identity_ids
                .insert(
                    npc.npc_id.clone(),
                    fork_child_id(&request.fork_id, "npc", &npc.npc_id)?,
                )
                .is_some()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_gameplay_snapshot_identity",
                ));
            }
        }

        let mut scene_ids = BTreeMap::new();
        for scene in &source.scene_state {
            if !matches!(scene.state.as_str(), "READY" | "ACTIVE" | "CLOSED")
                || scene.scene_key.trim().is_empty()
                || scene.name.trim().is_empty()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_scene_snapshot_shape",
                ));
            }
            scene_ids.insert(
                scene.scene_id.clone(),
                fork_child_id(&request.fork_id, "scene", &scene.scene_id)?,
            );
        }
        let child_active_scene_id =
            source
                .session_state
                .active_scene_id
                .as_ref()
                .map(|source_scene_id| {
                    scene_ids.get(source_scene_id).cloned().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_active_scene_missing"),
                    )
                })
                .transpose()?;

        let scenario_endings = source
            .conclusion_state
            .iter()
            .map(|ending| {
                Ok(serde_json::json!({
                    "id": ending.ending_id,
                    "summary": ending.summary,
                    "growth_awards": fork_materialized_growth_awards(
                        ending,
                        &character_identity_ids,
                    )?
                }))
            })
            .collect::<Result<Vec<Value>, CoreDomainRepositoryError>>()?;
        let scenario_document = serde_json::json!({
            "schema_version": 1,
            "kind": "FORK_SNAPSHOT",
            "fork_id": request.fork_id,
            "source_campaign_id": request.parent_campaign_id,
            "source_session_id": request.source_session_id,
            "source_snapshot_hash": snapshot.snapshot_hash,
            "endings": scenario_endings
        });
        let scenario_document_json = serde_json::to_string(&scenario_document)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let scenario_content_hash = format!(
            "sha256:{:x}",
            Sha256::digest(scenario_document_json.as_bytes())
        );
        let mut rows = vec![CampaignForkMaterializedRow::Scenario {
            scenario_id: child_scenario_id.clone(),
            ruleset_id: source.world_state.ruleset_id.clone(),
            format_version: "fork-snapshot-1".to_owned(),
            content_hash: scenario_content_hash,
            document_json: scenario_document_json,
            visibility_label: source.world_state.visibility_label.clone(),
            visibility_subject: source.world_state.visibility_subject.clone(),
        }];

        for character in &source.character_state {
            if !matches!(character.state.as_str(), "DRAFT" | "SUBMITTED" | "APPROVED")
                || character.display_name.trim().is_empty()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_character_snapshot_shape",
                ));
            }
            let sheet =
                character
                    .current_sheet
                    .as_ref()
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_character_sheet_missing",
                    ))?;
            if !sheet.sheet_json.is_object() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_character_sheet_shape",
                ));
            }
            let (visibility_label, visibility_subject) =
                derive_fork_character_visibility(character, sheet)?;
            let character_id = identity_ids
                .get(&character.character_id)
                .expect("character mapping was constructed above")
                .clone();
            let sheet_version_id =
                fork_child_id(&request.fork_id, "sheet", &character.character_id)?;
            rows.push(CampaignForkMaterializedRow::Character {
                character_id,
                owner_user_id: character.owner_user_id.clone(),
                display_name: character.display_name.clone(),
                state: character.state.clone(),
                initial_version_locked: character.initial_version_locked,
                sheet_version_id,
                sheet_json: serde_json::to_string(&sheet.sheet_json)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                sheet_locked: sheet.locked,
                visibility_label,
                visibility_subject,
            });
        }

        rows.push(CampaignForkMaterializedRow::Session {
            session_id: child_session_id.clone(),
            room_id: child_room_id.clone(),
            scenario_id: child_scenario_id.clone(),
            state: "ENDED".to_owned(),
            active_scene_id: child_active_scene_id,
            started_at_unix_ms: source.session_state.started_at_unix_ms,
            ended_at_unix_ms: source.session_state.ended_at_unix_ms,
            visibility_label: source.session_state.visibility_label.clone(),
            visibility_subject: source.session_state.visibility_subject.clone(),
        });
        for scene in &source.scene_state {
            rows.push(CampaignForkMaterializedRow::Scene {
                scene_id: scene_ids
                    .get(&scene.scene_id)
                    .expect("scene mapping was constructed above")
                    .clone(),
                session_id: child_session_id.clone(),
                scenario_id: child_scenario_id.clone(),
                room_id: child_room_id.clone(),
                scene_key: scene.scene_key.clone(),
                name: scene.name.clone(),
                state: scene.state.clone(),
                visibility_label: scene.visibility_label.clone(),
                visibility_subject: scene.visibility_subject.clone(),
            });
        }

        append_campaign_fork_state_rows(
            request,
            &source,
            &identity_ids,
            &child_session_id,
            &mut rows,
        )?;

        let mut row_digest = Sha256::new();
        for row in &rows {
            let encoded =
                serde_json::to_vec(row).map_err(|_| CoreDomainRepositoryError::Serialization)?;
            row_digest.update((encoded.len() as u64).to_be_bytes());
            row_digest.update(encoded);
        }
        let materialized_root_hash = format!("sha256:{:x}", row_digest.finalize());
        let child_state = serde_json::json!({
            "schema_version": 2,
            "kind": "CONTENT_ADDRESSED_FORK_MATERIALIZATION",
            "fork_id": request.fork_id,
            "child_campaign_id": request.child_campaign_id,
            "source_snapshot_hash": snapshot.snapshot_hash,
            "materialized_row_count": rows.len(),
            "materialized_root_hash": materialized_root_hash
        });
        let child_state_json = serde_json::to_string(&child_state)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let child_snapshot_hash =
            format!("sha256:{:x}", Sha256::digest(child_state_json.as_bytes()));
        let batches = fork_materialization_batches(&rows)?;
        Ok(CampaignForkMaterialization {
            child_session_id,
            child_scenario_id,
            child_state_json,
            child_snapshot_hash,
            rows,
            batches,
        })
    }
}
